//! Scout CLI library (parser, commands, output).

pub mod archive_cmd;
pub mod batch_cmd;
pub mod cli;
pub mod cli_error;
pub mod commands;
pub mod completions;
pub mod config_cmd;
pub mod diff_cmd;
pub mod execute;
pub mod exit;
pub mod man;
pub mod output;
pub mod tui;
pub mod util;

use clap::Parser;
use cli::{Cli, Commands, ConfigCommands, OutputFormatArg, TuiTabArg};
use cli_error::{print_error, print_scout_error, ErrorContext};
use commands::RunContext;
use exit::AppExit;
use output::{OutputFormat, OutputMode};
use scout_lib::{get_api_key, parse_scout_url, Client, Error};
use std::process::ExitCode;

pub fn run() -> ExitCode {
    match try_run() {
        Ok(()) => AppExit::Success.code(),
        Err(exit) => exit.code(),
    }
}

fn try_run() -> Result<(), AppExit> {
    let cli = Cli::parse();
    let error_context = ErrorContext {
        quiet: cli.quiet,
        verbose: cli.verbose,
        debug: util::debug_enabled(cli.debug),
    };

    if matches!(cli.command, Some(Commands::Version)) {
        println!("scout {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if let Some(Commands::Completions { shell }) = cli.command {
        return completions::run(shell).map_err(|error| print_error(&error, &error_context));
    }

    if matches!(cli.command, Some(Commands::Man)) {
        return man::run().map_err(|error| print_error(&error, &error_context));
    }

    if let Some(Commands::ParseUrl { url }) = &cli.command {
        let mode = resolve_output_mode(&cli);
        return run_parse_url(url.clone(), mode)
            .map_err(|error| print_scout_error(&error, &error_context));
    }

    if let Some(Commands::Config {
        ref command,
        ref options,
    }) = cli.command
    {
        let config_context = config_cmd::ConfigContext {
            json: matches!(cli.output, OutputFormatArg::Json) || cli.json || cli.json_pretty,
            quiet: cli.quiet,
            dry_run: options.dry_run,
        };
        return run_config(command.clone(), &config_context, resolve_output_mode(&cli));
    }

    if let Some(Commands::Diff { ref command }) = cli.command {
        let mode = resolve_output_mode(&cli);
        return diff_cmd::run_diff_command(command.clone(), &diff_cmd::DiffContext { mode })
            .map_err(|error| print_error(&error, &error_context));
    }

    if let Some(Commands::Batch {
        ref file,
        fail_fast,
    }) = cli.command
    {
        let operations = match batch_cmd::load_operations(file.as_deref()) {
            Err(batch_cmd::BatchLoadError::MissingInteractiveInput) => {
                batch_cmd::print_concise_usage();
                return Err(AppExit::Usage);
            }
            Err(batch_cmd::BatchLoadError::Invalid(message)) => {
                return Err(print_error(&message, &error_context));
            }
            Ok(operations) => operations,
        };
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| print_error(&error.to_string(), &error_context))?;
        return runtime.block_on(batch_cmd::run_batch(cli, operations, fail_fast));
    }

    if let Some(Commands::Archive { ref command }) = cli.command {
        if matches!(
            command,
            cli::ArchiveCommands::Path
                | cli::ArchiveCommands::Status { .. }
                | cli::ArchiveCommands::Export { .. }
                | cli::ArchiveCommands::Pull { dry_run: true, .. }
        ) {
            let mode = resolve_output_mode(&cli);
            return archive_cmd::run_archive_local_command(
                command.clone(),
                &archive_cmd::ArchiveContext {
                    mode,
                    quiet: cli.quiet,
                },
            )
            .map_err(|error| print_error(&error, &error_context));
        }
    }

    if cli.command.is_none() {
        if cli.no_input {
            return Err(print_error(
                "Interactive mode disabled. Pass a subcommand for non-interactive use.",
                &error_context,
            ));
        }
        if !util::stdin_is_tty() || !util::stdout_is_tty() {
            util::print_concise_usage(
                None,
                "Query ScoutAPM from the terminal. Pass a subcommand, or run \
                 with no arguments in a terminal for the interactive browser.",
                &[
                    "scout apps",
                    "scout endpoints 123 --range 1day",
                    "scout --help",
                ],
            );
            return Err(AppExit::Usage);
        }
    }

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| print_error(&error.to_string(), &error_context))?;

    runtime.block_on(async { run_async(cli, error_context).await })
}

