use crate::err::Result;
use diesel::prelude::*;

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
        // TODO: open with the following URI parameters when https://github.com/diesel-rs/diesel/pull/1292
        // is merged:
        // mode=ro
        // immutable=1
        let conn = SqliteConnection::establish(Self::PATH)?;
        Ok(Self(conn))
    }

    #[inline(always)]
    pub fn conn(&self) -> &SqliteConnection {
        &self.0
    }
}
