use crate::store::diff::{self, PackageDiff, StoreDiff};
use crate::store::Derivation;
use colored::Colorize;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;

pub fn package_diffs(cur_state: HashSet<Derivation>, old_state: HashSet<Derivation>) {
    let pkg_diffs = {
        let mut diffs = diff::get_package_diffs(&cur_state, &old_state);
        diffs.sort_unstable_by(sys_pkg_sorter);
        diffs
    };

    println!("{} package update(s)\n", pkg_diffs.len().to_string().blue());

    for diff in pkg_diffs {
        display_pkg_diff(diff);
    }
}

fn format_store_diff(diff: &StoreDiff) -> String {
    let suffix = match &diff.suffix {
        Some(suffix) => Cow::Owned(format!(" {{{}}}", suffix).blue().bold().to_string()),
        None => Cow::Borrowed(""),
    };

    format!(
        "{}{}: {}",
        diff.name.blue(),
        suffix,
        format_ver_change(diff)
    )
}

fn display_pkg_diff(mut diff: PackageDiff) {
    match diff.pkg {
        Some(pkg) => println!("{}", format_store_diff(&pkg)),
        None => println!("{}", diff.name.blue()),
    }

    if diff.deps.is_empty() {
        return;
    }

    diff.deps.sort_unstable_by(|x, y| x.name.cmp(&y.name));

    for dep in diff.deps {
        println!("{} {}", "^".yellow(), format_store_diff(&dep));
    }
}

fn sys_pkg_sorter(new: &PackageDiff, old: &PackageDiff) -> Ordering {
    match (&new.pkg, &old.pkg) {
        (Some(_), Some(_)) | (None, None) => new
            .deps
            .len()
            .cmp(&old.deps.len())
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
