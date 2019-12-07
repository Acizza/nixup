pub mod diff;

use crate::err::Result;
use rusqlite::params;
use serde_derive::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

#[derive(Debug, Eq, Serialize, Deserialize)]
pub struct Store {
    /// The store's unique id.
    /// Note that this cannot be used to identify a store persisently.
    pub id: u32,
    /// The store's name.
    pub name: String,
    /// The store's version.
    pub version: String,
    /// The suffix of the store's name.
    /// This can either be the derivation's output type, or a special variant of the store.
    pub suffix: Option<String>,
    /// The epoch time the store was registered on the system.
    pub register_time: u32,
}

impl Store {
    pub fn parse<P>(id: u32, register_time: u32, path: P) -> Option<Self>
    where
        P: Into<String>,
    {
        let path = path.into();

        let stripped = Self::strip(path)?;
        let mut split_sep = stripped.split('-').collect::<SmallVec<[&str; 4]>>();

        match split_sep.len() {
            0 | 1 => return None,
            2 => {
                if !split_sep[1].chars().any(char::is_numeric) {
                    return None;
                }

                let version = split_sep.swap_remove(1).into();
                let name = split_sep.swap_remove(0).into();

                return Some(Self {
                    id,
                    register_time,
                    name,
                    version,
                    suffix: None,
                });
            }
            _ => (),
        }

        let suffix = {
            let end = split_sep.len() - 1;

            if split_sep[end].chars().all(char::is_alphabetic) {
                Some(split_sep.swap_remove(end).to_string())
            } else {
                None
            }
        };

        let version = {
            let pos = split_sep.iter().position(|&s| Self::is_version_str(s))?;
            let ver_str = split_sep[pos..].join("-");

            unsafe {
                split_sep.set_len(pos);
            }

            ver_str
        };

        let name = split_sep.join("-");

        Some(Self {
            id,
            register_time,
            name,
            version,
            suffix,
        })
    }

    fn is_version_str(string: &str) -> bool {
        fn is_digit(b: u8) -> bool {
            b >= b'0' && b <= b'9'
        }

        if string.is_empty() {
            return false;
        }

        let mut bytes = string.as_bytes();

        match bytes[0] {
            b'v' => {
                if bytes.len() < 2 || !is_digit(bytes[1]) {
                    return false;
                }

                bytes = &bytes[1..];
            }
            b => {
                if !is_digit(b) {
                    return false;
                }
            }
        }

        bytes.iter().all(|&c| match c {
            c if is_digit(c) => true,
            b'.' | b'a'..=b'z' | b'_' => true,
            _ => false,
        })
    }

    pub fn strip<P>(path: P) -> Option<String>
    where
        P: AsRef<str> + Into<String>,
    {
        let path_ref = path.as_ref();

        match path_ref.find('-') {
            Some(idx) if path_ref.len() <= idx + 1 => None,
            Some(idx) => {
                let mut path = path.into();
                path.replace_range(..=idx, "");
                Some(path)
            }
            None => None,
        }
    }

    pub fn all_from_system(db: &SystemDatabase) -> Result<HashSet<Self>> {
        let mut stmt = db
            .conn()
            .prepare(include_str!("../../sql/select_system_stores.sql"))?;

        Self::unique_from_query(rusqlite::NO_PARAMS, &mut stmt)
    }

    /// Runs the specified SQL statement (`stmt`) and attemps to parse each returned row into a unique `Store`.
    /// Note that the query must select the store's ID, path, and registration time, in that order.
    ///
    /// See the documentation of the `get_unique` function for more information on what is considered a unique `Store`.
    fn unique_from_query<P>(params: P, stmt: &mut rusqlite::Statement) -> Result<HashSet<Self>>
    where
        P: IntoIterator,
        P::Item: rusqlite::ToSql,
    {
        let store_queries = stmt
            .query_map(params, |row| {
                let id = row.get(0)?;
                let path: String = row.get(1)?;
                let register_time: u32 = row.get(2)?;
                let store = Store::parse(id, register_time, path);
                Ok(store)
            })?
            .filter_map(|row| row.ok().and_then(|store| store));

        let stores = Self::get_unique(store_queries);
        Ok(stores)
    }

    /// Returns a new `HashSet` containing `Store`'s that are not considered to have duplicates.
    ///
    /// A `Store` that has different versions that were registered on the system within an hour
    /// of each other is considered to be a duplicate.
    ///
    /// Only filtering stores that were registered on the system within an hour of each other reduces
    /// false positives, as it likely means that the differing versions are from the same system update,
    /// rather than a separate one. We only want to filter out stores with differing versions from the same
    /// system update since there isn't a way to persistently identify a store across updates outside of its name.
    fn get_unique(stores: impl Iterator<Item = Self>) -> HashSet<Self> {
        let mut unique = HashSet::<Store>::new();
        let mut duplicates = HashSet::new();

        for store in stores {
            if duplicates.contains(&store.name) {
                continue;
            }

            if let Some(existing) = unique.get(&store) {
                let newer_reg_time = existing.register_time.max(store.register_time);
                let older_reg_time = existing.register_time.min(store.register_time);

                if newer_reg_time - older_reg_time < 3600 && existing.version != store.version {
                    unique.remove(&store);
                    duplicates.insert(store.name);
                }

                continue;
            }

            unique.insert(store);
        }

        unique
    }
}

