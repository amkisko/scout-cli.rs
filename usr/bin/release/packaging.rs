//! Sync and verify packaging manifests against the workspace version.

#[path = "packaging_edit.rs"]
mod packaging_edit;

use packaging_edit::{
    set_cargo_package_version, set_distversion, set_flatpak_tag, set_gentoo_readme_ebuild,
    set_homebrew_url, set_nix_version, set_pkgver, set_scout_lib_dep_version,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn sync_packaging(root: &Path) -> Result<Vec<String>, String> {
    let version = crate::workspace_version(root);
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
    replace_in_file(
        root,
        &mut updated,
        "scout/Cargo.toml",
        &format!("scout_lib = {{ path = \"../scout_lib\", version = \"{version}\""),
        |content| set_scout_lib_dep_version(content, &version),
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
    let version = crate::workspace_version(root);
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
    expect_contains(
        &mut mismatches,
        root.join("scout/Cargo.toml"),
        &format!("scout_lib = {{ path = \"../scout_lib\", version = \"{version}\""),
    );
    expect_contains(
        &mut mismatches,
        root.join("scout/Cargo.toml"),
        "name = \"scoutapm-cli\"",
    );
    expect_file(&mut mismatches, root.join("scout_lib/LICENSE.md"));
    expect_file(&mut mismatches, root.join("scout_lib/README.md"));
    expect_file(&mut mismatches, root.join("scout/LICENSE.md"));
    expect_file(&mut mismatches, root.join("scout/README.md"));

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

fn expect_file(mismatches: &mut Vec<String>, path: PathBuf) {
    if !path.is_file() {
        mismatches.push(format!("missing file: {}", path.display()));
    }
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
