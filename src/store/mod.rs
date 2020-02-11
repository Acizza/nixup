pub mod database;
pub mod diff;

use crate::err::Result;
use database::SystemDatabase;
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
        P: AsRef<str>,
    {
        const DELIMETER: u8 = b'-';

        let path = Self::strip_prefix(path.as_ref().as_bytes())?;

        // Get all of the indices for our delimeter
        let fragments = path
            .iter()
            .enumerate()
            .filter_map(|(i, &byte)| if byte == DELIMETER { Some(i) } else { None })
            .collect::<SmallVec<[usize; 4]>>();

        match fragments.len() {
            0 => return None,
            // Only having one delimiter is usually indicative of a "{name}-{version}" format, so we can
            // take a fast path here
            1 => {
                let version = &path[fragments[0] + 1..];

                if !version.iter().any(|b| b.is_ascii_digit()) {
                    return None;
                }

                let name = &path[..fragments[0]];

                // This is safe because we aren't modifying the path that we received,
                // and we received the path as a &str
                let store = unsafe {
                    Self {
                        id,
                        register_time,
                        name: String::from_utf8_unchecked(name.into()),
                        version: String::from_utf8_unchecked(version.into()),
                        suffix: None,
                    }
                };

                return Some(store);
            }
            _ => (),
        }

        // The suffix is the last fragment if it does not contain any numbers
        let (suffix, suffix_start) = {
            let last_frag = fragments[fragments.len() - 1];
            let slice = &path[last_frag + 1..];

            if !slice.iter().any(u8::is_ascii_digit) {
                (Some(slice), last_frag)
            } else {
                (None, path.len())
            }
        };

        // The version will be all fragments that match `is_version_str`
        let (version, version_start) = {
            let mut version = None;
            let mut version_start = 0;
            let mut frag_iter = fragments.iter().peekable();

            while let Some(&fragment) = frag_iter.next() {
                // We need to check for a version string on a per-fragment basis, as
                // `is_version_str` will disqualify our fragment character
                let slice = match frag_iter.peek() {
                    Some(&&next_frag) => &path[fragment + 1..next_frag],
                    None => &path[fragment + 1..],
                };

                if !Self::is_version_str(slice) {
                    continue;
                }

                version = Some(&path[fragment + 1..suffix_start]);
                version_start = fragment;
                break;
            }

            (version?, version_start)
        };

        // This is safe because we aren't modifying the path that we received,
        // and we received the path as a &str
        let store = unsafe {
            Self {
                id,
                register_time,
                name: String::from_utf8_unchecked(path[..version_start].into()),
                version: String::from_utf8_unchecked(version.into()),
                suffix: suffix.map(|sfx| String::from_utf8_unchecked(sfx.into())),
            }
        };

        Some(store)
    }

    fn is_version_str(mut bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        match bytes[0] {
            b'v' => {
                if bytes.len() < 2 || !bytes[1].is_ascii_digit() {
                    return false;
                }

                bytes = &bytes[1..];
            }
            b if !b.is_ascii_digit() => return false,
            _ => (),
        }

        bytes.iter().all(|c| match c {
            c if c.is_ascii_digit() => true,
            b'.' | b'a'..=b'z' | b'_' => true,
            _ => false,
        })
    }

    pub fn strip_prefix(bytes: &[u8]) -> Option<&[u8]> {
        const PREFIX_LEN: usize = "/nix/store/zzw3mjv8dcmrz4ran92pnyj97f05ff55-".len();
        const DASH_POS: usize = PREFIX_LEN - 1;

        // Every store starts with "/nix/store/{sha256 hash}-", so we can simply assume where
        // the end of the prefix is
        if bytes.len() > PREFIX_LEN && bytes[DASH_POS] == b'-' {
            return Some(&bytes[PREFIX_LEN..]);
        }

        // Even though every store should have hit the fast path above, we'll use a fallback
        // just in case the store path prefix is changed in the future
        let pos = bytes.iter().position(|b| *b == b'-')?;

        if bytes.len() <= pos + 1 {
            return None;
        }

        Some(&bytes[pos + 1..])
    }

    pub fn all_from_system(db: &SystemDatabase) -> Result<HashSet<Self>> {
        use database::schema::ValidPaths::dsl::*;
        use diesel::prelude::*;

        let stores = ValidPaths
            .filter(ca.is_null())
            .filter(path.not_like("%-completions"))
            .filter(path.not_like("%.tar.%"))
            .select((id, path, registrationTime))
            .order(registrationTime.desc())
            .get_results::<(i32, String, i32)>(db.conn())?
            .into_iter()
            .filter_map(|(store_id, store_path, reg)| {
                Store::parse(store_id as u32, reg as u32, store_path)
            });

        let unique = Self::get_unique(stores);

        Ok(unique)
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
        use database::schema::{Refs::dsl::*, ValidPaths::dsl::*};
        use diesel::prelude::*;

        let mut packages = HashSet::with_capacity(stores.len());

        db.conn().transaction::<_, diesel::result::Error, _>(|| {
            for store in stores {
                let is_dependency =
                    id.eq_any(Refs.filter(referrer.eq(store.id as i32)).select(reference));

                let all_deps = ValidPaths
                    .filter(ca.is_null())
                    .filter(id.ne(store.id as i32))
                    .filter(is_dependency)
                    .select((id, path, registrationTime))
                    .order(registrationTime.desc())
                    .get_results::<(i32, String, i32)>(db.conn())?
                    .into_iter()
                    .filter_map(|(store_id, store_path, reg)| {
                        Store::parse(store_id as u32, reg as u32, store_path)
                    });

                let deps = Store::get_unique(all_deps);
                packages.insert(Self { store, deps });
            }

            Ok(())
        })?;

        Ok(packages)
    }

    pub fn all_from_system(db: &SystemDatabase) -> Result<HashSet<Self>> {
        let stores = Store::all_from_system(db)?;
        Self::all_from_stores(stores, db)
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
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-fix-static.patch"),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-some-deriv.drv"),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-dash-edge-case-"),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-dash-short-"),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-"),
            store_tuple!("/nix/store/123shortprefix-short-prefix-1.0" => "short-prefix", "1.0", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-glxinfo-8.4.0" => "glxinfo", "8.4.0", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-pcre-8.42" => "pcre", "8.42", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-dxvk-v1.4.6" => "dxvk", "v1.4.6", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-dxvk-c47095a8dcfa4c376d8e9c4276865b7f298137d8" => "dxvk", "c47095a8dcfa4c376d8e9c4276865b7f298137d8", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-rpcs3-9165-8ca53f9" => "rpcs3", "9165-8ca53f9", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-single-version-8" => "single-version", "8", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-single-4" => "single", "4", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-wine-wow-4.21-staging" => "wine-wow", "4.21", Some("staging".into())),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-wine-wow-4.0-rc5-staging" => "wine-wow", "4.0-rc5", Some("staging".into())),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-ffmpeg-3.4.5-bin" => "ffmpeg", "3.4.5", Some("bin".into())),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-vulkan-loader-1.1.85" => "vulkan-loader", "1.1.85", None),
            store_tuple!("/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-vpnc-0.5.3-post-r550" => "vpnc", "0.5.3-post-r550", None),
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
        let store = "/nix/store/03lp4drizbh8cl3f9mjysrrzrg3ssakv-glxinfo-8.4.0".as_bytes();
        assert_eq!(
            Store::strip_prefix(store),
            Some("glxinfo-8.4.0".as_bytes()),
            "normal store"
        );

        let dash_edge_case = "/nix/store/zx6vs1b6xf07cprslk9is1fhwih21ix5-".as_bytes();
        assert_eq!(Store::strip_prefix(dash_edge_case), None, "dash edge case");
    }
}
