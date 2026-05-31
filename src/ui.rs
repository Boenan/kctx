use anyhow::{Context as _, Result};
use console::{Term, style};
use dialoguer::{Confirm, FuzzySelect, theme::ColorfulTheme};

/// Displays a fuzzy selection list and returns the selected item.
pub fn fuzzy_select(
    prompt: &str,
    items: &[String],
    default_index: usize,
) -> Result<Option<String>> {
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default_index)
        .items(items)
        .interact_opt()
        .context("Failed to read user selection")?;

    Ok(selection.map(|index| items[index].clone()))
}

/// Displays a confirmation prompt and returns the user's choice.
pub fn confirm(prompt: &str) -> Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(false)
        .report(false)
        .interact()
        .context("Failed to read confirmation")
}

/// Prints a standardized success message with a green checkmark.
pub fn print_success(message: &str) {
    println!("{} {}", style("✔").green(), message);
}

/// Prints a standardized error message with a red cross.
#[allow(dead_code)]
pub fn print_error(message: &str) {
    println!("{} {}", style("✘").red(), message);
}

/// Prints a standardized informational message with a cyan arrow.
pub fn print_info(message: &str) {
    println!("{} {}", style("›").cyan(), message);
}

/// Clears the last line of the terminal.
pub fn clear_last_line() -> Result<()> {
    Term::stdout()
        .clear_last_lines(1)
        .context("Failed to clear terminal line")
}

/// Prints a standardized confirmation result (yes/no) after a choice has been made.
pub fn print_choice_confirmed(prompt: &str, confirmed: bool) -> Result<()> {
    clear_last_line()?;
    if confirmed {
        println!(
            "{} {} · {}",
            style("✔").green(),
            prompt,
            style("yes").green()
        );
    } else {
        println!("{} {} · {}", style("✘").red(), prompt, style("no").red());
    }
    Ok(())
}
