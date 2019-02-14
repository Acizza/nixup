mod error;
mod store;

use crate::error::Error;
use crate::store::{PackageDiff, StoreDiff, SystemPackage, SystemPackageMap};
use clap::clap_app;
use colored::Colorize;
use hashbrown::HashMap;
use std::cmp::Ordering;
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
        detect_package_diff(new_pkgs, old_pkgs)?;
    } else {
        let old_pkgs = store::parse_system_packages()?;
        perform_dry_rebuild()?;
        let new_pkgs = store::parse_system_packages()?;

        detect_package_diff(new_pkgs, old_pkgs)?;
    }

    Ok(())
}

fn save_system_pkgs() -> Result<(), Error> {
    let packages = {
        let mut all = store::parse_system_packages()?;
        let mut cleaned = Vec::with_capacity(all.len());

        for (_, pkg) in all.drain() {
            cleaned.push(pkg);
        }

        cleaned
    };

    let savefile_path = get_saved_store_path()?;
    let mut file = File::create(savefile_path)?;
    rmp_serde::encode::write(&mut file, &packages)?;

    Ok(())
}

fn get_saved_system_pkgs() -> Result<SystemPackageMap, Error> {
    let path = get_saved_store_path()?;
    let file = File::open(path)?;

    let mut packages: Vec<SystemPackage> = rmp_serde::decode::from_read(file)?;
    let mut results = HashMap::with_capacity(packages.len());

    for pkg in packages.drain(..) {
        results.insert(pkg.path.name.clone(), pkg);
    }

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

fn detect_package_diff(
    mut new_pkgs: SystemPackageMap,
    mut old_pkgs: SystemPackageMap,
) -> Result<(), Error> {
    let format_ver_change = |diff: &StoreDiff| {
        let ver_to_str = if cfg!(not(no_colors)) {
            bolden_str_diff(&diff.ver_from, &diff.ver_to)
        } else {
            diff.ver_to.green().to_string()
        };

        format!("{} -> {}", diff.ver_from.red(), ver_to_str)
    };

    let new_gdeps = store::isolate_global_dependencies(&mut new_pkgs)?;
    let old_gdeps = store::isolate_global_dependencies(&mut old_pkgs)?;

    let mut pkg_diffs = store::get_package_diffs(&new_pkgs, &old_pkgs);
    pkg_diffs.sort_unstable_by(sys_pkg_sorter);

    println!(
        "{} system package update(s)\n",
        pkg_diffs.len().to_string().blue()
    );

    for mut diff in pkg_diffs {
        print!("{}", diff.name.blue());

        if let Some(pkg) = diff.pkg {
            println!(": {}", format_ver_change(&pkg));
        } else {
            println!();
        }

        if diff.deps.is_empty() {
            continue;
        }

        diff.deps.sort_unstable_by(|x, y| x.name.cmp(&y.name));

        for dep in diff.deps {
            println!(
                "{} {}: {}",
                "^".yellow(),
                dep.name.blue(),
                format_ver_change(&dep)
            );
        }
    }

    let mut gdep_diffs = store::get_store_diffs(&new_gdeps, &old_gdeps);
    gdep_diffs.sort_unstable_by(|x, y| x.name.cmp(&y.name));

    println!(
        "\n{} global dependency update(s)\n",
        gdep_diffs.len().to_string().blue()
    );

    for dep_diff in gdep_diffs {
        println!("{}: {}", dep_diff.name.blue(), format_ver_change(&dep_diff));
    }

    Ok(())
}

fn sys_pkg_sorter(new: &PackageDiff, old: &PackageDiff) -> Ordering {
    match (&new.pkg, &old.pkg) {
        (Some(_), Some(_)) | (None, None) => old
            .deps
            .len()
            .cmp(&new.deps.len())
            .then_with(|| new.name.cmp(&old.name)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
    }
}

fn bolden_str_diff<S>(from: S, to: S) -> String
where
    S: AsRef<str>,
{
    let from = from.as_ref();
    let to = to.as_ref();

    let mut result = String::with_capacity(to.len());
    let mut from_chars = from.chars();

    for to_ch in to.chars() {
        let from_ch = from_chars.next();
        let to_str = to_ch.to_string().green();

        if let Some(from_ch) = from_ch {
            if from_ch == to_ch {
                result.push_str(&to_str.to_string());
                continue;
            }
        }

        let to_str = to_str.bright_green().underline().to_string();
        result.push_str(&to_str);
    }

    result
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
