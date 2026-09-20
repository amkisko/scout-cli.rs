//! Open ScoutAPM UI URLs in a browser or print them.

use std::process::Command;

/// Open `url` in the platform browser, or print it when `print_only` is set.
///
/// When `print_only` is true, the URL goes to stderr so stdout can stay a single
/// JSON document for `--json` pipelines.
pub fn open_or_print_url(url: &str, print_only: bool, quiet: bool) -> Result<(), String> {
    if print_only {
        eprintln!("{url}");
        return Ok(());
    }

    open_browser(url)?;
    if !quiet {
        eprintln!("Opened {url}");
    }
    Ok(())
}

fn open_browser(url: &str) -> Result<(), String> {
    let result = {
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(url).status()
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd").args(["/C", "start", "", url]).status()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Command::new("xdg-open").arg(url).status()
        }
    };

    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("failed to open browser (exit {status})")),
        Err(error) => Err(format!("failed to open browser: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_only_writes_url() {
        open_or_print_url("https://scoutapm.com/apps/1", true, true).unwrap();
    }
}
