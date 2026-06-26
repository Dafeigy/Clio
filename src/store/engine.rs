use redb::{Database, ReadableTable, TableDefinition};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use crate::util::paths;

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("kv");

/// A local key-value store backed by a single `redb` file.
pub struct Store {
    db: Database,
    #[allow(dead_code)]
    name: String,
    path: PathBuf,
}

impl Store {
    /// Open (or create) a named database.
    ///
    /// If an existing database file is corrupted (e.g. from a previous crash),
    /// it is deleted and recreated with a warning. This handles redb panics
    /// that can occur when the file has a valid magic number but corrupt layout.
    pub fn open(name: &str) -> anyhow::Result<Self> {
        let name = if name.is_empty() { "default" } else { name };
        let path = paths::db_path(name)?;

        let db = Self::open_db(&name, &path)?;

        // Ensure the table exists.
        {
            let write_txn = db.begin_write()?;
            {
                let _table = write_txn.open_table(TABLE)?;
            }
            write_txn.commit()?;
        }

        Ok(Self { db, name: name.to_string(), path })
    }

    /// Try to open the database file, recovering from corruption if needed.
    fn open_db(name: &str, path: &PathBuf) -> anyhow::Result<Database> {
        if path.exists() {
            // Try opening the existing file. redb may panic if the file has a
            // valid magic number but corrupt layout data (e.g. crash during init).
            match Self::try_open(path) {
                Ok(db) => return Ok(db),
                Err(e) => {
                    // If this was a redb panic (not a normal error), the message
                    // is generic; log a clear warning either way.
                    eprintln!(
                        "Warning: database @{name} at {} appears corrupted, recreating it. ({e})",
                        path.display()
                    );
                    std::fs::remove_file(path)?;
                }
            }
        }

        // Fresh create (file doesn't exist or was just removed).
        match Self::try_open(path) {
            Ok(db) => Ok(db),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to create database @{name} at {}: {e}",
                path.display()
            )),
        }
    }

    /// Call `Database::create` (which opens-or-creates), catching panics.
    fn try_open(path: &PathBuf) -> Result<Database, anyhow::Error> {
        let path = path.clone();
        match panic::catch_unwind(AssertUnwindSafe(|| Database::create(&path))) {
            Ok(Ok(db)) => Ok(db),
            Ok(Err(e)) => Err(e.into()),
            Err(panic_payload) => {
                let msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "redb panicked".to_string()
                };
                Err(anyhow::anyhow!("{msg}"))
            }
        }
    }

    /// Returns the database name.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the file path of the underlying database.
    pub fn file_path(&self) -> &PathBuf {
        &self.path
    }

    /// Set a key to a value.
    pub fn set(&self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get the value for a key.
    pub fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        let value = table.get(key)?.map(|v| v.value().to_vec());
        Ok(value)
    }

    /// Delete a key.
    pub fn delete(&self, key: &[u8]) -> anyhow::Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.remove(key)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Return an iterator over all key-value pairs.
    pub fn iter(&self, reverse: bool) -> anyhow::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = table
            .iter()?
            .flatten()
            .map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
            .collect();

        if reverse {
            pairs.reverse();
        }

        Ok(pairs)
    }

    /// Return an iterator over keys only (avoids reading values from disk).
    pub fn iter_keys(&self, reverse: bool) -> anyhow::Result<Vec<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        let mut keys: Vec<Vec<u8>> = table
            .iter()?
            .flatten()
            .map(|(k, _)| k.value().to_vec())
            .collect();

        if reverse {
            keys.reverse();
        }

        Ok(keys)
    }

    /// Flush pending writes to disk.
    /// Note: redb's `compact()` re-keys the database to minimize file size.
    pub fn flush(&self) -> anyhow::Result<()> {
        // redb commits are durable by default via fsync.
        // Calling compact() hedges file growth, but requires &mut.
        // For read-only flush (e.g., before iterating), just sync is enough.
        drop(self.db.begin_read()?); // Ensure any pending work is visible
        Ok(())
    }

    /// Compute the SHA-256 checksum of the database file.
    pub fn checksum(&self) -> anyhow::Result<String> {
        use sha2::{Digest, Sha256};
        let data = std::fs::read(&self.path)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Returns the size of the database file in bytes.
    pub fn file_size(&self) -> anyhow::Result<u64> {
        let meta = std::fs::metadata(&self.path)?;
        Ok(meta.len())
    }
}
