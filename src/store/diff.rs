use super::{StorePath, StorePathMap, SystemPackageMap};

#[derive(Debug)]
pub struct StoreDiff {
    pub name: String,
    pub ver_from: String,
    pub ver_to: String,
}

impl StoreDiff {
    pub fn from_store(new: &StorePath, old: &StorePath) -> Option<StoreDiff> {
        if new.version == old.version {
            return None;
        }

        let diff = StoreDiff {
            name: new.name.clone(),
            ver_from: old.version.clone(),
            ver_to: new.version.clone(),
        };

        Some(diff)
    }

    pub fn from_store_list(new_stores: &StorePathMap, old_stores: &StorePathMap) -> Vec<StoreDiff> {
        let mut diffs = Vec::new();

        for new in new_stores {
            let old = match old_stores.get(&new.name) {
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

pub fn get_package_diffs(new: &SystemPackageMap, old: &SystemPackageMap) -> Vec<PackageDiff> {
    let mut diffs = Vec::new();

    for new_pkg in new.values() {
        let old_pkg = match old.get(&new_pkg.path.name) {
            Some(old_pkg) => old_pkg,
            None => continue,
        };

        let pkg_diff = StoreDiff::from_store(&new_pkg.path, &old_pkg.path);
        let dep_diffs = StoreDiff::from_store_list(&new_pkg.deps, &old_pkg.deps);

        if pkg_diff.is_none() && dep_diffs.is_empty() {
            continue;
        }

        let diff = PackageDiff {
            name: new_pkg.path.name.clone(),
            pkg: pkg_diff,
            deps: dep_diffs,
        };

        diffs.push(diff);
    }

    diffs
}

#[cfg(test)]
mod test {
    use super::super::test::{mkstore, mksyspkg};
    use super::*;

    fn mkstorediff<S>(name: S, from: S, to: S) -> StoreDiff
    where
        S: Into<String>,
    {
        StoreDiff {
            name: name.into(),
            ver_from: from.into(),
            ver_to: to.into(),
        }
    }

    #[test]
    fn detect_store_diffs() {
        let new_stores = vec![
            mkstore("glxinfo", "8.5.0"),
            mkstore("ffmpeg", "3.4.5"),
            mkstore("wine-wow|staging", "4.1"),
            mkstore("steam-runtime", "2019-02-15"),
            mkstore("dxvk", "v0.96"),
        ]
        .into_iter()
        .collect::<StorePathMap>();

        let old_stores = vec![
            mkstore("glxinfo", "8.4.0"),
            mkstore("ffmpeg", "3.4.5"),
            mkstore("wine-wow|staging", "4.0-rc5"),
            mkstore("steam-runtime", "2016-08-26"),
            mkstore("dxvk", "v0.96"),
        ]
        .into_iter()
        .collect::<StorePathMap>();

        let expected_diffs = vec![
            mkstorediff("glxinfo", "8.4.0", "8.5.0"),
            mkstorediff("wine-wow|staging", "4.0-rc5", "4.1"),
            mkstorediff("steam-runtime", "2016-08-26", "2019-02-15"),
        ];

        let diffs = StoreDiff::from_store_list(&new_stores, &old_stores);

        assert!(
            diffs.len() == expected_diffs.len(),
            "actual number of diffs does not match expected"
        );

        for diff in diffs {
            let expected = expected_diffs
                .iter()
                .find(|&x| x == &diff)
                .expect(&format!("expected diff not found! {:?}", diff.name));

            assert_eq!(diff.ver_from, expected.ver_from, "from version mismatch");
            assert_eq!(diff.ver_to, expected.ver_to, "to version mismatch");
        }
    }

    #[test]
    fn separate_global_deps() {
        let pkgs = vec![
            mksyspkg(
                mkstore("test1", "1"),
                vec![mkstore("db", "4.8.30"), mkstore("glibc", "2.27")]
                    .into_iter()
                    .collect::<StorePathMap>(),
            ),
            mksyspkg(
                mkstore("test2", "1"),
                vec![mkstore("db", "5.0.0"), mkstore("glibc", "2.27")]
                    .into_iter()
                    .collect::<StorePathMap>(),
            ),
            mksyspkg(
                mkstore("test3", "1"),
                vec![mkstore("db", "4.8.30"), mkstore("glibc", "2.27")]
                    .into_iter()
                    .collect::<StorePathMap>(),
            ),
        ]
        .into_iter()
        .map(|x| (x.path.name.clone(), x))
        .collect::<SystemPackageMap>();

        let expected_pkg_dep = [mkstore("db", "4.8.30"), mkstore("db", "5.0.0")];

        for pkg in pkgs.values() {
            let found = pkg.deps.iter().find(|pkg_dep| {
                for dep in &expected_pkg_dep {
                    if dep.name == pkg_dep.name && dep.version == pkg_dep.version {
                        return true;
                    }
                }

                false
            });

            assert!(
                found.is_some(),
                "failed to find package-specific dependency for {}",
                pkg.path.name
            );
        }
    }
}
