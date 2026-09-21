use assert_cmd::Command;
use predicates::prelude::*;

fn scout() -> Command {
    Command::cargo_bin("scout").unwrap()
}

#[test]
fn archive_pull_help_documents_windows_and_resources() {
    scout()
        .arg("archive")
        .arg("pull")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--range"))
        .stdout(predicate::str::contains("--resource"))
        .stdout(predicate::str::contains("7 days"))
        .stdout(predicate::str::contains("30 days"))
        .stdout(predicate::str::contains("0 = all"))
        .stdout(predicate::str::contains("endpoint_metrics"));
}

#[test]
fn archive_pull_dry_run_default_omits_traces() {
    let temp_home = std::env::temp_dir().join(format!("scout-archive-help-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();
    scout()
        .env(
            "SCOUT_ARCHIVE_HOME",
            temp_home.to_string_lossy().to_string(),
        )
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .env_remove("SCOUT_OP_ENTRY_PATH")
        .env_remove("SCOUT_BW_ITEM_ID")
        .env_remove("SCOUT_KPXC_DB")
        .env_remove("SCOUT_KPXC_ENTRY")
        .arg("archive")
        .arg("pull")
        .arg("123")
        .arg("--dry-run")
        .arg("--range")
        .arg("max")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\":true"))
        .stdout(predicate::str::contains("endpoint_metrics"))
        .stdout(predicate::str::contains("insights"))
        .stdout(predicate::str::contains("\"resources\""))
        .stdout(predicate::str::contains("traces").not());
    let _ = std::fs::remove_dir_all(temp_home);
}
