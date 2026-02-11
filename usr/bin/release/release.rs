//! Release script: format check, clippy, test, then tag and publish.
//! Run from repository root: cargo run -p release

use std::io::{self, Write};
use std::process::Command;

const GREEN: &str = "\x1b[0;32m";
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const NC: &str = "\x1b[0m";

fn main() {
    let root = std::env::current_dir().expect("current dir");
    let root = root.to_string_lossy();

    run_cmd(&root, "cargo", &["fmt", "--all", "--", "--check"]);
    run_cmd(
        &root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    );
    run_cmd(&root, "cargo", &["test", "--workspace"]);

    if !Command::new("git")
        .args(["diff", "--quiet"])
        .current_dir(&*root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        eprintln!(
            "{}git working directory not clean, please commit your changes first{}",
            RED, NC
        );
        eprintln!(
            "{}Note: cargo fmt may have modified files. Review and commit changes before releasing.{}",
            YELLOW, NC
        );
        std::process::exit(1);
    }

    let version = workspace_version(&root);
    println!("Ready to release version {}", version);
    print!("Continue? [Y/n] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err()
        || (line.trim() != "" && !line.trim().eq_ignore_ascii_case("y"))
    {
        println!("Exiting");
        std::process::exit(1);
    }

    run_cmd(
        &root,
        "cargo",
        &["publish", "-p", "scout_lib", "--allow-dirty"],
    );
    run_cmd(&root, "cargo", &["publish", "-p", "scout", "--allow-dirty"]);
    run_cmd(&root, "git", &["tag", &format!("v{}", version)]);
    run_cmd(&root, "git", &["push", "--tags"]);
    if Command::new("gh").arg("--version").output().is_ok() {
        run_cmd(
            &root,
            "gh",
            &[
                "release",
                "create",
                &format!("v{}", version),
                "--generate-notes",
            ],
        );
    } else {
        eprintln!(
            "{}gh CLI not found, skipping GitHub release creation{}",
            YELLOW, NC
        );
    }
    println!("{}Release {} completed!{}", GREEN, version, NC);
}

fn run_cmd(root: &str, bin: &str, args: &[&str]) {
    let display = format!("{} {}", bin, args.join(" "));
    eprintln!("{}{}{}", GREEN, display, NC);
    let status = Command::new(bin)
        .args(args)
        .current_dir(root)
        .status()
        .expect("run command");
    if !status.success() {
        eprintln!("{}Command failed: {}{}", RED, display, NC);
        std::process::exit(1);
    }
}

fn workspace_version(root: &str) -> String {
    let path = std::path::Path::new(root).join("Cargo.toml");
    let content = std::fs::read_to_string(&path).expect("read root Cargo.toml");
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version = ") {
            let v = line
                .trim_start_matches("version = ")
                .trim_matches('"')
                .trim();
            return v.to_string();
        }
    }
    panic!("version not found in workspace Cargo.toml");
}
