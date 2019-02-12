use failure::Fail;

macro_rules! impl_err_conv {
    ($struct:ident, $($from:ty => $to:ident,)+) => {
        $(
        impl From<$from> for $struct {
            fn from(f: $from) -> $struct {
                $struct::$to(f)
            }
        }
        )+
    };
}

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "store error")]
    Store(#[cause] StoreError),

    #[fail(display = "io error")]
    Io(#[cause] std::io::Error),

    #[fail(display = "failed to get cache directory")]
    FailedToGetCacheDir,

    #[fail(display = "message pack serialization failed")]
    RMPEncode(#[cause] rmp_serde::encode::Error),

    #[fail(display = "message pack deserialization failed")]
    RMPDecode(#[cause] rmp_serde::decode::Error),

    #[fail(display = "failed to execute process")]
    FailedToExecuteProcess(#[cause] std::io::Error),

    #[fail(display = "process returned bad exit code: {}", _0)]
    BadProcessExitCode(i32),
}

impl_err_conv!(Error,
    StoreError => Store,
    std::io::Error => Io,
    rmp_serde::encode::Error => RMPEncode,
    rmp_serde::decode::Error => RMPDecode,
);

#[derive(Fail, Debug)]
pub enum StoreError {
    #[fail(display = "io error")]
    Io(#[cause] std::io::Error),

    #[fail(display = "utf8 error")]
    Utf8(#[cause] std::string::FromUtf8Error),

    #[fail(display = "received unexpected command output")]
    MalformedOutput,
}

impl_err_conv!(StoreError,
    std::io::Error => Io,
    std::string::FromUtf8Error => Utf8,
);
