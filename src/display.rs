use crate::error::Error;
use crate::store::{self, PackageDiff, StoreDiff, SystemPackageMap};
use colored::Colorize;
use std::cmp::Ordering;

pub fn package_diffs(
    mut new_pkgs: SystemPackageMap,
    mut old_pkgs: SystemPackageMap,
) -> Result<(), Error> {
    let new_gdeps = store::isolate_global_dependencies(&mut new_pkgs)?;
    let old_gdeps = store::isolate_global_dependencies(&mut old_pkgs)?;

    let mut pkg_diffs = store::get_package_diffs(&new_pkgs, &old_pkgs);
    pkg_diffs.sort_unstable_by(sys_pkg_sorter);

    println!("{} package update(s)\n", pkg_diffs.len().to_string().blue());

    for diff in pkg_diffs {
        display_diff(diff);
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

fn display_diff(mut diff: PackageDiff) {
    print!("{}", diff.name.blue());

    if let Some(pkg) = diff.pkg {
        println!(": {}", format_ver_change(&pkg));
    } else {
        println!();
    }

    if diff.deps.is_empty() {
        return;
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

fn format_ver_change(diff: &StoreDiff) -> String {
    let ver_to_str = if cfg!(not(no_colors)) {
        bolden_str_diff(&diff.ver_from, &diff.ver_to)
    } else {
        diff.ver_to.green().to_string()
    };

    format!("{} -> {}", diff.ver_from.red(), ver_to_str)
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
