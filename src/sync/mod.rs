pub mod backend;
pub mod manifest;

use crate::store::Store;
use crate::sync::backend::S3Backend;
use crate::sync::manifest::{SyncManifest, SyncState};
use crate::util::paths;
use std::collections::BTreeMap;

/// Push a single database to S3.
///
/// Returns a message describing what was done.
pub async fn push_db(name: &str, force: bool) -> anyhow::Result<String> {
    let backend = S3Backend::load()?;
    let store = Store::open(name)?;
    let local_checksum = store.checksum()?;
    let local_size = store.file_size()?;

    let remote = backend.get_manifest().await?;
    let remote_entry = remote.as_ref().and_then(|m| m.databases.get(name));

    // Check if we're already in sync.
    if let Some(entry) = remote_entry {
        if entry.checksum == local_checksum {
            return Ok(format!("@{} is already in sync", name));
        }

        // If remote has also changed and force is not set, warn.
        if !force {
            if let Some(entry) = remote_entry {
                // Simple heuristic: if remote is newer than local file mtime, warn.
                let local_meta = std::fs::metadata(store.file_path())?;
                if let Ok(local_ts) = local_meta.modified() {
                    use std::time::SystemTime;
                    let remote_time = entry.last_modified.timestamp() as u64;
                    let local_time = local_ts
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    if remote_time > local_time {
                        anyhow::bail!(
                            "Remote @{} has been modified more recently. \
                             Use --force to overwrite remote.",
                            name
                        );
                    }
                }
            }
        }
    }

    // Upload.
    backend.upload_db(name, store.file_path()).await?;

    // Update manifest.
    let mut manifest = remote.unwrap_or_else(|| {
        SyncManifest::new(
            hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        )
    });
    manifest.upsert(name, local_checksum, local_size);
    backend.put_manifest(&manifest).await?;

    // Cache locally.
    cache_manifest(&manifest)?;

    Ok(format!("Pushed @{} ({:.1} KB)", name, local_size as f64 / 1024.0))
}

/// Pull a single database from S3.
pub async fn pull_db(name: &str, force: bool) -> anyhow::Result<String> {
    let backend = S3Backend::load()?;
    let remote = backend
        .get_manifest()
        .await?
        .ok_or_else(|| anyhow::anyhow!("no remote data found"))?;

    let remote_entry = remote
        .databases
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("@{} does not exist on remote", name))?;

    // If local exists and has changes, warn unless forced.
    if paths::db_exists(name)? && !force {
        let store = Store::open(name)?;
        let local_checksum = store.checksum()?;
        if local_checksum != remote_entry.checksum {
            anyhow::bail!(
                "Local @{} has unsynced changes. Use --force to overwrite local.",
                name
            );
        }
    }

    // Download.
    let local_path = paths::db_path(name)?;
    backend.download_db(name, &local_path).await?;

    // Cache manifest.
    cache_manifest(&remote)?;

    Ok(format!(
        "Pulled @{} ({:.1} KB)",
        name,
        remote_entry.size_bytes as f64 / 1024.0
    ))
}

