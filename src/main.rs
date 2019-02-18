mod display;
mod error;
mod store;

use crate::error::Error;
use crate::store::{SystemPackage, SystemPackageMap};
use clap::clap_app;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let args = clap_app!(nixup =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: env!("CARGO_PKG_AUTHORS"))
        (about: "A tool for NixOS to display which system packages have been updated")
        (@arg save_state: -s --savestate "Save the current system package state, so it can be compared to later with the -f flag")
        (@arg from_state: -f --fromstate "Use the state saved from the -s flag, instead of fetching the latest one")
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
        save_system_pkgs()?;
    } else if args.is_present("from_state") {
        let old_pkgs = get_saved_system_pkgs()?;
        let new_pkgs = store::parse_system_packages()?;
        display::package_diffs(new_pkgs, old_pkgs)?;
    } else {
        display_updates_from_cur_state()?;
    }

    Ok(())
}

fn display_updates_from_cur_state() -> Result<(), Error> {
    let euid = unsafe { libc::geteuid() };

    // We have to be running as root in this mode, otherwise NixOS will
    // only fetch updates for user packages when we perform a dry rebuild
    if euid != 0 {
        return Err(Error::MustRunAsRoot);
    }

    let old_pkgs = store::parse_system_packages()?;
    perform_dry_rebuild()?;
    let new_pkgs = store::parse_system_packages()?;

    display::package_diffs(new_pkgs, old_pkgs)
}

fn save_system_pkgs() -> Result<(), Error> {
    let packages = store::parse_system_packages()?
        .into_iter()
        .map(|(_, v)| v)
        .collect::<Vec<_>>();

    let savefile_path = get_saved_store_path()?;
    let mut file = File::create(savefile_path)?;
    rmp_serde::encode::write(&mut file, &packages)?;

    Ok(())
}

fn get_saved_system_pkgs() -> Result<SystemPackageMap, Error> {
    let path = get_saved_store_path()?;
    let file = File::open(path)?;

    let packages: Vec<SystemPackage> = rmp_serde::decode::from_read(file)?;
    let results = packages
        .into_iter()
        .map(|pkg| (pkg.path.name.clone(), pkg))
        .collect::<SystemPackageMap>();

    Ok(results)
}

fn perform_dry_rebuild() -> Result<(), Error> {
    let mut cmd = Command::new("nixos-rebuild");
    cmd.arg("dry-build");
    cmd.arg("--upgrade");

    let output = cmd.output().map_err(Error::FailedToExecuteProcess)?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(999);
        return Err(Error::BadProcessExitCode(code));
    }

    Ok(())
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

fn get_saved_store_path() -> Result<PathBuf, Error> {
    let path = get_cache_dir()?.join("saved_stores.mpack");
    Ok(path)
}
