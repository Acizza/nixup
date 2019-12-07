use snafu::{Backtrace, ErrorCompat, GenerateBacktrace, Snafu};
use std::io;
use std::path;
use std::result;

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

    #[snafu(display("sqlite error: {}", source))]
    Rusqlite {
        source: rusqlite::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("must run as root"))]
    RunAsRoot,
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(err: rmp_serde::encode::Error) -> Self {
        Error::RMPEncode {
            source: err,
            backtrace: Backtrace::generate(),
        }
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(err: rmp_serde::decode::Error) -> Self {
        Error::RMPDecode {
            source: err,
            backtrace: Backtrace::generate(),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Rusqlite {
            source: err,
            backtrace: Backtrace::generate(),
        }
    }
}

pub fn display_error(err: Error) {
    eprintln!("{}", err);

    if let Some(backtrace) = err.backtrace() {
        eprintln!("backtrace:\n{}", backtrace);
    }
}
