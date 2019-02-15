use crate::error::StoreError;
use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;

pub type Name = String;

#[derive(Serialize, Deserialize, Clone, Debug, Eq)]
pub struct StorePath {
    #[serde(skip)]
    pub path: Option<PathBuf>,
    pub name: Name,
    pub version: String,
}

impl Hash for StorePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for StorePath {
    fn eq(&self, other: &StorePath) -> bool {
        self.name == other.name
    }
}

impl Borrow<Name> for StorePath {
    fn borrow(&self) -> &Name {
        &self.name
    }
}

pub type StorePathMap = HashSet<StorePath>;

impl StorePath {
    pub fn parse<P>(path: P) -> Option<StorePath>
    where
        P: AsRef<str>,
    {
        let path = path.as_ref();

        let stripped = StorePath::strip(path)?;
        let mut split_sep = stripped.split('-').collect::<SmallVec<[&str; 4]>>();

        match split_sep.len() {
            0 | 1 => return None,
            2 => {
                if !StorePath::is_version_str(split_sep[1]) {
                    return None;
                }

                let version = split_sep.swap_remove(1).into();
                let name = split_sep.swap_remove(0).into();

                let store = StorePath {
                    path: Some(path.into()),
                    name,
                    version,
                };

                return Some(store);
            }
            _ => (),
        }

        let suffix = {
            let end = split_sep.len() - 1;

            if split_sep[end].chars().all(char::is_alphabetic) {
                Some(split_sep.swap_remove(end))
            } else {
                None
            }
        };

        let version = {
            let pos = split_sep
                .iter()
                .position(|&s| StorePath::is_version_str(s))?;

            let ver_str = split_sep[pos..].join("-");

            unsafe {
                split_sep.set_len(pos);
            }

            ver_str
        };

        let mut name = split_sep.join("-");

        if let Some(sfx) = suffix {
            name.reserve(1 + sfx.len());
            name.push('|');
            name.push_str(sfx);
        }

        let store = StorePath {
            path: Some(path.into()),
            name,
            version,
        };

        Some(store)
    }

    fn is_version_str(string: &str) -> bool {
        if !string.starts_with(|c| char::is_numeric(c) || c == 'v') {
            return false;
        }

        string
            .chars()
            .all(|c| c.is_numeric() || c == '.' || (c >= 'a' && c <= 'z') || c == '_')
    }

