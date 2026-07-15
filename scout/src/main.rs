//! ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors from the terminal.

use scout::run;
use std::process::ExitCode;

fn main() -> ExitCode {
    run()
}
