//! Run a single scout command and return structured data (for batch and scripting).

use crate::archive_cmd::{self, ArchiveContext};
use crate::cli::{ArchiveCommands, Commands, ConfigCommands};
use crate::commands::{api_command_value, RunContext};
use crate::config_cmd::{self, ConfigContext};
use crate::diff_cmd::{self, DiffContext};
use crate::output::OutputMode;
use clap::Parser;
use scout_lib::{parse_scout_url, Client};
use serde_json::Value;

pub fn command_requires_api(command: &Commands) -> bool {
    match command {
        Commands::ParseUrl { .. }
        | Commands::Config { .. }
        | Commands::Diff { .. }
        | Commands::Completions { .. }
        | Commands::Man
        | Commands::Version
        | Commands::Batch { .. } => false,
        Commands::Archive { command } => matches!(
            command,
            ArchiveCommands::Pull { dry_run: false, .. } | ArchiveCommands::Trace { .. }
        ),
        _ => true,
    }
}

pub fn command_allowed_in_batch(command: &Commands) -> Result<(), String> {
    match command {
        Commands::Completions { .. } => Err("completions cannot run inside batch".to_string()),
        Commands::Man => Err("man cannot run inside batch".to_string()),
        Commands::Version => Err("use batch only for data commands".to_string()),
        Commands::Batch { .. } => Err("nested batch is not supported".to_string()),
        Commands::Config {
            command: ConfigCommands::Set { .. } | ConfigCommands::Unset { .. },
            ..
        } => {
            Err("config set/unset are disabled in batch (state-changing)".to_string())
        }
        _ => Ok(()),
    }
}

pub async fn execute_command_value(
    client: Option<&Client>,
    command: Commands,
    run_context: &RunContext,
) -> Result<Value, String> {
    command_allowed_in_batch(&command)?;
    let json_mode = OutputMode::JsonCompact;

    match command {
        Commands::ParseUrl { url } => parse_url_value(&url),
        Commands::Config { command, options } => {
            let context = ConfigContext {
                json: true,
                quiet: run_context.quiet,
                dry_run: options.dry_run,
            };
            config_cmd::config_command_value(command, &context, json_mode)
        }
        Commands::Diff { command } => {
            diff_cmd::diff_command_value(command, &DiffContext { mode: json_mode })
        }
        Commands::Archive { command } => {
            let context = ArchiveContext {
                mode: json_mode,
                quiet: run_context.quiet,
            };
            archive_cmd::archive_command_value(client, command, &context).await
        }
        api_command => {
            let client = client.ok_or("this operation requires ScoutAPM API access")?;
            api_command_value(client, api_command, run_context)
                .await
                .map_err(|error| error.to_string())
        }
    }
}

fn parse_url_value(url: &str) -> Result<Value, String> {
    let url = if url == "-" {
        crate::util::read_stdin_line()?
    } else {
        url.to_string()
    };
    let parsed = parse_scout_url(&url).map_err(|error| error.to_string())?;
    serde_json::to_value(parsed).map_err(|error| error.to_string())
}

pub fn build_operation_argv(parent: &crate::cli::Cli, operation_args: &[String]) -> Vec<String> {
    let mut argv = vec!["scout".to_string()];
    if parent.quiet {
        argv.push("--quiet".to_string());
    }
    if parent.json {
        argv.push("--json".to_string());
    }
    if parent.plain {
        argv.push("--plain".to_string());
    }
    if parent.json_pretty {
        argv.push("--json-pretty".to_string());
    }
    if let Some(timeout) = parent.timeout {
        argv.push("--timeout".to_string());
        argv.push(timeout.to_string());
    }
    if let Some(ref api_base) = parent.api_base {
        argv.push("--api-base".to_string());
        argv.push(api_base.clone());
    }
    if let Some(app_id) = parent.app_id {
        argv.push("--app-id".to_string());
        argv.push(app_id.to_string());
    }
    argv.extend(operation_args.iter().cloned());
    argv
}

pub fn parse_operation_cli(
    parent: &crate::cli::Cli,
    operation_args: &[String],
) -> Result<crate::cli::Cli, String> {
    let argv = build_operation_argv(parent, operation_args);
    crate::cli::Cli::try_parse_from(argv).map_err(|error| error.to_string())
}

pub fn format_operation_error(error: String) -> String {
    error
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(error.as_str())
        .trim()
        .to_string()
}

pub fn operation_needs_api_from_args(parent: &crate::cli::Cli, operation_args: &[String]) -> bool {
    parse_operation_cli(parent, operation_args)
        .ok()
        .and_then(|cli| cli.command)
        .is_some_and(|command| command_requires_api(&command))
}
