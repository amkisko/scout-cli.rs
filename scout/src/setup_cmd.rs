//! Provision the scout agent skill, preferring pray.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SKILL_MD: &str = include_str!("../skills/scout-cli/SKILL.md");
const SCOUT_CLI_MD: &str = include_str!("../skills/scout-cli/scout-cli.md");
const PRAYFILE_SNIPPET: &str = r#"source "scout", git: "https://github.com/amkisko/scout-cli.rs.git", distribution: "prayers"

tree ".agents/skills" do
  pray "scout/scout-cli", "~> 1.0"
end
"#;

pub struct SetupOptions {
    pub copy: bool,
    pub path: Option<PathBuf>,
    pub quiet: bool,
}

pub fn run(options: SetupOptions) -> Result<(), String> {
    if options.copy {
        return copy_skill(
            options
                .path
                .as_deref()
                .unwrap_or(Path::new(".agents/skills/scout-cli")),
            options.quiet,
        );
    }

    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let prayfile = find_prayfile(&cwd);
    let pray_available = pray_on_path();

    if let Some(prayfile_path) = prayfile.as_ref() {
        if pray_available {
            run_pray_install(prayfile_path)?;
            let skill_dir = prayfile_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(".agents/skills/scout-cli");
            if skill_dir.join("SKILL.md").is_file() {
                if !options.quiet {
                    println!(
                        "Provisioned agent skill via pray at {}",
                        skill_dir.display()
                    );
                }
                return Ok(());
            }
            return Err(format!(
                "pray install finished but {} is missing; add this to Prayfile and re-run:\n\n{PRAYFILE_SNIPPET}",
                skill_dir.display()
            ));
        }

        if !options.quiet {
            println!("Found {} but pray is not on PATH.", prayfile_path.display());
            println!("Install pray, then run `pray install` or `scout setup` again.");
            println!(
                "Install: cargo install --git https://github.com/kiskolabs/pray --locked pray"
            );
            println!("Fallback: scout setup --copy");
            println!("Prayfile stanza:\n\n{PRAYFILE_SNIPPET}");
        }
        return Ok(());
    }

    if !options.quiet {
        println!("No Prayfile in this directory or parents.");
        println!(
            "Recommended: add a Prayfile that provisions scout/scout-cli, then run pray install."
        );
        println!("Prayfile stanza:\n\n{PRAYFILE_SNIPPET}");
        if pray_available {
            println!("pray is on PATH.");
        } else {
            println!("pray is not on PATH.");
            println!(
                "Install: cargo install --git https://github.com/kiskolabs/pray --locked pray"
            );
        }
        println!("Fallback without pray: scout setup --copy");
    }
    Ok(())
}

fn pray_on_path() -> bool {
    Command::new("pray")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn find_prayfile(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join("Prayfile");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn run_pray_install(prayfile: &Path) -> Result<(), String> {
    let dir = prayfile.parent().unwrap_or(Path::new("."));
    let status = Command::new("pray")
        .arg("install")
        .current_dir(dir)
        .status()
        .map_err(|error| format!("failed to run pray install: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("pray install failed with {status}"))
    }
}

fn copy_skill(dest: &Path, quiet: bool) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|error| error.to_string())?;
    fs::write(dest.join("SKILL.md"), SKILL_MD).map_err(|error| error.to_string())?;
    fs::write(dest.join("scout-cli.md"), SCOUT_CLI_MD).map_err(|error| error.to_string())?;
    if !quiet {
        println!(
            "Copied scout-cli skill to {} (fallback; prefer Prayfile when possible)",
            dest.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn copy_writes_skill_files() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dest = std::env::temp_dir().join(format!("scout-setup-{nanos}"));
        copy_skill(&dest, true).unwrap();
        assert!(dest.join("SKILL.md").is_file());
        assert!(dest.join("scout-cli.md").is_file());
        let _ = fs::remove_dir_all(dest);
    }
}