/// Bidirectional sync: push local changes, pull remote changes.
pub async fn sync_all() -> anyhow::Result<String> {
    let backend = S3Backend::load()?;
    let remote = backend.get_manifest().await?;
    let local_dbs = paths::list_dbs()?;

    let mut messages = Vec::new();

    // Collect all known database names.
    let mut all_dbs = BTreeMap::new();
    for db in &local_dbs {
        all_dbs.entry(db.clone()).or_insert(true);
    }
    if let Some(ref manifest) = remote {
        for db in manifest.databases.keys() {
            all_dbs.entry(db.clone()).or_insert(false);
        }
    }

    for (db_name, has_local) in &all_dbs {
        let store = if *has_local {
            Store::open(db_name).ok()
        } else {
            None
        };

        let local_checksum = store.as_ref().and_then(|s| s.checksum().ok());
        let remote_entry = remote.as_ref().and_then(|m| m.databases.get(db_name));

        let state = match (has_local, remote_entry) {
            (true, None) => SyncState::LocalOnly,
            (false, Some(_)) => SyncState::RemoteOnly,
            (true, Some(entry)) => {
                if let Some(ref lc) = local_checksum {
                    if lc == &entry.checksum {
                        SyncState::InSync
                    } else {
                        // Use modified time from the file to determine direction.
                        if let Some(s) = &store {
                            if let Ok(meta) = std::fs::metadata(s.file_path()) {
                                if let Ok(mtime) = meta.modified() {
                                    use std::time::SystemTime;
                                    let local_secs = mtime
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs() as i64;
                                    let remote_secs = entry.last_modified.timestamp();
                                    if local_secs > remote_secs {
                                        SyncState::LocalNewer
                                    } else {
                                        SyncState::RemoteNewer
                                    }
                                } else {
                                    SyncState::RemoteNewer
                                }
                            } else {
                                SyncState::RemoteNewer
                            }
                        } else {
                            SyncState::RemoteNewer
                        }
                    }
                } else {
                    SyncState::RemoteNewer
                }
            }
            (false, None) => continue, // shouldn't happen
        };

        match state {
            SyncState::InSync => {
                // Nothing to do.
            }
            SyncState::LocalOnly | SyncState::LocalNewer => {
                // Push local.
                let store = store.as_ref().unwrap();
                let size = store.file_size()?;
                backend.upload_db(db_name, store.file_path()).await?;
                messages.push(format!("  Pushed @{} ({:.1} KB)", db_name, size as f64 / 1024.0));
            }
            SyncState::RemoteOnly | SyncState::RemoteNewer => {
                // Pull remote.
                let entry = remote_entry.unwrap();
                let local_path = paths::db_path(db_name)?;
                backend.download_db(db_name, &local_path).await?;
                messages.push(format!(
                    "  Pulled @{} ({:.1} KB)",
                    db_name,
                    entry.size_bytes as f64 / 1024.0
                ));
            }
            SyncState::Diverged => {
                messages.push(format!(
                    "  ⚠ @{} diverged — use --force push|pull to resolve",
                    db_name
                ));
            }
        }
    }

    // Upload updated manifest.
    let mut manifest = remote.unwrap_or_else(|| {
        SyncManifest::new(
            hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        )
    });

    // Update manifest with current state of synced dbs.
    for db_name in &local_dbs {
        if let Ok(store) = Store::open(db_name) {
            if let (Ok(checksum), Ok(size)) = (store.checksum(), store.file_size()) {
                manifest.upsert(db_name, checksum, size);
            }
        }
    }
    backend.put_manifest(&manifest).await?;
    cache_manifest(&manifest)?;

    if messages.is_empty() {
        Ok("Already in sync.".to_string())
    } else {
        Ok(format!("Sync complete:\n{}", messages.join("\n")))
    }
}

/// Show a diff between local and remote state.
pub async fn sync_status() -> anyhow::Result<String> {
    let backend = S3Backend::load()?;
    let remote = backend.get_manifest().await?;
    let local_dbs = paths::list_dbs()?;

    let mut all_dbs = BTreeMap::new();
    for db in &local_dbs {
        all_dbs.entry(db.clone()).or_insert(true);
    }
    if let Some(ref manifest) = remote {
        for db in manifest.databases.keys() {
            all_dbs.entry(db.clone()).or_insert(false);
        }
    }

    if all_dbs.is_empty() {
        return Ok("No databases found (local or remote).".to_string());
    }

    let mut lines = vec![String::from("Database         Status")];
    lines.push(String::from("--------         ------"));

    for (db_name, has_local) in &all_dbs {
        let remote_entry = remote.as_ref().and_then(|m| m.databases.get(db_name));

        let status = match (has_local, remote_entry) {
            (true, None) => "local only".to_string(),
            (false, Some(e)) => format!("remote only ({})", e.size_bytes),
            (true, Some(entry)) => {
                let store = Store::open(db_name).ok();
                let local_checksum = store.as_ref().and_then(|s| s.checksum().ok());
                match local_checksum {
                    Some(lc) if &lc == &entry.checksum => "in sync".to_string(),
                    Some(_) => "changed".to_string(),
                    None => "unknown".to_string(),
                }
            }
            (false, None) => continue,
        };

        lines.push(format!("@{:<15} {}", db_name, status));
    }

    Ok(lines.join("\n"))
}

/// Cache the manifest locally so we can detect conflicts offline.
fn cache_manifest(manifest: &SyncManifest) -> anyhow::Result<()> {
    let path = paths::sync_manifest_path()?;
    let json = serde_json::to_vec_pretty(manifest)?;
    std::fs::write(&path, json)?;
    Ok(())
}
