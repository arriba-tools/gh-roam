use anyhow::{Result, bail};
use dialoguer::Confirm;
use std::io::IsTerminal;

/// True when a person can answer prompts. Prompts render on stderr so stdout stays clean.
fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Ask a yes/no question. `yes` answers it up front, and without a terminal it is required.
pub fn confirm(prompt: &str, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !interactive() {
        bail!("{prompt} Pass --yes to confirm without a prompt.");
    }
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_confirms_without_asking() {
        assert!(confirm("Delete it?", true).unwrap());
    }
}
