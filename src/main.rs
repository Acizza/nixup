mod display;
mod error;
mod store;

use crate::error::Error;
use crate::store::{StorePath, SystemPackageMap};
use clap::clap_app;
use serde_derive::{Deserialize, Serialize};
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
            let err: failure::Error = err.into();

            eprintln!("error: {}", err);

            for cause in err.iter_chain().skip(1) {
                eprintln!("  cause: {}", cause);
            }

            let backtrace = err.backtrace().to_string();

            if !backtrace.is_empty() {
                eprintln!("{}", backtrace);
            }

            std::process::exit(1);
        }
    }
}

fn run(args: &clap::ArgMatches) -> Result<(), Error> {
    if args.is_present("save_state") {
        let state = PackageState::get_current()?;
        state.save()?;
    } else {
        let old_state = PackageState::load()?;
        let cur_state = PackageState::get_current()?;

        display::package_diffs(cur_state, old_state)?;
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PackageState {
    pub kernel: StorePath,
    pub packages: SystemPackageMap,
}

impl PackageState {
    fn get_current() -> Result<PackageState, Error> {
        let kernel = store::parse_kernel_store()?;
        let packages = store::parse_system_packages()?;

        Ok(PackageState { kernel, packages })
    }

    fn save(&self) -> Result<(), Error> {
        let path = PackageState::get_save_path()?;
        let mut file = File::create(path)?;

        rmp_serde::encode::write(&mut file, self)?;

        Ok(())
    }

    fn load() -> Result<PackageState, Error> {
        let path = PackageState::get_save_path()?;
        let file = File::open(path)?;

        let state = rmp_serde::decode::from_read(file)?;

        Ok(state)
    }

    fn get_save_path() -> Result<PathBuf, Error> {
        let path = get_cache_dir()?.join("package_state.mpack");
        Ok(path)
    }
}

fn get_cache_dir() -> Result<PathBuf, Error> {
    let dir = dirs::cache_dir()
        .ok_or(Error::FailedToGetCacheDir)?
        .join(env!("CARGO_PKG_NAME"));

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    Ok(dir)
}
