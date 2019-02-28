use super::PackageState;
use crate::error::Error;
use crate::store::diff::{self, PackageDiff, StoreDiff};
use colored::Colorize;
use std::cmp::Ordering;

pub fn package_diffs(
    mut cur_state: PackageState,
    mut old_state: PackageState,
) -> Result<(), Error> {
    let gdep_diffs = {
        let new = diff::remove_global_deps(&mut cur_state.packages)?;
        let old = diff::remove_global_deps(&mut old_state.packages)?;

        let mut diffs = diff::get_store_diffs(&new, &old);
        diffs.sort_unstable_by(|x, y| x.name.cmp(&y.name));
        diffs
    };

    let pkg_diffs = {
        let mut diffs = diff::get_package_diffs(&cur_state.packages, &old_state.packages);
        diffs.sort_unstable_by(sys_pkg_sorter);
        diffs
    };

    let kernel_diff = diff::get_store_diff(&cur_state.kernel, &old_state.kernel);

    let num_updates = pkg_diffs.len() + (kernel_diff.is_some() as usize);
    println!("{} package update(s)\n", num_updates.to_string().blue());

    if let Some(kernel_diff) = kernel_diff {
        display_store_diff(&kernel_diff);
    }

    for diff in pkg_diffs {
        display_pkg_diff(diff);
    }

    println!(
        "\n{} global dependency update(s)\n",
        gdep_diffs.len().to_string().blue()
    );

    for dep_diff in gdep_diffs {
        println!("{}: {}", dep_diff.name.blue(), format_ver_change(&dep_diff));
    }

    Ok(())
}

fn display_store_diff(diff: &StoreDiff) {
    println!("{}: {}", diff.name.blue(), format_ver_change(diff));
}

fn display_pkg_diff(mut diff: PackageDiff) {
    match diff.pkg {
        Some(pkg) => display_store_diff(&pkg),
        None => println!("{}", diff.name.blue()),
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
        let to_str = to_ch.to_string();

        if let Some(from_ch) = from_ch {
            if from_ch == to_ch {
                result.push_str(&to_str.green().to_string());
                continue;
            }
        }

        let to_str = to_str.bright_green().underline().to_string();
        result.push_str(&to_str);
    }

    result
}
