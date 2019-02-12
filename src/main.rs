mod error;
mod store;

use crate::error::Error;
use crate::store::{StoreDiff, SystemPackage};
use clap::clap_app;
use colored::Colorize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;

fn main() {
    let args = clap_app!(nixup =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: env!("CARGO_PKG_AUTHORS"))
        (about: "A tool for NixOS to display which system packages have been updated")
        (@arg preupdate: -p --preupdate "Must be used before a system update")
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
    if args.is_present("preupdate") {
        save_system_pkgs()?;
    } else {
        detect_package_diff()?;
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

fn load_system_pkgs() -> Result<store::SystemPackageMap, Error> {
    let path = get_saved_store_path()?;
    let file = File::open(path)?;

    let mut packages: Vec<SystemPackage> = rmp_serde::decode::from_read(file)?;
    let mut results = HashMap::with_capacity(packages.len());

    for pkg in packages.drain(..) {
        results.insert(pkg.path.name.clone(), pkg);
    }

    Ok(results)
}

fn detect_package_diff() -> Result<(), Error> {
    let format_ver_change = |diff: &StoreDiff| {
        let ver_to_str = if cfg!(not(no_colors)) {
            bolden_str_diff(&diff.ver_from, &diff.ver_to)
        } else {
            diff.ver_to.green().to_string()
        };

        format!("{} -> {}", diff.ver_from.red(), ver_to_str)
    };

    let (old_pkgs, old_gdeps) = {
        let mut pkgs = load_system_pkgs()?;
        let deps = store::isolate_global_dependencies(&mut pkgs)?;

        (pkgs, deps)
    };

    let (new_pkgs, new_gdeps) = {
        let mut pkgs = store::parse_system_packages()?;
        let deps = store::isolate_global_dependencies(&mut pkgs)?;

        (pkgs, deps)
    };

    let pkg_diffs = store::get_package_diffs(&new_pkgs, &old_pkgs);

    println!(
        "{} system package(s) upgraded\n",
        pkg_diffs.len().to_string().blue()
    );

    for diff in &pkg_diffs {
        print!("{}: ", diff.name.blue());

        if let Some(pkg) = &diff.pkg {
            println!("{}", format_ver_change(pkg));
        } else {
            println!();
        }

        for dep in &diff.deps {
            println!("  {}: {}", dep.name.blue(), format_ver_change(dep));
        }
    }

    let gdep_diffs = store::get_store_diffs(&new_gdeps, &old_gdeps);

    println!(
        "\n{} global dependencies upgraded\n",
        gdep_diffs.len().to_string().blue()
    );

    for dep_diff in &gdep_diffs {
        println!("{}: {}", dep_diff.name.blue(), format_ver_change(dep_diff));
    }

    Ok(())
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
