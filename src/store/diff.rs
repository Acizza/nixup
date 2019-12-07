use super::{Derivation, Store};
use std::collections::HashSet;

#[derive(Debug)]
pub struct StoreDiff {
    pub name: String,
    pub suffix: Option<String>,
    pub ver_from: String,
    pub ver_to: String,
}

impl StoreDiff {
    pub fn from_store(new: &Store, old: &Store) -> Option<StoreDiff> {
        if new.version == old.version {
            return None;
        }

        // We only want stores with the same suffix
        match (&new.suffix, &old.suffix) {
            (Some(new_suffix), Some(old_suffix)) => {
                if new_suffix != old_suffix {
                    return None;
                }
            }
            (Some(_), None) | (None, Some(_)) => return None,
            (None, None) => (),
        }

        let diff = StoreDiff {
            name: new.name.clone(),
            suffix: new.suffix.clone(),
            ver_from: old.version.clone(),
            ver_to: new.version.clone(),
        };

        Some(diff)
    }

    pub fn from_store_list(
        new_stores: &HashSet<Store>,
        old_stores: &HashSet<Store>,
    ) -> Vec<StoreDiff> {
        let mut diffs = Vec::new();

        for new in new_stores {
            let old = match old_stores.get(&new) {
                Some(old) => old,
                None => continue,
            };

            let diff = match StoreDiff::from_store(new, old) {
                Some(diff) => diff,
                None => continue,
            };

            diffs.push(diff);
        }

        diffs
    }
}

impl PartialEq for StoreDiff {
    fn eq(&self, other: &StoreDiff) -> bool {
        self.name == other.name
    }
}

#[derive(Debug)]
pub struct PackageDiff {
    pub name: String,
    pub pkg: Option<StoreDiff>,
    pub deps: Vec<StoreDiff>,
}

pub fn get_package_diffs(new: &HashSet<Derivation>, old: &HashSet<Derivation>) -> Vec<PackageDiff> {
    let mut diffs = Vec::new();

    for new_pkg in new {
        let old_pkg = match old.get(&new_pkg) {
            Some(old_pkg) => old_pkg,
            None => continue,
        };

        let pkg_diff = StoreDiff::from_store(&new_pkg.store, &old_pkg.store);
        let dep_diffs = StoreDiff::from_store_list(&new_pkg.deps, &old_pkg.deps);

        if pkg_diff.is_none() && dep_diffs.is_empty() {
            continue;
        }

        let diff = PackageDiff {
            name: new_pkg.store.name.clone(),
            pkg: pkg_diff,
            deps: dep_diffs,
        };

        diffs.push(diff);
    }

    diffs
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! store {
        ($name:expr, $version:expr, $suffix:expr) => {
            Store {
                id: 0,
                register_time: 0,
                name: $name.into(),
                version: $version.into(),
                suffix: $suffix,
            }
        };
    }

    macro_rules! diff {
        ($name:expr, $ver_from:expr, $ver_to:expr) => {
            StoreDiff {
                name: $name.into(),
                suffix: None,
                ver_from: $ver_from.into(),
                ver_to: $ver_to.into(),
            }
        };
    }

    #[test]
    fn detect_store_diffs() {
        let new_stores = vec![
            store!("glxinfo", "8.5.0", None),
            store!("ffmpeg", "3.4.5", None),
            store!("wine-wow", "4.1", Some("staging".into())),
            store!("steam-runtime", "2019-02-15", None),
            store!("dxvk", "v0.96", None),
            store!("diff-suffix", "3.4.6", Some("bin".into())),
            store!("same-suffix", "1.0.1", Some("bin".into())),
            store!("partial-suffix", "1.0.1", None),
        ]
        .into_iter()
        .collect::<HashSet<Store>>();

        let old_stores = vec![
            store!("glxinfo", "8.4.0", None),
            store!("ffmpeg", "3.4.5", None),
            store!("wine-wow", "4.0-rc5", Some("staging".into())),
            store!("steam-runtime", "2016-08-26", None),
            store!("dxvk", "v0.96", None),
            store!("diff-suffix", "3.4.5", Some("out".into())),
            store!("same-suffix", "1.0.0", Some("bin".into())),
            store!("partial-suffix", "1.0.0", Some("bin".into())),
        ]
        .into_iter()
        .collect::<HashSet<Store>>();

        let expected_diffs = vec![
            diff!("glxinfo", "8.4.0", "8.5.0"),
            diff!("wine-wow", "4.0-rc5", "4.1"),
            diff!("steam-runtime", "2016-08-26", "2019-02-15"),
            diff!("same-suffix", "1.0.0", "1.0.1"),
        ];

        let diffs = StoreDiff::from_store_list(&new_stores, &old_stores);

        assert!(
            diffs.len() == expected_diffs.len(),
            "got {} diffs, expected {}:\n\texpected: {:?}\n\n\tgot: {:?}",
            diffs.len(),
            expected_diffs.len(),
            expected_diffs,
            diffs
        );

        for diff in diffs {
            let expected = expected_diffs
                .iter()
                .find(|&x| x == &diff)
                .expect(&format!("expected diff not found: {}", diff.name));

            assert_eq!(diff.ver_from, expected.ver_from, "old version mismatch");
            assert_eq!(diff.ver_to, expected.ver_to, "new version mismatch");
        }
    }
}
