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

    // Compute index-column width from the number of entries.
    let index_width = |len: usize| -> usize {
        if len == 0 {
            1
        } else {
            (len.ilog10() + 1) as usize
        }
    };

    if args.keys_only {
        let keys = store.iter_keys(args.reverse)?;
        let iw = index_width(keys.len());
        for (i, k) in keys.iter().enumerate() {
            format::print_indexed_key(i, iw, k);
        }
    } else if args.values_only {
        let pairs = store.iter(args.reverse)?;
        let iw = index_width(pairs.len());
        for (i, (_, v)) in pairs.iter().enumerate() {
            format::print_indexed_value(i, iw, v);
        }
    } else {
        let pairs = store.iter(args.reverse)?;
        let iw = index_width(pairs.len());

        // Compute the display width of each key so we can align values.
        let max_key_width = pairs
            .iter()
            .map(|(k, _)| format::display_width(k))
            .max()
            .unwrap_or(0);

        for (i, (k, v)) in pairs.iter().enumerate() {
            format::print_indexed_kv(
                i,
                iw,
                k,
                max_key_width,
                v,
                &args.delimiter,
                args.show_binary,
                args.max_value_width,
            );
        }
    }

    Ok(())
}

/// Get the value at the given 0-based list index (default db, forward order).
/// This is invoked by the top-level `clio -i N` flag.
pub fn run_get_by_index(index: usize) -> anyhow::Result<()> {
    let store = Store::open("default")?;
    store.flush()?;

    let keys = store.iter_keys(false)?;

    if index >= keys.len() {
        anyhow::bail!(
            "index {} out of range (0..{})",
            index,
            keys.len()
        );
    }

    let key = &keys[index];
    match store.get(key)? {
        Some(value) => {
            format::print_value(&value);
            Ok(())
        }
        None => {
            // This shouldn't happen since the key came from iter_keys,
            // but handle it gracefully just in case.
            anyhow::bail!(
                "key '{}' disappeared unexpectedly",
                String::from_utf8_lossy(key)
            )
        }
    }
}
