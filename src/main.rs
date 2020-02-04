#[macro_use]
extern crate diesel;

mod display;
mod err;
mod store;

use crate::err::Result;
use crate::store::database::SystemDatabase;
use crate::store::Derivation;
use gumdrop::Options;
use serde_derive::{Deserialize, Serialize};
use snafu::{ensure, ResultExt};
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

fn main() {
    let args = CmdOptions::parse_args_default_or_exit();

    match run(args) {
        Ok(_) => (),
        Err(err) => {
            err::display_error(err);
            std::process::exit(1);
        }
    }
}

fn run(args: CmdOptions) -> Result<()> {
    ensure!(is_root_user(), err::RunAsRoot);

    let system_db = SystemDatabase::open()?;

    if args.save_state {
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