    pub fn strip<P>(path: P) -> Option<String>
    where
        P: Into<String>,
    {
        let mut path = path.into();

        match path.find('-') {
            Some(idx) if path.len() <= idx + 1 => return None,
            Some(idx) => path.replace_range(..=idx, ""),
            None => return None,
        }

        Some(path)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemPackage {
    pub path: StorePath,
    pub deps: StorePathMap,
}

pub type SystemPackageMap = HashMap<Name, SystemPackage>;

impl SystemPackage {
    pub fn from_path<P>(path: P) -> SystemPackage
    where
        P: Into<StorePath>,
    {
        SystemPackage {
            path: path.into(),
            deps: HashSet::new(),
        }
    }

    pub fn with_deps<P>(path: P) -> Result<SystemPackage, StoreError>
    where
        P: Into<StorePath>,
    {
        let mut package = SystemPackage::from_path(path);
        package.parse_deps()?;

        Ok(package)
    }

    pub fn parse_deps(&mut self) -> Result<(), StoreError> {
        let path = match &self.path.path {
            Some(path) => path,
            None => return Ok(()),
        };

        self.deps.clear();

        let mut cmd = Command::new("nix-store");
        cmd.arg("-qR");
        cmd.arg(path);

        let output = {
            let output = cmd.output()?;
            let mut content = String::from_utf8(output.stdout)?;

            // The last line contains the current package, so strip it from the output
            if let Some(idx) = content.rfind("/nix/") {
                content.replace_range(idx.., "");
            }

            content
        };

        for raw_path in output.lines() {
            let path = match StorePath::parse(raw_path) {
                Some(path) => path,
                None => continue,
            };

            self.deps.insert(path);
        }

        Ok(())
    }
}

pub fn parse_system_stores() -> Result<StorePathMap, StoreError> {
    let mut cmd = Command::new("nixos-option");
    cmd.arg("environment.systemPackages");

    let output = {
        let output = cmd.output()?;
        let mut content = String::from_utf8(output.stdout)?;

        match (content.find("[ "), content.find(']')) {
            (Some(start), Some(end)) => {
                content.replace_range(end.., "");
                content.replace_range(..start + "[ ".len(), "");
                content
            }
            _ => return Err(StoreError::MalformedOutput),
        }
    };

    lazy_static! {
        static ref MATCHER: Regex = Regex::new("\"(.+?)\"").unwrap();
    }

    let mut stores = HashSet::<StorePath>::new();

    for split in output.split_whitespace() {
        let caps = match MATCHER.captures(&split) {
            Some(caps) => caps,
            None => continue,
        };

        let store = match StorePath::parse(&caps[1]) {
            Some(store) => store,
            None => continue,
        };

        // We only want to use the latest version of the store
        if let Some(existing) = stores.get(&store) {
            if existing.version > store.version {
                continue;
            }
        }

        stores.insert(store);
    }

    Ok(stores)
}

pub fn parse_system_packages() -> Result<SystemPackageMap, StoreError> {
    let stores = parse_system_stores()?;
    let mut packages = HashMap::with_capacity(stores.len());

    packages.par_extend(stores.into_par_iter().filter_map(|store| {
        let name = store.name.clone();
        let pkg = SystemPackage::with_deps(store).ok()?;

        Some((name, pkg))
    }));

    Ok(packages)
}

#[derive(Debug)]
pub struct StoreDiff {
    pub name: String,
    pub ver_from: String,
    pub ver_to: String,
}

pub fn get_store_diff(new: &StorePath, old: &StorePath) -> Option<StoreDiff> {
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

pub fn get_store_diffs(new_stores: &StorePathMap, old_stores: &StorePathMap) -> Vec<StoreDiff> {
    let mut diffs = Vec::new();

    for new in new_stores {
        let old = match old_stores.get(&new.name) {
            Some(old) => old,
            None => continue,
        };

        let diff = match get_store_diff(new, old) {
            Some(diff) => diff,
            None => continue,
        };

        diffs.push(diff);
    }

    diffs
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

        let pkg_diff = get_store_diff(&new_pkg.path, &old_pkg.path);
        let dep_diffs = get_store_diffs(&new_pkg.deps, &old_pkg.deps);

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

struct DependencyScan<'a> {
    last_version: &'a str,
    has_multiple_versions: bool,
}

impl<'a> DependencyScan<'a> {
    fn new(last_version: &'a str, has_multiple_versions: bool) -> DependencyScan {
        DependencyScan {
            last_version,
            has_multiple_versions,
        }
    }
}

pub fn isolate_global_dependencies(
    pkgs: &mut SystemPackageMap,
) -> Result<StorePathMap, StoreError> {
    let mut ver_tracker = HashMap::<&str, DependencyScan>::new();

    for pkg in pkgs.values_mut() {
        for dep in &pkg.deps {
            match ver_tracker.entry(&dep.name) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();

                    if dep.version != entry.last_version {
                        entry.has_multiple_versions = true;
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(DependencyScan::new(&dep.version, false));
                }
            }
        }
    }

    let dep_names = ver_tracker
        .into_iter()
        .filter_map(|(n, d)| {
            if d.has_multiple_versions {
                None
            } else {
                Some(n.to_string())
            }
        })
        .collect::<SmallVec<[String; 8]>>();

    let global_deps = pkgs
        .par_iter_mut()
        .fold(HashSet::new, |mut acc, (_, pkg)| {
            for name in &dep_names {
                if let Some(dep) = pkg.deps.take(name) {
                    acc.insert(dep);
                }
            }

            acc
        })
        .reduce(HashSet::new, |mut acc, x| {
            acc.extend(x);
            acc
        });

    Ok(global_deps)
}

#[cfg(test)]
mod test {
    use super::*;

    fn mkpath<S>(name: S, ver: S) -> Option<StorePath>
    where
        S: Into<String>,
    {
        Some(StorePath {
            path: None,
            name: name.into(),
            version: ver.into(),
        })
    }

    #[test]
    fn parse_store_info() {
        let paths = [
            (
                "/nix/store/123abc-glxinfo-8.4.0",
                mkpath("glxinfo", "8.4.0"),
            ),
            ("/nix/store/123abc-fix-static.patch", None),
            (
                "/nix/store/123abc-nix-wallpaper-simple-dark-gray_bottom.png.drv",
                None,
            ),
            ("/nix/store/123abc-pcre-8.42", mkpath("pcre", "8.42")),
            ("/nix/store/123abc-dxvk-v0.96", mkpath("dxvk", "v0.96")),
            (
                "/nix/store/123abc-dxvk-6062dfbef4d5c0f061b9f6e342acab54f34e089a",
                mkpath("dxvk", "6062dfbef4d5c0f061b9f6e342acab54f34e089a"),
            ),
            (
                "/nix/store/123abc-rpcs3-7788-4c59395",
                mkpath("rpcs3", "7788-4c59395"),
            ),
            ("/nix/store/123abc-gcc-7.4.0", mkpath("gcc", "7.4.0")),
            (
                "/nix/store/123abc-steam-runtime-2016-08-26",
                mkpath("steam-runtime", "2016-08-26"),
            ),
            (
                "/nix/store/123abc-wine-wow-4.0-rc5-staging",
                mkpath("wine-wow|staging", "4.0-rc5"),
            ),
            (
                "/nix/store/123abc-ffmpeg-3.4.5-bin",
                mkpath("ffmpeg|bin", "3.4.5"),
            ),
        ];

        for (full, path) in &paths {
            match StorePath::parse(full) {
                Some(result) => match path {
                    Some(p) => {
                        assert_eq!(result.name, p.name, "name mismatch");
                        assert_eq!(result.version, p.version, "version mismatch");
                    }
                    None => assert!(false, "{} was parsed when no result was expected", full),
                },
                None if path.is_some() => assert!(false, "{} failed to be parsed", full),
                None => (),
            }
        }
    }

    #[test]
    fn strip_store_path() {
        let store = "/nix/store/03lp4drizbh8cl3f9mjysrrzrg3ssakv-glxinfo-8.4.0";
        assert_eq!(StorePath::strip(store), Some("glxinfo-8.4.0".into()));
    }
}
