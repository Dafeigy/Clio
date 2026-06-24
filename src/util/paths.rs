use std::path::{Path, PathBuf};

/// Returns the base data directory for clio databases.
///
/// Respects `CLIO_DATA_DIR` env var for custom locations (useful for testing).
/// On Linux:   `~/.local/share/clio/`
/// On macOS:   `~/Library/Application Support/clio/`
/// On Windows: `C:\Users\<user>\AppData\Local\clio\`
pub fn data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(custom) = std::env::var("CLIO_DATA_DIR") {
        let dir = PathBuf::from(custom);
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }

    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine XDG data directory"))?
        .join("clio");

    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the path to a specific database file.
///
/// Example: `db_path("work")` → `~/.local/share/clio/work.redb`
pub fn db_path(name: &str) -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join(format!("{}.redb", name)))
}

/// Returns the path to the local sync manifest cache.
pub fn sync_manifest_path() -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join(".sync-manifest.json"))
}

/// Lists all known database names by scanning the data directory.
pub fn list_dbs() -> anyhow::Result<Vec<String>> {
    let dir = data_dir()?;
    let mut dbs = Vec::new();

    let entries = std::fs::read_dir(&dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem() {
                let name = stem.to_string_lossy().to_string();
                if path.extension().map_or(false, |e| e == "redb") {
                    dbs.push(name);
                }
            }
        }
    }

    dbs.sort();
    Ok(dbs)
}

/// Deletes a database file by name. Returns the path that was deleted.
pub fn delete_db(name: &str) -> anyhow::Result<PathBuf> {
    let path = db_path(name)?;
    if !path.exists() {
        anyhow::bail!("database @{} does not exist", name);
    }
    std::fs::remove_file(&path)?;
    Ok(path)
}

/// Checks whether a database exists.
pub fn db_exists(name: &str) -> anyhow::Result<bool> {
    let path = db_path(name)?;
    Ok(path.exists())
}

/// Formats a database path for display (with ~ for home dir).
pub fn display_path(path: &Path) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}
