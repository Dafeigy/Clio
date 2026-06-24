use crate::cli::parse_key;
use crate::store::Store;
use crate::util::format;

/// Arguments for the `list` command.
#[derive(clap::Args)]
pub struct ListArgs {
    /// Optional @db selector
    pub db: Option<String>,

    /// List in reverse lexicographic order
    #[arg(short = 'r', long)]
    pub reverse: bool,

    /// Only print keys (skip fetching values)
    #[arg(short = 'k', long)]
    pub keys_only: bool,

    /// Only print values
    #[arg(short = 'v', long)]
    pub values_only: bool,

    /// Delimiter between keys and values (default: tab)
    #[arg(short = 'd', long, default_value = "\t")]
    pub delimiter: String,

    /// Show binary values instead of omitting them
    #[arg(short = 'b', long)]
    pub show_binary: bool,

    /// Max displayed value width in terminal (0 = no truncation, default: 8).
    /// When a value exceeds this length, it is cut to `width` chars and
    /// suffixed with "...".  Piped output is never truncated.
    #[arg(short = 'w', long, default_value = "8")]
    pub max_value_width: usize,
}

pub fn run(args: ListArgs) -> anyhow::Result<()> {
    // Parse the optional @db argument; extract just the db name
    let db_name = match &args.db {
        Some(s) => {
            let (_, db) = parse_key(s)?;
            db
        }
        None => "default".to_string(),
    };

    let store = Store::open(&db_name)?;
    store.flush()?;

    if args.keys_only {
        let keys = store.iter_keys(args.reverse)?;
        for k in keys {
            format::print_key(&k);
        }
    } else if args.values_only {
        let pairs = store.iter(args.reverse)?;
        for (_, v) in pairs {
            format::print_value(&v);
        }
    } else {
        let pairs = store.iter(args.reverse)?;
        for (k, v) in pairs {
            format::print_kv(&k, &v, &args.delimiter, args.show_binary, args.max_value_width);
        }
    }

    Ok(())
}
