use std::borrow::Cow;

use owo_colors::OwoColorize;
use tokio::task;

use crate::{config::DevServerConfig, startup::Application};

enum ValueTone {
    Primary,
    Success,
    Warning,
    Danger,
    Accent,
    Muted,
}

pub fn print_startup_summary(config: &DevServerConfig, app: &Application) {
    let title = "WEB DEV SERVER";
    let border = "=".repeat(title.len() + 8);

    println!("{}", border.clone().bright_black());
    println!("  {}", title.cyan().bold());
    println!("{}", border.bright_black());

    let address_primary = app.primary_url();
    let address_alt = format!("http://localhost:{}", app.port());
    let base_dir = Cow::Owned(app.base_dir().display().to_string());
    let diff_mode = if app.diff_mode() {
        Cow::Borrowed("ENABLED")
    } else {
        Cow::Borrowed("disabled")
    };
    let watching = if config.diff_mode {
        Cow::Borrowed("Diff HTML/CSS updates")
    } else {
        Cow::Borrowed("Full page reloads")
    };
    let browser = if config.no_open_browser {
        Cow::Borrowed("Manual (--no-open-browser)")
    } else {
        Cow::Borrowed("Auto-open on start")
    };

    let rows: Vec<(&str, Cow<'_, str>, ValueTone)> = vec![
        ("Address", Cow::Owned(address_primary), ValueTone::Primary),
        ("Alt", Cow::Owned(address_alt), ValueTone::Muted),
        ("Base Dir", base_dir, ValueTone::Accent),
        (
            "Diff Mode",
            diff_mode,
            if app.diff_mode() {
                ValueTone::Success
            } else {
                ValueTone::Danger
            },
        ),
        ("Watching", watching, ValueTone::Warning),
        ("Browser", browser, ValueTone::Accent),
        (
            "Exit",
            Cow::Borrowed("Press Ctrl+C to stop"),
            ValueTone::Accent,
        ),
    ];

    let label_width = rows
        .iter()
        .map(|(label, _, _)| label.len())
        .max()
        .unwrap_or(0)
        + 1;

    for (label, value, tone) in rows {
        let padded_label = format!("{label:<label_width$}:", label_width = label_width);
        let colored_label = format!("{}", padded_label.bright_blue().bold());
        let colored_value = colorize(value.as_ref(), tone);

        println!("  {} {}", colored_label, colored_value);
    }

    println!();
    println!(
        "  {} {}",
        "Serving".bright_black(),
        app.base_dir().display().to_string().bright_black()
    );
    if config.no_open_browser {
        println!(
            "  {}",
            "Browser launch disabled (--no-open-browser)."
                .bright_black()
                .italic()
        );
    } else {
        println!(
            "  {}",
            "Copy the address above if your browser did not open automatically."
                .bright_black()
                .italic()
        );
    }
    println!(
        "  {}",
        "Leave this terminal open to keep the live server running."
            .bright_black()
            .italic()
    );
    println!();
}

fn colorize(value: &str, tone: ValueTone) -> String {
    match tone {
        ValueTone::Primary => value.bold().bright_white().to_string(),
        ValueTone::Success => value.bold().bright_green().to_string(),
        ValueTone::Warning => value.to_string().bright_yellow().to_string(),
        ValueTone::Danger => value.bold().bright_red().to_string(),
        ValueTone::Accent => value.to_string().bright_cyan().to_string(),
        ValueTone::Muted => value.to_string().dimmed().to_string(),
    }
}

pub fn launch_browser(url: String) {
    task::spawn(async move {
        match task::spawn_blocking(move || open::that(url)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("[web-dev-server] failed to open browser: {error}");
            }
            Err(error) => {
                eprintln!("[web-dev-server] browser task join error: {error}");
            }
        }
    });
}
