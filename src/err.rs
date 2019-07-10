use snafu::{Backtrace, ErrorCompat, Snafu};
use std::io;
use std::path;
use std::process;
use std::result;
use std::string;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("file io error [{:?}]: {}", path, source))]
    FileIO {
        path: path::PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("command io error: [{:?}]: {}", cmd, source))]
    CommandIO {
        cmd: process::Command,
        source: io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("utf8 string decode failed: {}", source))]
    UTF8Decode {
        source: string::FromUtf8Error,
        backtrace: Backtrace,
    },

    #[snafu(display("rmp encode error: {}", source))]
    RMPEncode {
        source: rmp_serde::encode::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("rmp decode error: {}", source))]
    RMPDecode {
        source: rmp_serde::decode::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("unable to get kernel store info"))]
    GetKernelStore { backtrace: Backtrace },
}

impl From<string::FromUtf8Error> for Error {
    fn from(err: string::FromUtf8Error) -> Self {
        Error::UTF8Decode {
            source: err,
            backtrace: Backtrace::new(),
        }
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(err: rmp_serde::encode::Error) -> Self {
        Error::RMPEncode {
            source: err,
            backtrace: Backtrace::new(),
        }
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(err: rmp_serde::decode::Error) -> Self {
        Error::RMPDecode {
            source: err,
            backtrace: Backtrace::new(),
        }
    }
}

pub fn display_error(err: Error) {
    eprintln!("{}", err);

    if let Some(backtrace) = err.backtrace() {
        eprintln!("backtrace:\n{}", backtrace);
    }
}
