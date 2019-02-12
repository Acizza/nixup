mod error;
mod store;

use crate::error::Error;
use crate::store::StorePath;
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
        save_system_packages()?;
    } else {
        detect_package_diff()?;
    }

    Ok(())
}

fn save_system_packages() -> Result<(), Error> {
    let stores = store::get_system_pkg_stores()?;

    let savefile_path = get_saved_store_path()?;
    let mut file = File::create(savefile_path)?;
    rmp_serde::encode::write(&mut file, &stores)?;

    Ok(())
}

fn detect_package_diff() -> Result<(), Error> {
    let old_stores = load_saved_system_stores()?;
    let current_stores = store::get_system_pkg_stores()?;
    let diffs = store::get_store_diffs(&current_stores, &old_stores);

    println!(
        "{} system package(s) upgraded\n",
        diffs.len().to_string().blue()
    );

    for diff in &diffs {
        let ver_to_str = if cfg!(not(no_colors)) {
            bolden_str_diff(&diff.ver_from, &diff.ver_to)
        } else {
            diff.ver_to.green().to_string()
        };

        println!(
            "{}: {} -> {}",
            diff.name.blue(),
            diff.ver_from.red(),
            ver_to_str
        );
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

fn load_saved_system_stores() -> Result<HashMap<String, StorePath>, Error> {
    let path = get_saved_store_path()?;
    let file = File::open(path)?;
    let mut stores: HashMap<String, StorePath> = rmp_serde::decode::from_read(file)?;

    for (name, store) in &mut stores {
        store.name = name.clone();
    }

    Ok(stores)
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