impl Hash for Store {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Store {
    fn eq(&self, other: &Store) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub store: Store,
    pub deps: HashSet<Store>,
}

impl Derivation {
    pub fn all_from_stores(stores: HashSet<Store>, db: &SystemDatabase) -> Result<HashSet<Self>> {
        let mut stmt = db
            .conn()
            .prepare(include_str!("../../sql/select_store_deps.sql"))?;

        let mut packages = HashSet::with_capacity(stores.len());

        for store in stores {
            let deps = Store::unique_from_query(params![store.id], &mut stmt)?;
            packages.insert(Self { store, deps });
        }

        Ok(packages)
    }

    pub fn all_from_system(db: &SystemDatabase) -> Result<HashSet<Self>> {
        let stores = Store::all_from_system(db)?;
        Derivation::all_from_stores(stores, db)
    }
}

impl Hash for Derivation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.store.name.hash(state);
    }
}

impl PartialEq for Derivation {
    fn eq(&self, other: &Derivation) -> bool {
        self.store.name == other.store.name
    }
}

pub struct SystemDatabase(rusqlite::Connection);

impl SystemDatabase {
    pub const PATH: &'static str = "/nix/var/nix/db/db.sqlite";

    pub fn open() -> Result<Self> {
        use rusqlite::{Connection, OpenFlags};
        let conn = Connection::open_with_flags(Self::PATH, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self(conn))
    }

    #[inline(always)]
    fn conn(&self) -> &rusqlite::Connection {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! store_tuple {
        ($path:expr => $name:expr, $version:expr, $suffix:expr) => {
            (
                $path,
                Some(Store {
                    id: 0,
                    register_time: 0,
                    name: $name.into(),
                    version: $version.into(),
                    suffix: $suffix,
                }),
            )
        };

        ($path:expr) => {
            ($path, None)
        };
    }

    #[test]
    fn parse_store_info() {
        let stores = [
            store_tuple!("/nix/store/123abc-fix-static.patch"),
            store_tuple!("/nix/store/123abc-some-deriv.drv"),
            store_tuple!("/nix/store/123abc-glxinfo-8.4.0" => "glxinfo", "8.4.0", None),
            store_tuple!("/nix/store/123abc-pcre-8.42" => "pcre", "8.42", None),
            store_tuple!("/nix/store/123abc-dxvk-v1.4.6" => "dxvk", "v1.4.6", None),
            store_tuple!("/nix/store/123abc-dxvk-c47095a8dcfa4c376d8e9c4276865b7f298137d8" => "dxvk", "c47095a8dcfa4c376d8e9c4276865b7f298137d8", None),
            store_tuple!("/nix/store/123abc-rpcs3-9165-8ca53f9" => "rpcs3", "9165-8ca53f9", None),
            store_tuple!("/nix/store/123abc-single-version-8" => "single-version", "8", None),
            store_tuple!("/nix/store/123abc-single-4" => "single", "4", None),
            store_tuple!("/nix/store/123abc-wine-wow-4.21-staging" => "wine-wow", "4.21", Some("staging".into())),
            store_tuple!("/nix/store/123abc-wine-wow-4.0-rc5-staging" => "wine-wow", "4.0-rc5", Some("staging".into())),
            store_tuple!("/nix/store/123abc-ffmpeg-3.4.5-bin" => "ffmpeg", "3.4.5", Some("bin".into())),
            store_tuple!("/nix/store/123abc-vulkan-loader-1.1.85" => "vulkan-loader", "1.1.85", None),
            store_tuple!("/nix/store/123abc-vpnc-0.5.3-post-r550" => "vpnc", "0.5.3-post-r550", None),
        ];

        for (path, expected_store) in &stores {
            match Store::parse(0, 0, *path) {
                Some(parsed) => match expected_store {
                    Some(expected) => {
                        assert_eq!(expected.name, parsed.name, "name mismatch");
                        assert_eq!(expected.version, parsed.version, "version mismatch");
                        assert_eq!(expected.suffix, parsed.suffix, "suffix mismatch");
                    }
                    None => panic!(
                        "{} was parsed when no result was expected: {:?}",
                        path, parsed
                    ),
                },
                None if expected_store.is_some() => panic!("{} failed to be parsed", path),
                None => (),
            }
        }
    }

    #[test]
    fn strip_store_path() {
        let store = "/nix/store/03lp4drizbh8cl3f9mjysrrzrg3ssakv-glxinfo-8.4.0";
        assert_eq!(Store::strip(store), Some("glxinfo-8.4.0".into()));
    }
}
