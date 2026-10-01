//! C boundary for terminal sessions. See `include/ratatui_js.h` for ownership.
use ratatui_js_crossterm::{ErrorDescription, EventPoll, Session, SessionOptions, TerminalStream};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice,
    time::Duration,
};

pub const ABI_VERSION: u32 = 1;
pub const OK: u32 = 0;
pub const TIMEOUT: u32 = 1;
pub const CLOSED: u32 = 2;
pub const ERROR: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RtBytes {
    pub data: *mut u8,
    pub len: usize,
}
impl Default for RtBytes {
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
        }
    }
}
impl RtBytes {
    fn owned(bytes: Vec<u8>) -> Self {
        let bytes = bytes.into_boxed_slice();
        let len = bytes.len();
        Self {
            data: Box::into_raw(bytes).cast::<u8>(),
            len,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_abi_version() -> u32 {
    ABI_VERSION
}
#[unsafe(no_mangle)]
pub extern "C" fn rt_protocol_version() -> u32 {
    ratatui_js_core::PROTOCOL_VERSION
}

fn boundary(
    error: *mut RtBytes,
    operation: impl FnOnce() -> Result<u32, Box<ErrorDescription>>,
) -> u32 {
    if error.is_null() {
        return ERROR;
    }
    // SAFETY: the caller supplies writable, aligned output storage.
    unsafe {
        error.write(RtBytes::default());
    }
    let result = catch_unwind(AssertUnwindSafe(operation)).unwrap_or_else(|_| {
        Err(Box::new(ErrorDescription::new(
            "panic",
            "ABI operation panicked",
        )))
    });
    match result {
        Ok(status) => status,
        Err(description) => {
            // This schema contains only strings/vectors, so serialization cannot fail.
            let bytes = serde_json::to_vec(&description).expect("serialize error");
            unsafe {
                error.write(RtBytes::owned(bytes));
            }
            ERROR
        }
    }
}
fn invalid(message: &str) -> Box<ErrorDescription> {
    Box::new(ErrorDescription::new("invalidArgument", message))
}

/// # Safety
/// Outputs must be non-null, aligned, writable, and disjoint. See the C header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_create(
    abi: u32,
    protocol: u32,
    flags: u32,
    handle: *mut *mut Session,
    error: *mut RtBytes,
) -> u32 {
    // SAFETY: this compatibility entrypoint has the same pointer contract.
    unsafe { rt_create_with_streams(abi, protocol, flags, -1, -1, handle, error) }
}

/// # Safety
/// Same output contract as rt_create. Descriptors are borrowed until return;
/// -1 selects the standard stream, -2 opens /dev/tty, nonnegative values select
/// Unix file descriptors. The session owns duplicates of selected descriptors.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_create_with_streams(
    abi: u32,
    protocol: u32,
    flags: u32,
    input: i32,
    output: i32,
    handle: *mut *mut Session,
    error: *mut RtBytes,
) -> u32 {
    if !handle.is_null() {
        unsafe {
            handle.write(ptr::null_mut());
        }
    }
    boundary(error, || {
        if handle.is_null() {
            return Err(invalid("null handle output"));
        }
        if abi != ABI_VERSION {
            return Err(Box::new(ErrorDescription::new(
                "unsupportedAbi",
                "unsupported ABI version",
            )));
        }
        if protocol != ratatui_js_core::PROTOCOL_VERSION {
            return Err(Box::new(ErrorDescription::new(
                "unsupportedProtocol",
                "unsupported protocol version",
            )));
        }
        if flags & !31 != 0 {
            return Err(invalid("unknown session flags"));
        }
        let select = |value| match value {
            -1 => Ok(TerminalStream::Standard),
            -2 => Ok(TerminalStream::Tty),
            0.. => Ok(TerminalStream::FileDescriptor(value)),
            _ => Err(invalid("invalid terminal stream selector")),
        };
        let session = Session::open(SessionOptions {
            input: select(input)?,
            output: select(output)?,
            alternate_screen: flags & 1 != 0,
            mouse_capture: flags & 2 != 0,
            bracketed_paste: flags & 4 != 0,
            focus_reporting: flags & 8 != 0,
            enhanced_keyboard: flags & 16 != 0,
        })
        .map_err(|e| Box::new(e.description()))?;
        unsafe {
            handle.write(Box::into_raw(Box::new(session)));
        }
        Ok(OK)
    })
}

/// # Safety
/// Handle is live; input is readable for len bytes; outputs are writable and
/// disjoint from input and each other. All storage lives until the call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_render(
    handle: *const Session,
    data: *const u8,
    len: usize,
    output: *mut RtBytes,
    error: *mut RtBytes,
) -> u32 {
    if !output.is_null() {
        unsafe {
            output.write(RtBytes::default());
        }
    }
    boundary(error, || {
        if handle.is_null() || output.is_null() || data.is_null() {
            return Err(invalid("null render argument"));
        }
        if len > ratatui_js_core::MAX_FRAME_BYTES {
            let mut error = ErrorDescription::new("invalidFrame", "$: encoded frame is too large");
            error.path = Some("$".into());
            return Err(Box::new(error));
        }
        let result = unsafe { (&*handle).render_json(slice::from_raw_parts(data, len)) }
            .map_err(|e| Box::new(e.description()))?;
        let bytes = serde_json::to_vec(&result).map_err(|e| invalid(&e.to_string()))?;
        unsafe {
            output.write(RtBytes::owned(bytes));
        }
        Ok(OK)
    })
}

