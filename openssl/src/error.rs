//! Errors returned by OpenSSL library.
//!
//! OpenSSL errors are stored in an `ErrorStack`.  Most methods in the crate
//! returns a `Result<T, ErrorStack>` type.
//!
//! # Examples
//!
//! ```
//! use openssl::error::ErrorStack;
//! use openssl::bn::BigNum;
//!
//! let an_error = BigNum::from_dec_str("Cannot parse letters");
//! match an_error {
//!     Ok(_)  => (),
//!     Err(e) => println!("Parsing Error: {:?}", e),
//! }
//! ```
use libc::{c_char, c_ulong};
use std::fmt;
use std::error;
use std::ffi::CStr;
use std::io;
use std::str;
use std::ptr;
use std::borrow::Cow;

use ffi;

/// Collection of [`Error`]s from OpenSSL.
///
/// [`Error`]: struct.Error.html
#[derive(Debug, Clone)]
pub struct ErrorStack(Vec<Error>);

impl ErrorStack {
    /// Returns the contents of the OpenSSL error stack.
    pub fn get() -> ErrorStack {
        let mut vec = vec![];
        while let Some(err) = Error::get() {
            vec.push(err);
        }
        ErrorStack(vec)
    }
}

impl ErrorStack {
    /// Returns the errors in the stack.
    pub fn errors(&self) -> &[Error] {
        &self.0
    }
}

impl fmt::Display for ErrorStack {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for err in &self.0 {
            if !first {
                fmt.write_str(", ")?;
            }
            write!(fmt, "{}", err)?;
            first = false;
        }
        Ok(())
    }
}

impl error::Error for ErrorStack {
    fn description(&self) -> &str {
        "An OpenSSL error stack"
    }
}

impl From<ErrorStack> for io::Error {
    fn from(e: ErrorStack) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }
}

impl From<ErrorStack> for fmt::Error {
    fn from(_: ErrorStack) -> fmt::Error {
        fmt::Error
    }
}

/// An error reported from OpenSSL.
#[derive(Clone)]
pub struct Error {
    code: c_ulong,
    file: *const c_char,
    line: u32,
    data: Option<Cow<'static, str>>,
}

unsafe impl Sync for Error {}
unsafe impl Send for Error {}

impl Error {
    /// Returns the first error on the OpenSSL error stack.
    pub fn get() -> Option<Error> {
        unsafe {
            ffi::init();

            let mut file = ptr::null();
            let mut line = 0;
            let mut data = ptr::null();
            let mut flags = 0;
            match ffi::ERR_get_error_line_data(&mut file, &mut line, &mut data, &mut flags) {
                0 => None,
                code => {
                    // The memory referenced by data is only valid until that slot is overwritten
                    // in the error stack, so we'll need to copy it off if it's dynamic
                    let data = if flags & ffi::ERR_TXT_STRING != 0 {
                        let bytes = CStr::from_ptr(data as *const _).to_bytes();
                        let data = str::from_utf8(bytes).unwrap();
                        let data = if flags & ffi::ERR_TXT_MALLOCED != 0 {
                            Cow::Owned(data.to_string())
                        } else {
                            Cow::Borrowed(data)
                        };
                        Some(data)
                    } else {
                        None
                    };
                    Some(Error {
                        code,
                        file,
                        line: line as u32,
                        data,
                    })
                }
            }
        }
    }

    /// Returns the raw OpenSSL error code for this error.
    pub fn code(&self) -> c_ulong {
        self.code
    }

    /// Returns the name of the library reporting the error, if available.
    pub fn library(&self) -> Option<&'static str> {
        unsafe {
            let cstr = ffi::ERR_lib_error_string(self.code);
            if cstr.is_null() {
                return None;
            }
            let bytes = CStr::from_ptr(cstr as *const _).to_bytes();
            Some(str::from_utf8(bytes).unwrap())
        }
    }

    /// Returns the name of the function reporting the error.
    pub fn function(&self) -> Option<&'static str> {
        unsafe {
            let cstr = ffi::ERR_func_error_string(self.code);
            if cstr.is_null() {
                return None;
            }
            let bytes = CStr::from_ptr(cstr as *const _).to_bytes();
            Some(str::from_utf8(bytes).unwrap())
        }
    }

    /// Returns the reason for the error.
    pub fn reason(&self) -> Option<&'static str> {
        unsafe {
            let cstr = ffi::ERR_reason_error_string(self.code);
            if cstr.is_null() {
                return None;
            }
            let bytes = CStr::from_ptr(cstr as *const _).to_bytes();
            Some(str::from_utf8(bytes).unwrap())
        }
    }

    /// Returns the name of the source file which encountered the error.
    pub fn file(&self) -> &'static str {
        unsafe {
            assert!(!self.file.is_null());
            let bytes = CStr::from_ptr(self.file as *const _).to_bytes();
            str::from_utf8(bytes).unwrap()
        }
    }

    /// Returns the line in the source file which encountered the error.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns additional data describing the error.
    pub fn data(&self) -> Option<&str> {
        self.data.as_ref().map(|s| &**s)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("Error");
        builder.field("code", &self.code());
        if let Some(library) = self.library() {
            builder.field("library", &library);
        }
        if let Some(function) = self.function() {
            builder.field("function", &function);
        }
        if let Some(reason) = self.reason() {
            builder.field("reason", &reason);
        }
        builder.field("file", &self.file());
        builder.field("line", &self.line());
        if let Some(data) = self.data() {
            builder.field("data", &data);
        }
        builder.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "error:{:08X}", self.code())?;
        match self.library() {
            Some(l) => write!(fmt, ":{}", l)?,
            None => write!(fmt, ":lib({})", ffi::ERR_GET_LIB(self.code()))?,
        }
        match self.function() {
            Some(f) => write!(fmt, ":{}", f)?,
            None => write!(fmt, ":func({})", ffi::ERR_GET_FUNC(self.code()))?,
        }
        match self.reason() {
            Some(r) => write!(fmt, ":{}", r)?,
            None => write!(fmt, ":reason({})", ffi::ERR_GET_FUNC(self.code()))?,
        }
        write!(
            fmt,
            ":{}:{}:{}",
            self.file(),
            self.line(),
            self.data().unwrap_or("")
        )
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "an OpenSSL error"
    }
}
