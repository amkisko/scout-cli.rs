//! Run multiple scout operations in one invocation.

use crate::cli::Cli;
use crate::commands::RunContext;
use crate::execute::{execute_command_value, format_operation_error, parse_operation_cli};
use crate::exit::AppExit;
use crate::output::{self, OutputMode};
use crate::util;
use scout_lib::{get_api_key, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchLoadError {
    MissingInteractiveInput,
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchOperation {
    #[serde(default)]
    pub id: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchItemResult {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub args: Vec<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchReport {
    pub operations: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<BatchItemResult>,
}

pub fn print_concise_usage() {
    util::print_concise_usage(
        Some("batch"),
        "Run multiple scout operations from a JSON plan. \
         Output is always a JSON report on stdout.",
        &[
            "echo '[{\"args\":[\"archive\",\"path\"]}]' | scout batch",
            "scout batch --file plan.json",
            "scout batch --help",
        ],
    );
}

pub fn load_operations(file: Option<&str>) -> Result<Vec<BatchOperation>, BatchLoadError> {
    let text = match file {
        Some("-") | None if !util::stdin_is_tty() => read_stdin_text()?,
        Some(path) => std::fs::read_to_string(Path::new(path))
            .map_err(|error| BatchLoadError::Invalid(format!("read batch file: {error}")))?,
        None => return Err(BatchLoadError::MissingInteractiveInput),
    };
    parse_operations_json(&text)
}

fn read_stdin_text() -> Result<String, BatchLoadError> {
    use std::io::Read;
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|error| BatchLoadError::Invalid(format!("read stdin: {error}")))?;
    if buffer.trim().is_empty() {
        return Err(BatchLoadError::Invalid("batch stdin is empty".to_string()));
    }
    Ok(buffer)
}

fn parse_operations_json(text: &str) -> Result<Vec<BatchOperation>, BatchLoadError> {
    if let Ok(operations) = serde_json::from_str::<Vec<BatchOperation>>(text) {
        return validate_operations(operations);
    }
    if let Ok(wrapper) = serde_json::from_str::<BatchFile>(text) {
        return validate_operations(wrapper.operations);
    }
    Err(BatchLoadError::Invalid(
        "batch input must be a JSON array or {\"operations\":[...]}".to_string(),
    ))
}

#[derive(Debug, Deserialize)]
struct BatchFile {
    operations: Vec<BatchOperation>,
}

fn validate_operations(
    operations: Vec<BatchOperation>,
) -> Result<Vec<BatchOperation>, BatchLoadError> {
    if operations.is_empty() {
        return Err(BatchLoadError::Invalid(
            "batch must include at least one operation".to_string(),
        ));
    }
    for (index, operation) in operations.iter().enumerate() {
        if operation.args.is_empty() {
            return Err(BatchLoadError::Invalid(format!(
                "operation {index} is missing args"
            )));
        }
    }
    Ok(operations)
}

pub fn batch_output_mode(cli: &Cli) -> OutputMode {
    if cli.json_pretty || matches!(cli.output, crate::cli::OutputFormatArg::Json) {
        OutputMode::JsonPretty
    } else {
        OutputMode::JsonCompact
    }
}

fn batch_needs_api(parent: &Cli, operations: &[BatchOperation]) -> bool {
    operations.iter().any(|operation| {
        parse_operation_cli(parent, &operation.args)
            .ok()
            .and_then(|cli| cli.command)
            .is_some_and(|command| crate::execute::command_requires_api(&command))
    })
}

pub async fn run_batch(
    parent: Cli,
    operations: Vec<BatchOperation>,
    fail_fast: bool,
) -> Result<(), AppExit> {
    let needs_api = batch_needs_api(&parent, &operations);
    let output_mode = batch_output_mode(&parent);
    let planned_total = operations.len();

    let mut client: Option<Client> = None;
    if needs_api {
        util::progress_message(parent.quiet, "Resolving API key…");
        let (api_key, _) = get_api_key()
            .map_err(|error| crate::cli_error::print_error(&error, &error_context(&parent)))?;
        client = Some(Client::with_options(
            api_key,
            parent.timeout.unwrap_or(15),
            parent.api_base.clone(),
        ));
    }

    let run_context = RunContext {
        mode: output_mode,
        quiet: parent.quiet,
        app_id_override: parent.app_id,
        error_context: error_context(&parent),
    };

    let total = operations.len();
    let mut results = Vec::with_capacity(total);

    for (index, operation) in operations.into_iter().enumerate() {
        let label = operation
            .id
            .clone()
            .unwrap_or_else(|| operation.args.join(" "));
        util::progress_message(
            parent.quiet,
            &format!("Batch {}/{}: {label}", index + 1, total),
        );

        let item_result = match parse_operation_cli(&parent, &operation.args) {
            Ok(parsed) => match parsed.command {
                None => failed_item(
                    index,
                    operation,
                    "operation args must include a subcommand".to_string(),
                ),
                Some(command) => {
                    match execute_command_value(client.as_ref(), command, &run_context).await {
                        Ok(data) => BatchItemResult {
                            index,
                            id: operation.id,
                            args: operation.args,
                            ok: true,
                            data: Some(data),
                            error: None,
                        },
                        Err(error) => failed_item(index, operation, error),
                    }
                }
            },
            Err(error) => failed_item(index, operation, error),
        };
        let operation_failed = !item_result.ok;
        results.push(item_result);
        if fail_fast && operation_failed {
            break;
        }
    }

    let succeeded = results.iter().filter(|item| item.ok).count();
    let failed = results.len() - succeeded;
    let attempted = results.len();
    let report = BatchReport {
        operations: results.len(),
        succeeded,
        failed,
        results: results.clone(),
    };

    let value = serde_json::to_value(&report).map_err(|error| {
        crate::cli_error::print_error(&error.to_string(), &error_context(&parent))
    })?;
    output::emit_value(output_mode, &value)
        .map_err(|error| crate::cli_error::print_error(&error, &error_context(&parent)))?;

    if failed > 0 {
        if !parent.quiet {
            if fail_fast && attempted < planned_total {
                util::user_notice(
                    false,
                    &format!(
                        "Stopped after first failure ({attempted} of {planned_total} operations run)."
                    ),
                );
            } else {
                util::user_notice(
                    false,
                    &format!(
                        "{failed} of {attempted} operations failed — see results[].error in the report."
                    ),
                );
            }
        }
        Err(exit_for_batch_results(&results))
    } else {
        Ok(())
    }
}

fn failed_item(index: usize, operation: BatchOperation, error: String) -> BatchItemResult {
    BatchItemResult {
        index,
        id: operation.id,
        args: operation.args,
        ok: false,
        data: None,
        error: Some(format_operation_error(error)),
    }
}

fn exit_for_batch_results(results: &[BatchItemResult]) -> AppExit {
    if results.iter().any(|item| {
        !item.ok
            && item.error.as_deref().is_some_and(|error| {
                let lower = error.to_lowercase();
                lower.contains("api key") || lower.contains("authentication")
            })
    }) {
        return AppExit::Auth;
    }
    AppExit::General
}

fn error_context(parent: &Cli) -> crate::cli_error::ErrorContext {
    crate::cli_error::ErrorContext {
        quiet: parent.quiet,
        verbose: parent.verbose,
        debug: util::debug_enabled(parent.debug),
    }
}
