use crate::err::{self, Result};
use diesel::prelude::*;
use snafu::ensure;

table! {
    #[allow(non_snake_case)]
    Refs (referrer, reference) {
        referrer -> Integer,
        reference -> Integer,
    }
}

table! {
    #[allow(non_snake_case)]
    ValidPaths {
        id -> Integer,
        path -> Text,
        hash -> Text,
        registrationTime -> Integer,
        deriver -> Nullable<Text>,
        narSize -> Nullable<Integer>,
        ultimate -> Nullable<Integer>,
        sigs -> Nullable<Text>,
        ca -> Nullable<Text>,
    }
}

allow_tables_to_appear_in_same_query!(Refs, ValidPaths);

pub struct SystemDatabase(SqliteConnection);

impl SystemDatabase {
    pub const PATH: &'static str = "/nix/var/nix/db/db.sqlite";

    pub fn open() -> Result<Self> {
        let immutable_conn = format!("file:{}?mode=ro&immutable=1", Self::PATH);

        // TODO: only try opening immutably if/when https://github.com/diesel-rs/diesel/pull/1292 is merged
        match SqliteConnection::establish(&immutable_conn) {
            Ok(conn) => Ok(Self(conn)),
            Err(_) => {
                ensure!(is_root_user(), err::DBNeedsRoot);
                let conn = SqliteConnection::establish(Self::PATH)?;
                Ok(Self(conn))
            }
        }
    }

    #[inline(always)]
    pub fn conn(&self) -> &SqliteConnection {
        &self.0
    }
}

fn is_root_user() -> bool {
    unsafe { libc::getuid() == 0 }
}
