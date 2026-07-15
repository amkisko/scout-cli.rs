//! Process exit codes for script-friendly error handling.

use scout_lib::Error;
use std::process::ExitCode;

/// Non-zero exit codes mapped to failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppExit {
    Success = 0,
    General = 1,
    Usage = 2,
    Auth = 3,
    Api = 4,
    Io = 5,
}

impl AppExit {
    pub fn code(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

impl From<AppExit> for ExitCode {
    fn from(value: AppExit) -> Self {
        value.code()
    }
}

pub fn exit_for_scout_error(error: &Error) -> AppExit {
    match error {
        Error::Auth(_) => AppExit::Auth,
        Error::Api(_) => AppExit::Api,
        Error::Other(message) => {
            let lower = message.to_lowercase();
            if lower.contains("not found")
                || lower.contains("invalid")
                || lower.contains("unknown")
                || lower.contains("must")
            {
                AppExit::Usage
            } else if lower.contains("read ")
                || lower.contains("write ")
                || lower.contains("io ")
                || lower.contains("could not resolve")
            {
                AppExit::Io
            } else {
                AppExit::General
            }
        }
    }
}

pub fn exit_for_message(message: &str) -> AppExit {
    let lower = message.to_lowercase();
    if lower.contains("api key") || lower.contains("authentication") {
        AppExit::Auth
    } else if lower.contains("unknown config")
        || lower.contains("invalid")
        || lower.contains("not set")
        || lower.contains("must not be empty")
        || lower.contains("batch input")
        || lower.contains("batch stdin")
        || lower.contains("batch must")
        || lower.contains("missing args")
        || lower.starts_with("operation ")
    {
        AppExit::Usage
    } else if lower.contains("read ")
        || lower.contains("write ")
        || lower.contains("could not resolve")
    {
        AppExit::Io
    } else {
        AppExit::General
    }
}
