use crate::error::StoreError;
use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Eq)]
pub struct StorePath {
    #[serde(skip)]
    pub name: String,
    pub version: String,
    pub suffix: Option<String>,
}

impl StorePath {
    pub fn parse_stripped<P>(path: P) -> Option<StorePath>
    where
        P: AsRef<str>,
    {
        // Black magic incoming!
        lazy_static! {
            static ref MATCHER: Regex = Regex::new(
                r"(?P<name>[\w\-\.]+?)-v?(?P<version>\d[\d\.\-a-z_]+?(?:-[a-z]+?\d+)?)(?:\.[a-z]+|-(?P<suffix>[a-z]+)|$)"
            )
            .unwrap();
        }

        let caps = match MATCHER.captures(path.as_ref()) {
            Some(caps) => caps,
            None => return None,
        };

        let store = StorePath {
            name: caps["name"].into(),
            version: caps["version"].into(),
            suffix: caps.name("suffix").map(|s| s.as_str().into()),
        };

        Some(store)
    }

    pub fn parse<P>(path: P) -> Option<StorePath>
    where
        P: AsRef<Path>,
    {
        let path = StorePath::strip(path)?;
        StorePath::parse_stripped(path)
    }

    pub fn strip<P>(path: P) -> Option<String>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let mut name = path.file_name()?.to_string_lossy().into_owned();

        match name.find('-') {
            Some(idx) if name.len() <= idx + 1 => return None,
            Some(idx) => name.replace_range(..=idx, ""),
            None => return None,
        }

        Some(name)
    }
}

impl PartialEq for StorePath {
    fn eq(&self, other: &StorePath) -> bool {
        self.name == other.name && self.version == other.version && self.suffix == other.suffix
    }
}

pub fn get_system_pkg_stores() -> Result<HashMap<String, StorePath>, StoreError> {
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

    let mut stores = HashMap::<_, StorePath>::new();

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
        if let Some(existing) = stores.get(&store.name) {
            if existing.version > store.version {
                continue;
            }
        }

        stores.insert(store.name.clone(), store);
    }

    Ok(stores)
}

#[derive(Debug)]
pub struct StoreDiff {
    pub name: String,
    pub ver_from: String,
    pub ver_to: String,
}

pub fn get_store_diffs(
    new_stores: &HashMap<String, StorePath>,
    old_stores: &HashMap<String, StorePath>,
) -> Vec<StoreDiff> {
    let mut diffs = Vec::new();

    for new in new_stores.values() {
        let old = match old_stores.get(&new.name) {
            Some(old) => old,
            None => continue,
        };

        if new.version.ends_with(&old.version) {
            continue;
        }

        match (&new.suffix, &old.suffix) {
            (None, None) => (),
            (Some(new_sfx), Some(old_sfx)) => {
                if !new_sfx.ends_with(old_sfx) {
                    continue;
                }
            }
            (Some(_), None) => continue,
            (None, Some(_)) => continue,
        }

        let diff = StoreDiff {
            name: new.name.clone(),
            ver_from: old.version.clone(),
            ver_to: new.version.clone(),
        };

        diffs.push(diff);
    }

    diffs
}

#[cfg(test)]
mod test {
    use super::*;

    fn mkpath<S>(name: S, ver: S, sfx: Option<S>) -> Option<StorePath>
    where
        S: Into<String>,
    {
        Some(StorePath {
            name: name.into(),
            version: ver.into(),
            suffix: sfx.map(|s| s.into()),
        })
    }

    #[test]
    fn parse_store_info() {
        let paths = [
            ("glxinfo-8.4.0", mkpath("glxinfo", "8.4.0", None)),
            ("fix-static.patch", None),
            ("nix-wallpaper-simple-dark-gray_bottom.png.drv", None),
            ("pcre-8.42", mkpath("pcre", "8.42", None)),
            ("dxvk-v0.96", mkpath("dxvk", "0.96", None)),
            (
                "dxvk-6062dfbef4d5c0f061b9f6e342acab54f34e089a",
                mkpath("dxvk", "6062dfbef4d5c0f061b9f6e342acab54f34e089a", None),
            ),
            ("rpcs3-7788-4c59395", mkpath("rpcs3", "7788-4c59395", None)),
            ("gcc-7.4.0", mkpath("gcc", "7.4.0", None)),
            (
                "steam-runtime-2016-08-26",
                mkpath("steam-runtime", "2016-08-26", None),
            ),
            (
                "wine-wow-4.0-rc5-staging",
                mkpath("wine-wow", "4.0-rc5", Some("staging")),
            ),
            ("ffmpeg-3.4.5-bin", mkpath("ffmpeg", "3.4.5", Some("bin"))),
        ];

        for (full, path) in &paths {
            match StorePath::parse_stripped(full) {
                Some(result) => match path {
                    Some(p) => assert_eq!(result, *p),
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
        assert_eq!(StorePath::parse(store), mkpath("glxinfo", "8.4.0", None));
    }
}