async fn run_async(cli: Cli, error_context: ErrorContext) -> Result<(), AppExit> {
    if cli.command.is_none() {
        util::progress_message(cli.quiet, "Starting Scout…");
    }
    util::progress_message(cli.quiet, "Resolving API key…");
    let (api_key, _source) = get_api_key().map_err(|error| print_error(&error, &error_context))?;
    let client = Client::with_options(api_key, cli.timeout.unwrap_or(15), cli.api_base.clone());
    let mode = resolve_output_mode(&cli);
    let run_context = RunContext {
        mode,
        quiet: cli.quiet,
        app_id_override: cli.app_id,
        error_context: error_context.clone(),
    };

    if cli.command.is_none() {
        let tui_options = tui::Options {
            app: cli.app.clone(),
            tab: match cli.tab {
                TuiTabArg::Endpoints => tui::Tab::Endpoints,
                TuiTabArg::Insights => tui::Tab::Insights,
                TuiTabArg::Metrics => tui::Tab::Metrics,
                TuiTabArg::Errors => tui::Tab::Errors,
            },
            refresh_secs: cli.refresh,
            use_utc: cli.utc,
            no_color: cli.no_color || !util::color_enabled(cli.no_color),
        };
        return tui::run(&client, tui_options)
            .await
            .map_err(|error| print_error(&error, &error_context));
    }

    let command = cli.command.unwrap();
    match command {
        Commands::Archive {
            command:
                cli::ArchiveCommands::Pull {
                    app,
                    from,
                    to,
                    range,
                    resource,
                    metric,
                    force,
                    incremental,
                    trace_id,
                    trace_endpoint_limit,
                    dry_run,
                },
        } => archive_cmd::run_archive_pull(
            &client,
            archive_cmd::ArchivePullRequest {
                app,
                from,
                to,
                range,
                resource,
                metric,
                trace_id,
                trace_endpoint_limit,
                force,
                incremental,
                dry_run,
            },
            &archive_cmd::ArchiveContext {
                mode: run_context.mode,
                quiet: run_context.quiet,
            },
        )
        .await
        .map_err(|error| print_error(&error, &error_context)),
        Commands::Archive {
            command:
                cli::ArchiveCommands::Trace {
                    app,
                    trace_id,
                    force,
                },
        } => archive_cmd::run_archive_trace(
            &client,
            app,
            trace_id,
            force,
            &archive_cmd::ArchiveContext {
                mode: run_context.mode,
                quiet: run_context.quiet,
            },
        )
        .await
        .map_err(|error| print_error(&error, &error_context)),
        other => commands::run_api_command(&client, other, &run_context)
            .await
            .map_err(|error| print_scout_error(&error, &error_context)),
    }
}

fn resolve_output_mode(cli: &Cli) -> OutputMode {
    let output = match cli.output {
        OutputFormatArg::Plain => OutputFormat::Plain,
        OutputFormatArg::Json => OutputFormat::Json,
    };
    output::resolve_output_mode(output, cli.json, cli.plain, cli.json_pretty)
}

fn run_config(
    command: ConfigCommands,
    context: &config_cmd::ConfigContext,
    mode: OutputMode,
) -> Result<(), AppExit> {
    match command {
        ConfigCommands::List => config_cmd::run_list(context),
        ConfigCommands::Get { key } => config_cmd::run_get(&key, context),
        ConfigCommands::Set { key, value } => config_cmd::run_set(&key, &value, context),
        ConfigCommands::Unset { key } => config_cmd::run_unset(&key, context),
        ConfigCommands::Path => config_cmd::run_path(context, mode),
    }
    .map_err(|error| {
        print_error(
            &error,
            &ErrorContext {
                quiet: context.quiet,
                verbose: false,
                debug: false,
            },
        )
    })
}

fn run_parse_url(url: String, mode: OutputMode) -> Result<(), Error> {
    let url = if url == "-" {
        util::read_stdin_line().map_err(Error::Other)?
    } else {
        url
    };
    let parsed = parse_scout_url(&url).map_err(Error::Other)?;
    let value = serde_json::to_value(&parsed).map_err(|error| Error::Other(error.to_string()))?;
    output::emit_value(mode, &value).map_err(Error::Other)
}