/// # Safety
/// Handle is live and output/error are writable, aligned, and disjoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_poll_event(
    handle: *const Session,
    timeout_ms: u32,
    output: *mut RtBytes,
    error: *mut RtBytes,
) -> u32 {
    if !output.is_null() {
        unsafe {
            output.write(RtBytes::default());
        }
    }
    boundary(error, || {
        if handle.is_null() || output.is_null() {
            return Err(invalid("null poll argument"));
        }
        match unsafe { (&*handle).poll_event(Duration::from_millis(timeout_ms.into())) }
            .map_err(|e| Box::new(e.description()))?
        {
            EventPoll::Timeout => Ok(TIMEOUT),
            EventPoll::Closed => Ok(CLOSED),
            EventPoll::Event(event) => {
                let bytes = serde_json::to_vec(&event).map_err(|e| invalid(&e.to_string()))?;
                unsafe {
                    output.write(RtBytes::owned(bytes));
                }
                Ok(OK)
            }
        }
    })
}

/// # Safety
/// Handle is live; error is writable. May overlap render/poll/close calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_close(handle: *const Session, error: *mut RtBytes) -> u32 {
    boundary(error, || {
        if handle.is_null() {
            return Err(invalid("null session"));
        }
        unsafe { (&*handle).close() }.map_err(|e| Box::new(e.description()))?;
        Ok(OK)
    })
}

/// # Safety
/// Handle has been closed (even if close failed), all calls have returned, and
/// it will never be used again. Error is writable and disjoint from the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_destroy(handle: *mut Session, error: *mut RtBytes) -> u32 {
    boundary(error, || {
        if handle.is_null() {
            return Err(invalid("null session"));
        }
        // Calling close first also gives callers a safe cleanup fallback.
        let session = unsafe { Box::from_raw(handle) };
        let outcome = session.close().map_err(|e| Box::new(e.description()));
        drop(session);
        outcome?;
        Ok(OK)
    })
}

/// # Safety
/// The exact native buffer pair is released once, after its readers finish.
/// A null/zero pair is allowed. Forged pointers or lengths are undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rt_bytes_free(data: *mut u8, len: usize) {
    if !data.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)));
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panic_is_structured_and_buffer_is_owned() {
        let mut error = RtBytes::default();
        assert_eq!(boundary(&mut error, || panic!("test panic")), ERROR);
        let value: serde_json::Value =
            unsafe { serde_json::from_slice(slice::from_raw_parts(error.data, error.len)) }
                .unwrap();
        assert_eq!(value["code"], "panic");
        unsafe {
            rt_bytes_free(error.data, error.len);
        }
    }
}
