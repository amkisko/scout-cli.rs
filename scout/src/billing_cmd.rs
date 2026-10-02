//! `scout billing` — org usage for the current billing period.

use crate::output::{self, OutputMode};
use scout_lib::{Client, Error};

pub async fn run(client: &Client, mode: OutputMode) -> Result<(), Error> {
    let usage = client.get_org_usage().await?;
    output::emit_value(mode, &usage).map_err(Error::Other)
}

/// Format a simple usage bar for human output consumers (tests + future UI).
pub fn usage_bar(used: i64, limit: i64, width: usize) -> String {
    if limit <= 0 || width == 0 {
        return String::new();
    }
    let mut filled = ((used as f64 / limit as f64) * width as f64).round() as usize;
    if filled > width {
        filled = width;
    }
    if filled == 0 && used > 0 {
        filled = 1;
    }
    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_usage_shows_at_least_one_block() {
        let bar = usage_bar(1, 1000, 20);
        assert!(bar.contains('█'));
    }
}
