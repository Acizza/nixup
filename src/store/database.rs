use anyhow::{anyhow, Context, Result};
use diesel::prelude::*;

pub mod schema {
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
}

pub struct SystemDatabase(SqliteConnection);

impl SystemDatabase {
    pub const PATH: &'static str = "/nix/var/nix/db/db.sqlite";

    pub fn open() -> Result<Self> {
        let immutable_conn = format!("file:{}?mode=ro&immutable=1", Self::PATH);

        // TODO: only try opening immutably if/when https://github.com/diesel-rs/diesel/pull/1292 is merged
        match SqliteConnection::establish(&immutable_conn) {
            Ok(conn) => Ok(Self(conn)),
            Err(_) => {
                if !is_root_user() {
                    return Err(anyhow!("must run program as root to access the Nix database\nto avoid needing root access, compile SQLite with SQLITE_USE_URI=1"));
                }

                let conn = SqliteConnection::establish(Self::PATH)
                    .context("failed to establish SQLite connection to nix database")?;

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
