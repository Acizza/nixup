#[macro_use]
extern crate diesel;

mod display;
mod store;

use crate::store::database::SystemDatabase;
use crate::store::Derivation;
use anyhow::{anyhow, Context, Result};
use gumdrop::Options;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::PathBuf;

#[derive(Options)]
struct CmdOptions {
    #[options(help = "print help message")]
    help: bool,
    #[options(
        help = "save the current system package state. Run with this flag before a system update and without this flag after updating to see what was updated"
    )]
    save_state: bool,
}

fn main() -> Result<()> {
    let args = CmdOptions::parse_args_default_or_exit();

    let system_db = SystemDatabase::open().context("failed to open nix database")?;

    if args.save_state {
        let pkgs = Derivation::all_from_system(&system_db)
            .context("failed to parse system derivations")?;

        let state = PackageState::new(pkgs);
        state.save().context("failed to save system package state")
    } else {
        let old_state = PackageState::load().context("failed to load system package state")?;
        let cur_state = Derivation::all_from_system(&system_db)
            .context("failed to parse system derivations")?;

        display::package_diffs(cur_state, old_state.take());
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct PackageState(HashSet<Derivation>);

impl PackageState {
    fn new(packages: HashSet<Derivation>) -> Self {
        PackageState(packages)
    }

    fn save(&self) -> Result<()> {
        let path = Self::save_path().context("failed to get system package state path")?;

        let mut file = File::create(&path).with_context(|| {
            anyhow!("failed to create package state file at {}", path.display())
        })?;

        rmp_serde::encode::write(&mut file, self).with_context(|| {
            anyhow!(
                "failed to encode system package state at {}",
                path.display()
            )
        })?;

        Ok(())
    }

    fn load() -> Result<Self> {
        let path = Self::save_path().context("failed to get system package state path")?;

        let file = File::open(&path)
            .with_context(|| anyhow!("failed to open package state file at {}", path.display()))?;

        let state = rmp_serde::decode::from_read(file).with_context(|| {
            anyhow!(
                "failed to decode system package state at {}",
                path.display()
            )
        })?;

        Ok(state)
    }

    fn save_path() -> Result<PathBuf> {
        let path = get_data_dir()
            .context("failed to get local data directory")?
            .join("packages.mpack");

        Ok(path)
    }

    #[inline(always)]
    fn take(self) -> HashSet<Derivation> {
        self.0
    }
}

fn get_data_dir() -> Result<PathBuf> {
    let dir = dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share/"))
        .join(env!("CARGO_PKG_NAME"));

    if !dir.exists() {
        fs::create_dir_all(&dir).context("failed to create directory")?;
    }

    Ok(dir)
}
