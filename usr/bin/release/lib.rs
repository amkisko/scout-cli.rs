//! Shared release helpers: workspace version, packaging sync, and loc limits.

mod loc_limits;

pub use loc_limits::{
    check_loc_limits, write_baseline as write_loc_baseline, LocFinding, LocFindingKind, LocReport,
    HARD_LIMIT as HARD_LOC_LIMIT, SOFT_LIMIT as SOFT_LOC_LIMIT,
};

use std::fs;
use std::path::{Path, PathBuf};

pub fn workspace_version(root: &Path) -> String {
    let content = fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml");
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version = ") {
            return line
                .trim_start_matches("version = ")
                .trim_matches('"')
                .trim()
                .to_string();
        }
    }
    panic!("version not found in workspace Cargo.toml");
}

pub fn sync_packaging(root: &Path) -> Result<Vec<String>, String> {
    let version = workspace_version(root);
    let mut updated = Vec::new();

    replace_in_file(
        root,
        &mut updated,
        "flake.nix",
        &format!(r#"version = "{version}";"#),
        |content| set_nix_version(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/nix/default.nix",
        &format!(r#"version = "{version}";"#),
        |content| set_nix_version(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/nix/flake.nix",
        &format!(r#"version = "{version}";"#),
        |content| set_nix_version(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/homebrew/scout-cli.rb",
        &format!(
            "url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v{version}.tar.gz\""
        ),
        |content| set_homebrew_url(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/flatpak/io.github.amkisko.scout-cli.yml",
        &format!("tag: v{version}"),
        |content| set_flatpak_tag(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/aur/PKGBUILD",
        &format!("pkgver={version}"),
        |content| set_pkgver(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/freebsd/Makefile",
        &format!("DISTVERSION=\t{version}"),
        |content| set_distversion(content, &version),
    )?;
    replace_in_file(
        root,
        &mut updated,
        "usr/bin/release/Cargo.toml",
        &format!("version = \"{version}\""),
        |content| set_cargo_package_version(content, &version),
    )?;

    rename_gentoo_ebuild(root, &version, &mut updated)?;
    replace_in_file(
        root,
        &mut updated,
        "packaging/gentoo/README.md",
        &format!("scout-cli-{version}.ebuild"),
        |content| set_gentoo_readme_ebuild(content, &version),
    )?;

    Ok(updated)
}

pub fn check_packaging(root: &Path) -> Result<(), Vec<String>> {
    let version = workspace_version(root);
    let mut mismatches = Vec::new();

    expect_contains(
        &mut mismatches,
        root.join("flake.nix"),
        &format!(r#"version = "{version}";"#),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/nix/default.nix"),
        &format!(r#"version = "{version}";"#),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/nix/flake.nix"),
        &format!(r#"version = "{version}";"#),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/homebrew/scout-cli.rb"),
        &format!("v{version}.tar.gz"),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/flatpak/io.github.amkisko.scout-cli.yml"),
        &format!("tag: v{version}"),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/aur/PKGBUILD"),
        &format!("pkgver={version}"),
    );
    expect_contains(
        &mut mismatches,
        root.join("packaging/freebsd/Makefile"),
        &format!("DISTVERSION=\t{version}"),
    );
    expect_contains(
        &mut mismatches,
        root.join("usr/bin/release/Cargo.toml"),
        &format!("version = \"{version}\""),
    );

    let ebuild = root.join(format!(
        "packaging/gentoo/app-misc/scout-cli/scout-cli-{version}.ebuild"
    ));
    if !ebuild.is_file() {
        mismatches.push(format!("missing gentoo ebuild: {}", ebuild.display()));
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(mismatches)
    }
}

fn replace_in_file(
    root: &Path,
    updated: &mut Vec<String>,
    relative_path: &str,
    expected_snippet: &str,
    transform: impl FnOnce(&str) -> String,
) -> Result<(), String> {
    let path = root.join(relative_path);
    let content =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let next = transform(&content);
    if next == content {
        if !content.contains(expected_snippet) {
            return Err(format!(
                "could not update {} to include {expected_snippet}",
                path.display()
            ));
        }
        return Ok(());
    }
    fs::write(&path, next).map_err(|error| format!("write {}: {error}", path.display()))?;
    updated.push(relative_path.to_string());
    Ok(())
}

fn expect_contains(mismatches: &mut Vec<String>, path: PathBuf, expected: &str) {
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            mismatches.push(format!("read {}: {error}", path.display()));
            return;
        }
    };
    if !content.contains(expected) {
        mismatches.push(format!(
            "{} is out of sync (expected {expected})",
            path.display()
        ));
    }
}

fn set_nix_version(content: &str, version: &str) -> String {
    replace_line_value(content, "version = ", &format!("\"{version}\";"))
}

fn set_homebrew_url(content: &str, version: &str) -> String {
    let prefix = "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v";
    replace_prefixed_line(content, prefix, &format!("{prefix}{version}.tar.gz\""))
}

fn set_flatpak_tag(content: &str, version: &str) -> String {
    replace_line_value(content, "tag: v", version)
}

fn set_pkgver(content: &str, version: &str) -> String {
    replace_line_value(content, "pkgver=", version)
}

fn set_distversion(content: &str, version: &str) -> String {
    replace_line_value(content, "DISTVERSION=\t", version)
}

fn set_cargo_package_version(content: &str, version: &str) -> String {
    replace_line_value(content, "version = ", &format!("\"{version}\""))
}

fn set_gentoo_readme_ebuild(content: &str, version: &str) -> String {
    let pattern = "scout-cli-";
    content
        .lines()
        .map(|line| {
            if let Some(index) = line.find(pattern) {
                let suffix_start = index + pattern.len();
                if let Some(end) = line[suffix_start..].find(".ebuild") {
                    let mut next = line.to_string();
                    next.replace_range(suffix_start..suffix_start + end, version);
                    return next;
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn replace_line_value(content: &str, prefix: &str, value: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with(prefix) {
                let indent = line.len() - line.trim_start().len();
                format!("{}{prefix}{value}", &line[..indent])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn replace_prefixed_line(content: &str, prefix: &str, replacement: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.starts_with(prefix) || line.contains(prefix) {
                replacement.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if content.ends_with('\n') { "\n" } else { "" }
}

fn rename_gentoo_ebuild(
    root: &Path,
    version: &str,
    updated: &mut Vec<String>,
) -> Result<(), String> {
    let directory = root.join("packaging/gentoo/app-misc/scout-cli");
    let target = directory.join(format!("scout-cli-{version}.ebuild"));
    if target.is_file() {
        return Ok(());
    }

    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?;
    let mut source = None;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read gentoo ebuild dir: {error}"))?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with("scout-cli-") && name.ends_with(".ebuild") {
            source = Some(entry.path());
            break;
        }
    }

    let source =
        source.ok_or_else(|| format!("no scout-cli ebuild found in {}", directory.display()))?;
    if source == target {
        return Ok(());
    }
    fs::rename(&source, &target).map_err(|error| {
        format!(
            "rename {} -> {}: {error}",
            source.display(),
            target.display()
        )
    })?;
    updated.push(target.strip_prefix(root).unwrap().display().to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_nix_version_replaces_existing_value() {
        let input = "  version = \"0.1.0\";\n";
        assert_eq!(set_nix_version(input, "0.2.0"), "  version = \"0.2.0\";\n");
    }

    #[test]
    fn set_homebrew_url_replaces_tagged_archive_url() {
        let input =
            "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.1.0.tar.gz\"\n";
        let expected =
            "  url \"https://github.com/amkisko/scout-cli.rs/archive/refs/tags/v0.2.0.tar.gz\"";
        assert_eq!(set_homebrew_url(input, "0.2.0"), format!("{expected}\n"));
    }

    #[test]
    fn set_gentoo_readme_ebuild_updates_filename_reference() {
        let input = "Template: scout-cli-0.1.0.ebuild\n";
        assert_eq!(
            set_gentoo_readme_ebuild(input, "0.2.0"),
            "Template: scout-cli-0.2.0.ebuild\n"
        );
    }
}
