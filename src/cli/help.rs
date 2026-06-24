use anstyle::{AnsiColor, Color, Style};

/// Wraps text in the given ANSI style.
fn styled(text: &str, style: Style) -> String {
    format!("{}{}{}", style.render(), text, style.render_reset())
}

/// Color a usage string: command/args in `base`, `[...]` in `bracket`.
fn color_usage(text: &str, base: Style, bracket: Style) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    // Which color are we currently rendering?  false = base (pink), true = bracket (gray).
    let mut in_bracket = false;

    while !rest.is_empty() {
        let pos = if in_bracket {
            rest.find(']').map(|p| p + 1)
        } else {
            rest.find('[')
        };

        match pos {
            Some(p) => {
                let (chunk, remainder) = rest.split_at(p);
                out.push_str(&styled(chunk, if in_bracket { bracket } else { base }));
                rest = remainder;
                in_bracket = !in_bracket;
            }
            None => {
                out.push_str(&styled(rest, if in_bracket { bracket } else { base }));
                break;
            }
        }
    }

    out
}

/// Print the main help page (bare `clio`, `clio --help`, or `clio -h`).
pub fn print_help() {
    use std::io::Write;

    let heading_style = Style::new()
        .fg_color(Some(Color::Ansi(AnsiColor::BrightCyan)))
        .bold();
    let pink_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Blue)));
    let green_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightGreen)));
    let gray_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)));

    let _ = writeln!(std::io::stdout(), "  Clio, a personal key value store with s3 sync.\n");

    // ── USAGE ──
    let _ = writeln!(std::io::stdout(), "  {}", styled("USAGE", heading_style));
    let _ = writeln!(
        std::io::stdout(),
        "    {}",
        color_usage("clio [command] [--flags]", pink_style, gray_style),
    );
    let _ = writeln!(std::io::stdout());

    // ── COMMANDS ──
    let _ = writeln!(std::io::stdout(), "  {}", styled("COMMANDS", heading_style));

    let commands: &[(&str, &str); 12] = &[
        ("set KEY [@DB] [VALUE]", "Set a value for a key with an optional @db"),
        ("get KEY [@DB]",         "Get a value for a key with an optional @db"),
        ("delete KEY [@DB]",      "Delete a key with an optional @db"),
        ("list [@DB] [--flags]",  "List key value pairs with an optional @db"),
        ("list-dbs",              "List databases"),
        ("delete-db @DB",         "Delete a database and all its contents"),
        ("push [@DB] [--flags]",  "Push a local database to S3"),
        ("pull [@DB] [--flags]",  "Pull a remote database from S3"),
        ("sync [--flags]",        "Bidirectional sync"),
        ("sync-status",           "Show sync status (local vs remote diff)"),
        ("init-config",           "Create a config file template"),
        ("help [command]",        "Help about any command"),
    ];

    let max_visible = commands
        .iter()
        .map(|(u, _)| u.len())
        .max()
        .unwrap_or(24);
    let pad_to = max_visible + 4;

    for (usage, desc) in commands {
        let colored = color_usage(usage, pink_style, gray_style);
        let visible = usage.len();
        let padding = pad_to.saturating_sub(visible);

        let _ = writeln!(
            std::io::stdout(),
            "    {}{:padding$}{}",
            colored,
            "",
            desc,
            padding = padding,
        );
    }

    // ── FLAGS ──
    let _ = writeln!(std::io::stdout());
    let _ = writeln!(std::io::stdout(), "  {}", styled("FLAGS", heading_style));

    let _ = writeln!(
        std::io::stdout(),
        "    {:<23} Help for clio",
        styled("-h --help", green_style),
    );
    let _ = writeln!(
        std::io::stdout(),
        "    {:<23} Version for clio",
        styled("-V --version", green_style),
    );
}
