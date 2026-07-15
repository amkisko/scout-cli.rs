//! Human-oriented CLI error rendering.

use crate::exit::AppExit;
use scout_lib::Error;
use std::io::{self, Write};

const ISSUES_URL: &str = "https://github.com/amkisko/scout-cli.rs/issues/new";

#[derive(Clone)]
pub struct ErrorContext {
    pub quiet: bool,
    pub verbose: bool,
    pub debug: bool,
}

pub fn print_error(message: &str, context: &ErrorContext) -> AppExit {
    let _ = writeln!(io::stderr(), "Error: {message}");
    exit_for_message(message, context)
}

pub fn print_scout_error(error: &Error, context: &ErrorContext) -> AppExit {
    let _ = writeln!(io::stderr(), "Error: {error}");
    if !context.quiet {
        print_hints(error);
        if context.verbose || context.debug {
            print_verbose_details(error, context);
        }
        if should_suggest_bug_report(error) {
            let _ = writeln!(
                io::stderr(),
                "\nReport a bug: {ISSUES_URL} (scout {})",
                env!("CARGO_PKG_VERSION")
            );
        }
    }
    crate::exit::exit_for_scout_error(error)
}

fn print_hints(error: &Error) {
    match error {
        Error::Auth(_) => {
            let _ = writeln!(
                io::stderr(),
                "Hint: run `scout config list` and verify your secret backend."
            );
            let _ = writeln!(
                io::stderr(),
                "      See `scout config path` and the README secret-backend section."
            );
        }
        Error::Api(api) => {
            if api.status_code == Some(401) {
                let _ = writeln!(
                    io::stderr(),
                    "Hint: authentication failed — check your API key via `scout config list`."
                );
            } else if api.status_code == Some(404) {
                let _ = writeln!(
                    io::stderr(),
                    "Hint: the requested resource was not found. Check app and resource IDs."
                );
            }
        }
        Error::Other(message) => {
            if message.contains("Time range") {
                let _ = writeln!(
                    io::stderr(),
                    "Hint: use --range 30min, 1day, or 7days, or --from/--to in ISO 8601."
                );
            }
        }
    }
}

fn print_verbose_details(error: &Error, context: &ErrorContext) {
    if let Error::Api(api) = error {
        if let Some(code) = api.status_code {
            let _ = writeln!(io::stderr(), "Status: {code}");
        }
        if context.debug {
            if let Some(data) = &api.response_data {
                let _ = writeln!(io::stderr(), "Response: {data}");
            }
        }
    }
}

fn should_suggest_bug_report(error: &Error) -> bool {
    matches!(error, Error::Other(message) if message.contains("reqwest") || message.contains("JSON"))
}

fn exit_for_message(message: &str, _context: &ErrorContext) -> AppExit {
    crate::exit::exit_for_message(message)
}
