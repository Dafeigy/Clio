pub mod set;
pub mod get;
pub mod delete;
pub mod list;
pub mod db;
pub mod sync;
pub mod config;
pub mod help;

use clap::{Parser, Subcommand};
use clap::builder::Styles;

fn help_styles() -> Styles {
    use anstyle::{AnsiColor, Color, Style};

    Styles::styled()
        .header(
            Style::new()
                .fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
                .bold(),
        )
        .literal(
            Style::new()
                .fg_color(Some(Color::Ansi(AnsiColor::BrightRed))),
        )
        .usage(
            Style::new()
                .fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
                .bold(),
        )
}

/// Clio, a personal key value store.
#[derive(Parser)]
#[command(
    name = "clio",
    version,
    about = "Clio, a personal key value store.",
    styles = help_styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Set a value for a key with an optional @db.
    /// If VALUE is omitted, reads from stdin.
    #[command(visible_alias = "put")]
    Set(set::SetArgs),

    /// Get a value for a key with an optional @db.
    Get(get::GetArgs),

    /// Delete a key with an optional @db.
    #[command(visible_aliases = ["del", "rm"])]
    Delete(delete::DeleteArgs),

    /// List key-value pairs with an optional @db.
    #[command(visible_alias = "ls")]
    List(list::ListArgs),

    /// List all databases.
    #[command(visible_alias = "ls-db")]
    ListDbs,

    /// Delete a database and all its contents.
    #[command(visible_aliases = ["del-db", "rm-db"])]
    DeleteDb(db::DeleteDbArgs),

    /// Push a local database to S3.
    Push(sync::PushArgs),

    /// Pull a database from S3, overwriting local.
    Pull(sync::PullArgs),

    /// Bidirectional sync: push local changes, pull remote changes.
    Sync(sync::SyncArgs),

    /// Show sync status (local vs remote diff).
    SyncStatus,

    /// Create a config file template at ~/.config/clio/config.toml.
    InitConfig,
}

/// Parses a KEY@DB argument into (key_bytes, db_name).
///
/// Rules:
/// - `foo`      → (b"foo", "default")
/// - `foo@bar`  → (b"foo", "bar")
/// - `@bar`     → (b"", "bar")     — used for list, delete-db
/// - `foo@a@b`  → error            — too many @
pub fn parse_key(key: &str) -> anyhow::Result<(Vec<u8>, String)> {
    let parts: Vec<&str> = key.splitn(3, '@').collect();

    match parts.len() {
        1 => {
            let k = parts[0].to_lowercase();
            Ok((k.into_bytes(), "default".to_string()))
        }
        2 => {
            let k = parts[0].to_lowercase();
            let db = parts[1].to_lowercase();
            Ok((k.into_bytes(), db))
        }
        _ => {
            anyhow::bail!("bad key format '{}', use KEY@DB", key)
        }
    }
}
