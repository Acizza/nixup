pub mod diff;

use crate::err::{self, Result};
use rayon::prelude::*;
use serde_derive::{Deserialize, Serialize};
use smallvec::SmallVec;
use snafu::{OptionExt, ResultExt};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
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
                if !split_sep[1].chars().any(char::is_numeric) {
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

    pub fn with_deps<P>(path: P) -> Result<SystemPackage>
    where
        P: Into<StorePath>,
    {
        let mut package = SystemPackage::from_path(path);
        package.parse_deps()?;

        Ok(package)
    }

    pub fn parse_deps(&mut self) -> Result<()> {
        let path = match &self.path.path {
            Some(path) => path,
            None => return Ok(()),
        };

        let mut cmd = Command::new("nix-store");
        cmd.arg("-qR");
        cmd.arg(path);

        let output = {
            let output = cmd.output().context(err::CommandIO { cmd })?;
            let mut content = String::from_utf8(output.stdout)?;

            // The last line contains the current package, so strip it from the output
            if let Some(idx) = content.rfind("/nix/") {
                content.replace_range(idx.., "");
            }

            content
        };

        self.deps = parse_unique_stores(output.lines());
        Ok(())
    }
}

fn parse_unique_stores<'a, I>(paths: I) -> StorePathMap
where
    I: IntoIterator<Item = &'a str>,
{
    let mut stores = HashSet::<StorePath>::new();
    let mut duplicates = HashSet::<String>::new();

    for path in paths {
        let store = match StorePath::parse(path) {
            Some(store) => store,
            None => continue,
        };

        if duplicates.contains(&store.name) {
            continue;
        }

        if stores.contains(&store.name) {
            stores.remove(&store);
            duplicates.insert(store.name);
            continue;
        }

        stores.insert(store);
    }

    stores
}

pub fn parse_kernel_store() -> Result<StorePath> {
    let mut cmd = Command::new("nix-store");
    cmd.arg("-qR");
    cmd.arg("/nix/var/nix/profiles/system/kernel");

    let output = {
        let output = cmd.output().context(err::CommandIO { cmd })?;
        String::from_utf8(output.stdout)?
    };

    let path = output.lines().next().context(err::GetKernelStore)?;
    let store = StorePath::parse(path).context(err::GetKernelStore)?;

    Ok(store)
}

pub fn parse_system_stores() -> Result<StorePathMap> {
    let mut cmd = Command::new("nix-store");
    cmd.arg("-q");
    cmd.arg("--references");
    cmd.arg("/nix/var/nix/profiles/system/sw/");

    let output = {
        let output = cmd.output().context(err::CommandIO { cmd })?;
        String::from_utf8(output.stdout)?
    };

    let stores = parse_unique_stores(output.lines());
    Ok(stores)
}

pub fn parse_system_packages() -> Result<SystemPackageMap> {
    let stores = parse_system_stores()?;
    let mut packages = HashMap::with_capacity(stores.len());

    packages.par_extend(stores.into_par_iter().filter_map(|store| {
        let name = store.name.clone();
        let pkg = SystemPackage::with_deps(store).ok()?;

        Some((name, pkg))
    }));

    Ok(packages)
}

#[cfg(test)]
mod test {
    use super::*;

    pub fn mkstore<S>(name: S, ver: S) -> StorePath
    where
        S: Into<String>,
    {
        StorePath {
            path: None,
            name: name.into(),
            version: ver.into(),
        }
    }

    pub fn mksyspkg<P, V>(path: P, deps: V) -> SystemPackage
    where
        P: Into<StorePath>,
        V: Into<StorePathMap>,
    {
        SystemPackage {
            path: path.into(),
            deps: deps.into(),
        }
    }

    #[test]
    fn parse_store_info() {
        let paths = [
            (
                "/nix/store/123abc-glxinfo-8.4.0",
                Some(mkstore("glxinfo", "8.4.0")),
            ),
            ("/nix/store/123abc-fix-static.patch", None),
            (
                "/nix/store/123abc-nix-wallpaper-simple-dark-gray_bottom.png.drv",
                None,
            ),
            ("/nix/store/123abc-pcre-8.42", Some(mkstore("pcre", "8.42"))),
            (
                "/nix/store/123abc-dxvk-v0.96",
                Some(mkstore("dxvk", "v0.96")),
            ),
            (
                "/nix/store/123abc-dxvk-6062dfbef4d5c0f061b9f6e342acab54f34e089a",
                Some(mkstore("dxvk", "6062dfbef4d5c0f061b9f6e342acab54f34e089a")),
            ),
            (
                "/nix/store/123abc-dxvk-fe781df591465b196ae273bf9f110797274d84bd",
                Some(mkstore("dxvk", "fe781df591465b196ae273bf9f110797274d84bd")),
            ),
            (
                "/nix/store/123abc-rpcs3-7788-4c59395",
                Some(mkstore("rpcs3", "7788-4c59395")),
            ),
            ("/nix/store/123abc-gcc-7.4.0", Some(mkstore("gcc", "7.4.0"))),
            (
                "/nix/store/123abc-steam-runtime-2016-08-26",
                Some(mkstore("steam-runtime", "2016-08-26")),
            ),
            (
                "/nix/store/123abc-wine-wow-4.0-rc5-staging",
                Some(mkstore("wine-wow|staging", "4.0-rc5")),
            ),
            (
                "/nix/store/123abc-ffmpeg-3.4.5-bin",
                Some(mkstore("ffmpeg|bin", "3.4.5")),
            ),
            (
                "/nix/store/123abc-vulkan-loader-1.1.85",
                Some(mkstore("vulkan-loader", "1.1.85")),
            ),
            (
                "/nix/store/123abc-vpnc-0.5.3-post-r550",
                Some(mkstore("vpnc", "0.5.3-post-r550")),
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
