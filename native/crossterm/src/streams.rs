//! Stream handles are borrowed at open and duplicated before terminal mutation.
use crate::SessionError;
use ratatui::{
    backend::{Backend, ClearType, CrosstermBackend, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
};
use std::io::{self, Write};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{UnixInput, UnixModes};

/// Unix descriptors must remain valid until `open` returns. The session owns
/// duplicates, never the caller's originals. Standard streams work on Windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalStream {
    #[default]
    Standard,
    FileDescriptor(i32),
    /// Open the controlling terminal at /dev/tty (Unix only).
    Tty,
}

pub(crate) enum Output {
    Standard(io::Stdout),
    #[cfg(unix)]
    File(std::fs::File),
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Standard(writer) => writer.write(bytes),
            #[cfg(unix)]
            Self::File(writer) => writer.write(bytes),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Standard(writer) => writer.flush(),
            #[cfg(unix)]
            Self::File(writer) => writer.flush(),
        }
    }
}

pub(crate) struct Streams {
    pub output: Output,
    pub render_output: Output,
    #[cfg(unix)]
    pub custom: Option<(UnixModes, UnixInput)>,
}
impl Streams {
    pub fn open(input: TerminalStream, output: TerminalStream) -> Result<Self, SessionError> {
        if input == TerminalStream::Standard && output == TerminalStream::Standard {
            return Ok(Self {
                output: Output::Standard(io::stdout()),
                render_output: Output::Standard(io::stdout()),
                #[cfg(unix)]
                custom: None,
            });
        }
        #[cfg(unix)]
        {
            unix::open(input, output).map_err(|e| SessionError::io("open terminal streams", e))
        }
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedCapability(
                "custom terminal streams on this platform",
            ))
        }
    }
}

/// Crossterm draws ANSI commands, but its size/cursor queries use global stdio.
/// Override those queries for a selected Unix terminal.
pub(crate) struct TerminalBackend {
    backend: CrosstermBackend<Output>,
    #[cfg(unix)]
    size_handle: Option<std::fs::File>,
}
impl TerminalBackend {
    pub fn new(output: Output) -> io::Result<Self> {
        #[cfg(unix)]
        let size_handle = match &output {
            Output::File(file) => Some(file.try_clone()?),
            Output::Standard(_) => None,
        };
        Ok(Self {
            backend: CrosstermBackend::new(output),
            #[cfg(unix)]
            size_handle,
        })
    }
}
impl Backend for TerminalBackend {
    type Error = io::Error;
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.backend.draw(content)
    }
    fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()
    }
    fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()
    }
    fn get_cursor_position(&mut self) -> io::Result<Position> {
        #[cfg(unix)]
        if self.size_handle.is_some() {
            // Fullscreen rendering never needs a cursor query. Do not start a
            // competing reader just to implement an unused backend operation.
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "cursor queries require sole input ownership",
            ));
        }
        self.backend.get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.backend.set_cursor_position(position)
    }
    fn clear(&mut self) -> io::Result<()> {
        self.backend.clear()
    }
    fn clear_region(&mut self, region: ClearType) -> io::Result<()> {
        self.backend.clear_region(region)
    }
    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.backend.append_lines(n)
    }
    fn size(&self) -> io::Result<Size> {
        #[cfg(unix)]
        if let Some(file) = &self.size_handle {
            return unix::window_size(file).map(|size| size.columns_rows);
        }
        self.backend.size()
    }
    fn window_size(&mut self) -> io::Result<WindowSize> {
        #[cfg(unix)]
        if let Some(file) = &self.size_handle {
            return unix::window_size(file);
        }
        self.backend.window_size()
    }
    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.backend)
    }
}
