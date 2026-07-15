use assert_cmd::Command;
use predicates::prelude::*;

fn scout() -> Command {
    Command::cargo_bin("scout").unwrap()
}

#[test]
fn version_flag_prints_version() {
    scout()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("scout 0.3.0"));
}

#[test]
fn version_subcommand_prints_version() {
    scout()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("scout 0.3.0"));
}

#[test]
fn help_flag_shows_usage() {
    scout()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Query ScoutAPM"));
}

#[test]
fn parse_url_works_without_api_key() {
    scout()
        .arg("parse-url")
        .arg("https://scoutapm.com/apps/42")
        .assert()
        .success()
        .stdout(predicate::str::contains("app_id"));
}

#[test]
fn global_output_flag_after_subcommand() {
    scout()
        .arg("parse-url")
        .arg("https://scoutapm.com/apps/42")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"app_id\":42"));
}

#[test]
fn missing_api_key_reports_auth_exit_code() {
    let temp_home = std::env::temp_dir().join(format!("scout-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .env_remove("SCOUT_OP_ENTRY_PATH")
        .env_remove("SCOUT_BW_ITEM_ID")
        .env_remove("SCOUT_KPXC_DB")
        .env_remove("SCOUT_KPXC_ENTRY")
        .arg("apps")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("API key not found"))
        .stderr(predicate::str::contains("scout config path"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn no_input_blocks_default_tui() {
    scout()
        .arg("--no-input")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Interactive mode disabled"));
}

#[test]
fn completions_bash_generates_script() {
    scout()
        .arg("completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("_scout"));
}

#[test]
fn man_command_prints_roff() {
    scout()
        .arg("man")
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH"));
}

#[test]
fn archive_path_works_without_api_key() {
    let temp_home = std::env::temp_dir().join(format!("scout-archive-cli-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_ARCHIVE_HOME", temp_home.to_string_lossy().to_string())
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("archive")
        .arg("path")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("archive_home"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn archive_help_lists_pull_and_status() {
    scout()
        .arg("archive")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("pull"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn diff_help_lists_subcommands() {
    scout()
        .arg("diff")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("endpoints"))
        .stdout(predicate::str::contains("metrics"))
        .stdout(predicate::str::contains("errors"))
        .stdout(predicate::str::contains("jobs"));
}

#[test]
fn quiet_still_prints_errors() {
    let temp_home = std::env::temp_dir().join(format!("scout-quiet-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .env_remove("SCOUT_OP_ENTRY_PATH")
        .env_remove("SCOUT_BW_ITEM_ID")
        .env_remove("SCOUT_KPXC_DB")
        .env_remove("SCOUT_KPXC_ENTRY")
        .arg("apps")
        .arg("--quiet")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("API key not found"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn config_path_json_output() {
    scout()
        .arg("config")
        .arg("path")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("scout_home"))
        .stdout(predicate::str::contains("config_path"));
}

#[test]
fn endpoints_help_hides_tui_flags() {
    scout()
        .arg("endpoints")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("APP_ID"))
        .stdout(predicate::str::contains("--tab").not())
        .stdout(predicate::str::contains("--refresh").not());
}

#[test]
fn parse_url_without_url_shows_help() {
    scout()
        .arg("parse-url")
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse-url"))
        .stderr(predicate::str::contains("Examples:"));
}

#[test]
fn diff_metrics_rejects_unknown_metric_type() {
    scout()
        .arg("diff")
        .arg("metrics")
        .arg("123")
        .arg("not_a_metric")
        .arg("--left-date")
        .arg("2025-01-01")
        .arg("--right-date")
        .arg("2025-01-08")
        .assert()
        .failure()
        .stderr(predicate::str::contains("response_time"));
}

#[test]
fn top_level_help_documents_exit_codes() {
    scout()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Exit codes"));
}

#[test]
fn archive_pull_dry_run_works_without_api_key() {
    let temp_home = std::env::temp_dir().join(format!("scout-archive-dry-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_ARCHIVE_HOME", temp_home.to_string_lossy().to_string())
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
        .arg("1day")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\":true"))
        .stdout(predicate::str::contains("\"app_id\":123"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn diff_missing_snapshot_suggests_archive_pull() {
    let temp_home = std::env::temp_dir().join(format!("scout-diff-hint-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_ARCHIVE_HOME", temp_home.to_string_lossy().to_string())
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("diff")
        .arg("endpoints")
        .arg("123")
        .arg("--left-from")
        .arg("2025-01-01T00:00:00Z")
        .arg("--left-to")
        .arg("2025-01-02T00:00:00Z")
        .arg("--right-from")
        .arg("2025-01-08T00:00:00Z")
        .arg("--right-to")
        .arg("2025-01-09T00:00:00Z")
        .assert()
        .failure()
        .stderr(predicate::str::contains("archive pull 123"))
        .stderr(predicate::str::contains("archive status 123"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn config_dry_run_notice_goes_to_stderr() {
    let temp_home = std::env::temp_dir().join(format!("scout-config-stderr-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("config")
        .arg("--dry-run")
        .arg("set")
        .arg("op.entry_path")
        .arg("op://Vault/Item")
        .assert()
        .success()
        .stderr(predicate::str::contains("would set"))
        .stdout(predicate::str::is_empty());

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_local_ops_without_api_key() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-local-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .env_remove("SCOUT_OP_ENTRY_PATH")
        .env_remove("SCOUT_BW_ITEM_ID")
        .env_remove("SCOUT_KPXC_DB")
        .env_remove("SCOUT_KPXC_ENTRY")
        .arg("batch")
        .write_stdin(
            r#"[{"id":"archive","args":["archive","path"]},{"id":"config","args":["config","path"]}]"#,
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("\"succeeded\":2"))
        .stdout(predicate::str::contains("archive_home"))
        .stdout(predicate::str::contains("config_path"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_rejects_nested_batch() {
    scout()
        .arg("batch")
        .write_stdin(r#"[{"args":["batch"]}]"#)
        .assert()
        .failure()
        .stdout(predicate::str::contains("nested batch is not supported"));
}

#[test]
fn batch_rejects_config_set() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-config-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("batch")
        .write_stdin(r#"[{"args":["config","set","op.entry_path","op://Vault/Item"]}]"#)
        .assert()
        .failure()
        .stdout(predicate::str::contains("config set/unset are disabled in batch"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_help_documents_plan_format() {
    scout()
        .arg("batch")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("operations"))
        .stdout(predicate::str::contains("--file"))
        .stdout(predicate::str::contains("config set/unset"));
}

#[test]
fn batch_empty_stdin_returns_usage_exit() {
    scout()
        .arg("batch")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("batch stdin is empty"));
}

#[test]
fn batch_invalid_json_returns_usage_exit() {
    scout()
        .arg("batch")
        .write_stdin("not json")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("batch input must be a JSON array"));
}

#[test]
fn batch_partial_failure_prints_stderr_summary() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-partial-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("batch")
        .write_stdin(
            r#"[{"args":["archive","path"]},{"args":["batch"]}]"#,
        )
        .assert()
        .failure()
        .stderr(predicate::str::contains("1 of 2 operations failed"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_json_pretty_formats_report() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-pretty-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("batch")
        .arg("--json-pretty")
        .write_stdin(r#"[{"args":["archive","path"]}]"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  \"succeeded\": 1"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_fail_fast_stops_after_first_failure() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-failfast-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("batch")
        .arg("--fail-fast")
        .write_stdin(
            r#"[{"args":["batch"]},{"args":["archive","path"]},{"args":["config","path"]}]"#,
        )
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"operations\":1"))
        .stdout(predicate::str::contains("nested batch is not supported"))
        .stdout(predicate::str::contains("archive_home").not())
        .stderr(predicate::str::contains("Stopped after first failure"));

    let _ = std::fs::remove_dir_all(temp_home);
}

#[test]
fn batch_without_fail_fast_runs_remaining_operations() {
    let temp_home = std::env::temp_dir().join(format!("scout-batch-noff-{}", std::process::id()));
    std::fs::create_dir_all(&temp_home).unwrap();

    scout()
        .env("SCOUT_HOME", temp_home.to_string_lossy().to_string())
        .arg("batch")
        .write_stdin(
            r#"[{"args":["batch"]},{"args":["archive","path"]}]"#,
        )
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"operations\":2"))
        .stdout(predicate::str::contains("archive_home"));

    let _ = std::fs::remove_dir_all(temp_home);
}
