mod cli;
mod config;
mod store;
mod sync;
mod util;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Show a custom welcome / help page when:
    // - bare `clio` (no args)
    // - `clio --help` or `clio -h` (only these flags, no subcommand)
    if args.len() <= 1 {
        cli::help::print_help();
        return;
    }

    // Only intercept top-level help/version flags, not subcommand flags.
    if args.len() == 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                cli::help::print_help();
                return;
            }
            "--version" | "-V" => {
                println!("clio {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => {}
        }
    }

    let cli = Cli::parse();

    // Top-level `-i` / `--index` flag: get value by list index.
    if let Some(index) = cli.index {
        let result = cli::list::run_get_by_index(index);
        if let Err(err) = result {
            eprintln!("error: {:#}", err);
            std::process::exit(1);
        }
        return;
    }

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            cli::help::print_help();
            return;
        }
    };

    let result = match command {
        Command::Set(args) => cli::set::run(args),
        Command::Get(args) => cli::get::run(args),
        Command::Delete(args) => cli::delete::run(args),
        Command::List(args) => cli::list::run(args),
        Command::ListDbs => cli::db::run_list_dbs(),
        Command::DeleteDb(args) => cli::db::run_delete_db(args),
        Command::Push(args) => cli::sync::run_push(args).await,
        Command::Pull(args) => cli::sync::run_pull(args).await,
        Command::Sync(args) => cli::sync::run_sync(args).await,
        Command::SyncStatus => cli::sync::run_sync_status().await,
        Command::InitConfig => cli::config::run_init_config(),
    };

    if let Err(err) = result {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}
