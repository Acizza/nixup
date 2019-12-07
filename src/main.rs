mod display;
mod err;
mod store;

use crate::err::Result;
use crate::store::{Derivation, SystemDatabase};
use clap::clap_app;
use serde_derive::{Deserialize, Serialize};
use snafu::{ensure, ResultExt};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::PathBuf;

fn main() {
    let args = clap_app!(nixup =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: env!("CARGO_PKG_AUTHORS"))
        (about: "A tool for NixOS to display which system packages have been updated")
        (@arg save_state: -s --save "Save the current system package state. Run with this flag before a system update and without any flags afterwards to see what was updated.")
    )
    .get_matches();

    match run(&args) {
        Ok(_) => (),
        Err(err) => {
            err::display_error(err);
            std::process::exit(1);
        }
    }
}

fn run(args: &clap::ArgMatches) -> Result<()> {
    ensure!(is_root_user(), err::RunAsRoot);

    let system_db = SystemDatabase::open()?;

    if args.is_present("save_state") {
        let pkgs = Derivation::all_from_system(&system_db)?;
        let state = PackageState::new(pkgs);
        state.save()
    } else {
        let old_state = PackageState::load()?;
        let cur_state = Derivation::all_from_system(&system_db)?;

        display::package_diffs(cur_state, old_state.take());
        Ok(())
    }
}

fn is_root_user() -> bool {
    unsafe { libc::getuid() == 0 }
}

#[derive(Serialize, Deserialize)]
struct PackageState(HashSet<Derivation>);

impl PackageState {
    fn new(packages: HashSet<Derivation>) -> Self {
        PackageState(packages)
    }

    fn save(&self) -> Result<()> {
        let path = Self::save_path()?;
        let mut file = File::create(&path).context(err::FileIO { path })?;
        rmp_serde::encode::write(&mut file, self)?;
        Ok(())
    }

    fn load() -> Result<Self> {
        let path = Self::save_path()?;
        let file = File::open(&path).context(err::FileIO { path })?;
        let state = rmp_serde::decode::from_read(file)?;
        Ok(state)
    }

    fn save_path() -> Result<PathBuf> {
        let path = get_data_dir()?.join("packages.mpack");
        Ok(path)
    }

    #[inline(always)]
    fn take(self) -> HashSet<Derivation> {
        self.0
    }
}

fn get_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share/"))
        .join(env!("CARGO_PKG_NAME"));

    if !dir.exists() {
        fs::create_dir_all(&dir).context(err::FileIO { path: &dir })?;
    }

    Ok(dir)
}
