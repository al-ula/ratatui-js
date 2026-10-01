use super::{Output, Streams, TerminalStream};
use crate::{TerminalEvent, events::normalize, input::InputSource};
use ratatui::{
    backend::WindowSize,
    crossterm::event::{Event, KeyboardEnhancementFlags},
    layout::Size,
};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd},
    time::{Duration, Instant},
};
#[path = "parse.rs"]
mod parse;

#[derive(Debug, PartialEq, Eq)]
enum InternalEvent {
    Event(Event),
    CursorPosition(u16, u16),
    KeyboardEnhancementFlags(KeyboardEnhancementFlags),
    PrimaryDeviceAttributes,
}

fn descriptor(selection: TerminalStream, standard: i32) -> io::Result<File> {
    let fd = match selection {
        TerminalStream::Standard => standard,
        TerminalStream::FileDescriptor(fd) if fd >= 0 => fd,
        TerminalStream::FileDescriptor(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "negative terminal descriptor",
            ));
        }
        TerminalStream::Tty => return OpenOptions::new().read(true).write(true).open("/dev/tty"),
    };
    // SAFETY: fcntl validates fd; the returned descriptor is newly owned. Do
    // not construct BorrowedFd from an untrusted possibly-invalid ABI integer.
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: duplicate was returned by fcntl and has no other Rust owner.
    Ok(unsafe { File::from_raw_fd(duplicate) })
}
fn check(file: &File, input: bool) -> io::Result<()> {
    // SAFETY: the file owns a live descriptor; neither call stores a pointer.
    let mode = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    if mode < 0 {
        return Err(io::Error::last_os_error());
    }
    let access = mode & libc::O_ACCMODE;
    if (input && access == libc::O_WRONLY) || (!input && access == libc::O_RDONLY) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "terminal descriptor has wrong access mode",
        ));
    }
    // Nonblocking reads/writes make exact output and blocking read semantics
    // unreliable. Duplicates share file status flags, so never change them.
    if mode & libc::O_NONBLOCK != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "terminal descriptor must be blocking",
        ));
    }
    Ok(())
}
pub(super) fn open(input: TerminalStream, output: TerminalStream) -> io::Result<Streams> {
    // Validate both borrowed selectors before duplication: duplicating input
    // could otherwise reuse an invalid output descriptor number and mask it.
    for (selection, standard) in [(input, 0), (output, 1)] {
        let fd = match selection {
            TerminalStream::Standard => standard,
            TerminalStream::FileDescriptor(fd) => fd,
            TerminalStream::Tty => continue,
        };
        // SAFETY: fcntl accepts arbitrary integers and validates the handle.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let input = descriptor(input, 0)?;
    let output = descriptor(output, 1)?;
    check(&input, true)?;
    check(&output, false)?;
    let modes = UnixModes {
        input: input.try_clone()?,
        output: output.try_clone()?,
        original: None,
    };
    let source = UnixInput {
        input,
        output: output.try_clone()?,
        buffer: Vec::new(),
        pending: VecDeque::new(),
        last_size: None,
        escape_since: None,
    };
    Ok(Streams {
        render_output: Output::File(output.try_clone()?),
        output: Output::File(output),
        custom: Some((modes, source)),
    })
}

pub(crate) fn window_size(file: &File) -> io::Result<WindowSize> {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::uninit();
    // SAFETY: ioctl writes a winsize to valid storage on success.
    if unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCGWINSZ, size.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let size = unsafe { size.assume_init() };
    if size.ws_col == 0 || size.ws_row == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal size is zero",
        ));
    }
    Ok(WindowSize {
        columns_rows: Size::new(size.ws_col, size.ws_row),
        pixels: Size::new(size.ws_xpixel, size.ws_ypixel),
    })
}

pub(crate) struct UnixModes {
    input: File,
    output: File,
    original: Option<libc::termios>,
}
impl UnixModes {
    pub fn is_terminal(&self) -> bool {
        // SAFETY: both descriptors are owned and live.
        unsafe {
            libc::isatty(self.input.as_raw_fd()) == 1 && libc::isatty(self.output.as_raw_fd()) == 1
        }
    }
    fn attributes(&self) -> io::Result<libc::termios> {
        let mut attrs = std::mem::MaybeUninit::uninit();
        // SAFETY: tcgetattr writes termios on success to valid storage.
        if unsafe { libc::tcgetattr(self.input.as_raw_fd(), attrs.as_mut_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { attrs.assume_init() })
    }
    pub fn raw_active(&self) -> io::Result<bool> {
        let attrs = self.attributes()?;
        Ok(attrs.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG) == 0)
    }
    pub fn enable_raw(&mut self) -> io::Result<()> {
        let mut attrs = self.attributes()?;
        self.original = Some(attrs);
        // SAFETY: cfmakeraw modifies initialized termios; tcsetattr borrows it.
        unsafe {
            libc::cfmakeraw(&mut attrs);
        }
        attrs.c_cc[libc::VMIN] = 1;
        attrs.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(self.input.as_raw_fd(), libc::TCSANOW, &attrs) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn restore(&mut self) -> io::Result<()> {
        if let Some(attrs) = &self.original {
            // SAFETY: original attributes and input remain alive through cleanup.
            if unsafe { libc::tcsetattr(self.input.as_raw_fd(), libc::TCSANOW, attrs) } < 0 {
                return Err(io::Error::last_os_error());
            }
            self.original = None;
        }
        Ok(())
    }
}

pub(crate) struct UnixInput {
    input: File,
    output: File,
    buffer: Vec<u8>,
    pending: VecDeque<InternalEvent>,
    last_size: Option<Size>,
    escape_since: Option<Instant>,
}
impl UnixInput {
    pub fn prepare(&mut self) -> io::Result<()> {
        self.last_size = Some(window_size(&self.output)?.columns_rows);
        Ok(())
    }
    fn read_available(&mut self, timeout: Duration) -> io::Result<()> {
        let mut poll = libc::pollfd {
            fd: self.input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll borrows exactly one initialized pollfd.
        let result = unsafe {
            libc::poll(
                &mut poll,
                1,
                timeout.as_millis().min(i32::MAX as u128) as i32,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(error);
        }
        if result == 0 {
            if self.buffer == b"\x1b"
                && self
                    .escape_since
                    .is_some_and(|start| start.elapsed() >= Duration::from_millis(50))
            {
                self.pending.push_back(InternalEvent::Event(Event::Key(
                    ratatui::crossterm::event::KeyCode::Esc.into(),
                )));
                self.buffer.clear();
                self.escape_since = None;
            }
            return Ok(());
        }
        if poll.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(io::Error::other("terminal input poll failed"));
        }
        let mut bytes = [0u8; 4096];
        let count = self.input.read(&mut bytes)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "terminal input closed",
            ));
        }
        for byte in &bytes[..count] {
            self.buffer.push(*byte);
            if self.buffer.len() == 1 && *byte == 0x1b {
                self.escape_since = Some(Instant::now());
            }
            if self.buffer.len() > 1_048_576 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "terminal input sequence is too large",
                ));
            }
            match parse::parse_event(&self.buffer, true) {
                Ok(Some(event)) => {
                    if self.pending.len() >= 4096 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "too many terminal events during negotiation",
                        ));
                    }
                    self.pending.push_back(event);
                    self.buffer.clear();
                    self.escape_since = None;
                }
                Ok(None) => {}
                Err(_) => {
                    self.buffer.clear();
                    self.escape_since = None;
                }
            }
        }
        Ok(())
    }
    /// Negotiation completes before the reader thread starts. Retain ordinary
    /// events received during the probe for that same reader.
    pub fn keyboard_supported(&mut self, output: &mut impl io::Write) -> io::Result<bool> {
        output.write_all(b"\x1b[?u\x1b[c")?;
        output.flush()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut flags = false;
        loop {
            let mut retained = VecDeque::new();
            while let Some(event) = self.pending.pop_front() {
                match event {
                    InternalEvent::KeyboardEnhancementFlags(_) => flags = true,
                    InternalEvent::PrimaryDeviceAttributes => {
                        retained.append(&mut self.pending);
                        self.pending = retained;
                        return Ok(flags);
                    }
                    event => retained.push_back(event),
                }
            }
            self.pending = retained;
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "keyboard support query timed out",
                ));
            }
            self.read_available(Duration::from_millis(50))?;
        }
    }
}
impl UnixInput {
    fn take_event(&mut self) -> Option<TerminalEvent> {
        while let Some(event) = self.pending.pop_front() {
            if let InternalEvent::Event(event) = event
                && let Some(event) = normalize(event)
            {
                return Some(event);
            }
        }
        None
    }
}
impl InputSource for UnixInput {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>> {
        let size = window_size(&self.output)?.columns_rows;
        if let Some(previous) = self.last_size.replace(size)
            && previous != size
        {
            return Ok(Some(TerminalEvent::Resize {
                width: size.width,
                height: size.height,
            }));
        }
        if let Some(event) = self.take_event() {
            return Ok(Some(event));
        }
        // Read only one bounded batch per call. Even a continuous stream of
        // ignored terminal responses must return control for cancellation.
        self.read_available(timeout)?;
        Ok(self.take_event())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use std::io::Write;

    fn source() -> (UnixInput, File) {
        let mut descriptors = [-1; 2];
        // SAFETY: pipe writes two newly owned fds into valid storage on success.
        assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
        // SAFETY: pipe just allocated these distinct descriptors; each has one owner.
        let input = unsafe { File::from_raw_fd(descriptors[0]) };
        let writer = unsafe { File::from_raw_fd(descriptors[1]) };
        (
            UnixInput {
                input,
                output: File::open("/dev/null").unwrap(),
                buffer: Vec::new(),
                pending: VecDeque::new(),
                last_size: None,
                escape_since: None,
            },
            writer,
        )
    }

    #[test]
    fn fragmented_sequences_and_unicode_survive_multiple_reads() {
        let (mut source, mut writer) = source();
        writer.write_all(b"\x1b[").unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert!(source.take_event().is_none());
        writer.write_all(b"A").unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert_eq!(
            source.take_event(),
            normalize(Event::Key(KeyCode::Up.into()))
        );
        let bytes = "界".as_bytes();
        writer.write_all(&bytes[..1]).unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert!(source.take_event().is_none());
        writer.write_all(&bytes[1..]).unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert_eq!(
            source.take_event(),
            normalize(Event::Key(KeyCode::Char('界').into()))
        );
        writer.write_all(b"\n").unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert_eq!(
            source.take_event(),
            normalize(Event::Key(ratatui::crossterm::event::KeyEvent::new(
                KeyCode::Char('j'),
                KeyModifiers::CONTROL
            )))
        );
    }

    #[test]
    fn standalone_escape_finishes_after_ambiguity_timeout() {
        let (mut source, mut writer) = source();
        writer.write_all(b"\x1b").unwrap();
        source.read_available(Duration::ZERO).unwrap();
        assert!(source.take_event().is_none());
        source.escape_since = Some(Instant::now() - Duration::from_millis(60));
        source.read_available(Duration::ZERO).unwrap();
        assert_eq!(
            source.take_event(),
            normalize(Event::Key(KeyCode::Esc.into()))
        );
    }

    #[test]
    fn eof_is_reported_instead_of_spinning() {
        let (mut source, writer) = source();
        drop(writer);
        assert_eq!(
            source.read_available(Duration::ZERO).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
