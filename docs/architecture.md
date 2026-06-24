# Clio — Architecture Design

> A cross-device-synced key-value store CLI tool built in Rust, inspired by [charmbracelet/skate](https://github.com/charmbracelet/skate).

---

## 1. Overview

Clio is a personal key-value store accessible from the command line. It stores data locally in an embedded database and supports **cross-device synchronization** via S3-compatible object storage. The design is based on the Go-based `skate` CLI but reimagined in Rust with sync as a first-class feature.

### Core Principles

- **Offline-first** — all reads/writes hit local storage; sync is explicit
- **Simple mental model** — `KEY@DB` namespacing, case-insensitive keys
- **Composable** — pipe values in/out via stdin/stdout
- **Sync is opt-in** — no daemon, no background tasks; the user controls when sync happens

---

## 2. Command Surface

```
clio set KEY[@DB] [VALUE]         # Set a key; reads stdin if VALUE omitted
clio get KEY[@DB]                 # Get a key's value, prints to stdout
clio delete KEY[@DB]              # Delete a key (aliases: del, rm)
clio list [@DB]                   # List key-value pairs
clio list-dbs                     # List all databases (aliases: ls-db)
clio delete-db @DB                # Delete an entire database (aliases: del-db, rm-db)

clio push [@DB]                   # Upload local DB to S3
clio pull [@DB]                   # Download remote DB from S3
clio sync                         # Bidirectional sync of all DBs
clio sync-status                  # Show sync status (local vs remote diffs)
```

### Key Format

```
KEY@DB
```

- `KEY` — case-insensitive, stored lowercased
- `@DB` — optional database selector; defaults to `"default"`
- Examples: `foo@work`, `api-key@secrets`, `foo` (uses default DB)

### List Flags (matching skate)

| Flag | Short | Description |
|------|-------|-------------|
| `--reverse` | `-r` | Reverse lexicographic order |
| `--keys-only` | `-k` | Only print keys |
| `--values-only` | `-v` | Only print values |
| `--delimiter` | `-d` | Delimiter between key and value (default: `\t`) |
| `--show-binary` | `-b` | Print binary values instead of omitting them |

---

## 3. Crate Architecture

```
src/
├── main.rs              # Entry point, tokio runtime bootstrap
├── cli/                 # CLI layer (clap derive)
│   ├── mod.rs           # Re-exports, shared CLI utilities
│   ├── set.rs           # set command
│   ├── get.rs           # get command
│   ├── delete.rs        # delete command
│   ├── list.rs          # list command
│   ├── db.rs            # list-dbs, delete-db commands
│   └── sync.rs          # push, pull, sync, sync-status commands
├── store/               # Local storage abstraction
│   ├── mod.rs           # Store trait + open/close logic
│   └── engine.rs        # redb-backed implementation
├── sync/                # Cross-device sync
│   ├── mod.rs           # Sync orchestrator (push/pull/sync logic)
│   ├── backend.rs       # S3 client wrapper (rust-s3)
│   └── manifest.rs      # Manifest data structures + diff logic
└── util/                # Shared utilities
    ├── mod.rs
    ├── paths.rs         # XDG data directory resolution
    └── format.rs        # Terminal-aware output formatting (binary detection)
```

### Dependency Graph

```
cli ──────► store ──────► redb
  │           │
  │           ▼
  └───────► sync ───────► rust-s3, sha2, serde_json
              │
              ▼
           util ─────────► dirs
```

### Crate Dependencies

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Local storage
redb = "2"

# S3 sync
rust-s3 = { version = "0.34", features = ["sync", "tokio"] }  # or aws-sdk-s3
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Hashing (manifest checksums)
sha2 = "0.10"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Paths
dirs = "5"

# Error handling
anyhow = "1"
thiserror = "2"

# Terminal detection (for binary output handling)
is-terminal = "0.4"
```

---

## 4. Storage Layer

### 4.1 Local Engine: `redb`

Each database (`@work`, `@default`, etc.) maps to a single `.redb` file on disk:

```
~/.local/share/clio/
├── default.redb
├── work.redb
├── secrets.redb
└── .sync-manifest.json        # local copy of last-known sync state
```

**Why `redb` over alternatives:**

| Candidate | Verdict |
|-----------|---------|
| `sled` | Semi-abandoned (last release 2021), known data-corruption bugs |
| `rocksdb` | C++ dependency, heavy build, overkill for CLI KV store |
| `lmdb` | C dependency, mmap-based (file size concerns) |
| `sqlite` | Relational overhead for pure KV; schema migration burden |
| **`redb`** | Pure Rust, actively maintained, ACID, zero-copy reads, Copy-on-Write B-tree |

### 4.2 Store API

```rust
// store/mod.rs
pub struct Store {
    db: redb::Database,
    name: String,  // database name, e.g. "default"
}

impl Store {
    /// Open (or create) a named database at the XDG data path.
    pub fn open(name: &str) -> Result<Self>;

    /// Set a key to a value.
    pub fn set(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Get the value for a key.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Delete a key.
    pub fn delete(&self, key: &[u8]) -> Result<()>;

    /// Iterate over all key-value pairs.
    pub fn iter(&self, reverse: bool) -> impl Iterator<Item = (Vec<u8>, Vec<u8>)>;

    /// Iterate over keys only (no value fetch — performance optimization).
    pub fn iter_keys(&self, reverse: bool) -> impl Iterator<Item = Vec<u8>>;

    /// Return the file path of the underlying database.
    pub fn file_path(&self) -> &Path;

    /// Flush pending writes to disk.
    pub fn flush(&self) -> Result<()>;
}
```

**Key design notes:**

- Keys are stored as raw bytes — the `KEY@DB` parsing happens at the CLI layer, the store is pure `&[u8] → &[u8]`
- The `default` table in redb acts as the single keyspace (no secondary tables needed)
- `iter_keys` avoids reading values from disk for the `--keys-only` flag

---

## 5. Sync Architecture

### 5.1 Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Conflict resolution | **Last-Write-Wins (LWW)** | Simple, predictable, no merge logic |
| Sync granularity | **Per-database file** | A DB is the unit of sync; no per-key diffs |
| Protocol | **Object storage (S3)** | Already available, no server to run |
| Transport | **S3 PutObject / GetObject** | Simplest possible; full-file upload/download |
| Change detection | **SHA-256 checksum + timestamp** | Avoids byte-by-byte comparison |

### 5.2 S3 Object Layout

```
s3://<bucket>/<prefix>/
├── manifest.json           # Global sync manifest
├── default.redb            # Database files (one per DB)
├── work.redb
└── secrets.redb
```

### 5.3 Manifest Format

```json
{
  "version": 1,
  "device_id": "laptop-01",
  "databases": {
    "default": {
      "last_modified": "2024-06-24T10:30:00Z",
      "checksum": "sha256:1a2b3c...",
      "size_bytes": 4096
    },
    "work": {
      "last_modified": "2024-06-23T18:00:00Z",
      "checksum": "sha256:d4e5f6...",
      "size_bytes": 8192
    }
  }
}
```

Each database entry records:
- `last_modified` — ISO 8601 timestamp (set by the uploading device)
- `checksum` — SHA-256 of the `.redb` file (for integrity + change detection)
- `size_bytes` — file size in bytes

### 5.4 Sync Operations

#### `clio push [@DB]`

```
1. Compute local checksum of <db>.redb
2. Fetch remote manifest.json from S3
3. Compare: if local == remote checksum → "already in sync", exit
4. If remote is newer AND local changed → conflict (warn, abort unless --force)
5. Upload <db>.redb to S3
6. Update manifest with new checksum + timestamp
7. Upload manifest.json to S3
```

#### `clio pull [@DB]`

```
1. Fetch remote manifest.json from S3
2. Download <db>.redb from S3
3. Verify checksum against manifest
4. Replace local <db>.redb
5. Update local .sync-manifest.json
```

#### `clio sync`

```
1. Fetch remote manifest
2. For each database known locally and/or remotely:
   a. If only local → push
   b. If only remote → pull
   c. If both exist:
      - same checksum → skip
      - local newer → push
      - remote newer → pull
3. Report summary of actions taken
```

#### `clio sync-status`

```
1. Fetch remote manifest
2. For each database, compute diff:
   - "local only" / "remote only" / "diverged" / "in sync"
3. Print a table showing status
```

### 5.5 Conflict Handling

Conflicts occur when both local and remote have changed since the last sync:

```
clio sync
  ⚠ work: both local and remote have changed (diverged)
  Use --force push|pull to resolve
```

Resolution:
- `--force push` — overwrite remote with local
- `--force pull` — overwrite local with remote
- Future: `--merge` could do per-key LWW if database internals are exposed

### 5.6 S3 Backend Abstraction

```rust
// sync/backend.rs
pub struct S3Backend {
    bucket: String,
    prefix: String,
    client: Bucket,  // rust-s3 Bucket
}

impl S3Backend {
    pub fn from_env() -> Result<Self>;
    // Reads:
    //   CLIO_S3_ENDPOINT   (default: s3.amazonaws.com)
    //   CLIO_S3_BUCKET
    //   CLIO_S3_PREFIX     (default: "clio/")
    //   CLIO_S3_REGION     (default: us-east-1)
    //   CLIO_S3_ACCESS_KEY
    //   CLIO_S3_SECRET_KEY

    pub async fn get_manifest(&self) -> Result<SyncManifest>;
    pub async fn put_manifest(&self, manifest: &SyncManifest) -> Result<()>;
    pub async fn download_db(&self, name: &str, dest: &Path) -> Result<()>;
    pub async fn upload_db(&self, name: &str, source: &Path) -> Result<()>;
}
```

---

## 6. Data Flow

### 6.1 `set` command

```
User: clio set api-key@secrets sk-abc123
         │
         ▼
    cli/set.rs: parse "api-key@secrets" → key=b"api-key", db="secrets"
         │
         ▼
    store::Store::open("secrets") → opens ~/.local/share/clio/secrets.redb
         │
         ▼
    store.set(b"api-key", b"sk-abc123")
         │
         ▼
    redb: WriteTransaction → insert into default table → commit
```

### 6.2 `get` command

```
User: clio get api-key@secrets
         │
         ▼
    cli/get.rs: parse → key=b"api-key", db="secrets"
         │
         ▼
    store::Store::open("secrets") → read-only transaction
         │
         ▼
    store.get(b"api-key") → Some(b"sk-abc123")
         │
         ▼
    stdout: "sk-abc123\n"   (no newline if piped)
```

### 6.3 `push` command

```
User: clio push secrets
         │
         ▼
    cli/sync.rs: push("secrets")
         │
         ▼
    sync/mod.rs: push_db("secrets")
         │
    ┌────────────────────────────────────┐
    │ 1. Compute local SHA-256            │
    │    of ~/.local/share/clio/          │
    │    secrets.redb                     │
    │                                     │
    │ 2. S3Backend::get_manifest()        │
    │    → GET s3://bucket/prefix/        │
    │      manifest.json                  │
    │                                     │
    │ 3. Compare checksums               │
    │    same → "in sync", exit           │
    │                                     │
    │ 4. Upload:                          │
    │    PUT s3://bucket/prefix/          │
    │    secrets.redb                     │
    │                                     │
    │ 5. Update manifest, upload          │
    └────────────────────────────────────┘
         │
         ▼
    stdout: "Pushed 'secrets' (4.0 KB)"
```

---

## 7. Implementation Phases (Action Plan)

### Phase 0: Project Scaffold

| Step | Task | Est. |
|------|------|------|
| 0.1 | Set up `Cargo.toml` with all dependencies | S |
| 0.2 | Create module structure (`cli/`, `store/`, `sync/`, `util/`) | S |
| 0.3 | Implement `util/paths.rs` — XDG data directory resolution | S |
| 0.4 | Implement `util/format.rs` — terminal-aware printing | S |

**Milestone:** Project compiles, `clio --help` works (empty commands).

---

### Phase 1: Local KV Store

| Step | Task | Est. |
|------|------|------|
| 1.1 | Implement `store/engine.rs` — `Store` struct with `open`, `set`, `get`, `delete` | M |
| 1.2 | Implement `Store::iter` and `Store::iter_keys` | S |
| 1.3 | Implement `cli/set.rs` — `set` command with stdin support | M |
| 1.4 | Implement `cli/get.rs` — `get` command | S |
| 1.5 | Implement `cli/delete.rs` — `delete` command | S |
| 1.6 | Implement `cli/list.rs` — `list` with all flags | M |
| 1.7 | Implement `cli/db.rs` — `list-dbs`, `delete-db` | M |
| 1.8 | Implement fuzzy-matching suggestions for `delete-db` | S |

**Milestone:** Full local KV store matching skate's feature set.

---

### Phase 2: S3 Sync

| Step | Task | Est. |
|------|------|------|
| 2.1 | Implement `sync/manifest.rs` — manifest types, serialize/deserialize, diff | M |
| 2.2 | Implement `sync/backend.rs` — S3 client with env-based config | M |
| 2.3 | Implement `sync/mod.rs` — `push_db` logic | M |
| 2.4 | Implement `sync/mod.rs` — `pull_db` logic | M |
| 2.5 | Implement `sync/mod.rs` — `sync_all` (bidirectional) | M |
| 2.6 | Implement `sync/mod.rs` — `sync_status` (diff report) | S |
| 2.7 | Implement `cli/sync.rs` — CLI commands for push/pull/sync/sync-status | M |
| 2.8 | Add `--force` flag to push/pull for conflict resolution | S |

**Milestone:** Cross-device sync working end-to-end.

---

### Phase 3: Polish & Robustness

| Step | Task | Est. |
|------|------|------|
| 3.1 | Integration tests for all CLI commands | L |
| 3.2 | Unit tests for sync manifest logic | M |
| 3.3 | Error messages: user-friendly output on S3 auth failures | S |
| 3.4 | Add `clio config` command for setting S3 credentials interactively | M |
| 3.5 | Binary release build configuration (strip, LTO, etc.) | S |
| 3.6 | README with installation and usage docs | M |

---

### Size Guide

| Label | Meaning |
|-------|---------|
| S | Small — <1 hour |
| M | Medium — 1-3 hours |
| L | Large — 3-6 hours |

---

## 8. Configuration & Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CLIO_S3_ENDPOINT` | S3-compatible endpoint URL | `https://s3.amazonaws.com` |
| `CLIO_S3_BUCKET` | Bucket name | *(required)* |
| `CLIO_S3_PREFIX` | Object key prefix within bucket | `clio/` |
| `CLIO_S3_REGION` | AWS / S3 region | `us-east-1` |
| `CLIO_S3_ACCESS_KEY` | Access key ID | *(from AWS SDK cred chain)* |
| `CLIO_S3_SECRET_KEY` | Secret access key | *(from AWS SDK cred chain)* |

**S3-compatible services tested against:**
- AWS S3
- Cloudflare R2
- MinIO (self-hosted)
- Backblaze B2

---

## 9. Design Decisions Log

### DD-001: Async vs Sync CLI

**Decision:** Use `tokio` async runtime at the `main` level.

**Rationale:** S3 network calls are inherently async. While the local store is sync (`redb` is blocking), we can call `spawn_blocking` for DB operations from the async context. This avoids mixing blocking and non-blocking I/O in the same thread.

**Alternative considered:** Use `ureq` for blocking HTTP + sync throughout. Rejected because `rust-s3`'s async API is more mature, and tokio is the Rust ecosystem standard.

### DD-002: Full-file sync vs per-key delta sync

**Decision:** Full-file sync (upload/download the entire `.redb` file).

**Rationale:**
- A personal KV store is unlikely to exceed a few MB
- Per-key delta sync requires understanding redb's internal format, which is fragile
- Simplicity: fewer bugs, easier reasoning about consistency
- S3 PUT/GET of small files is fast and cheap

**Future consideration:** If databases grow large, consider redb's WAL or a changelog-based incremental sync.

### DD-003: No background sync daemon

**Decision:** Sync is triggered explicitly via CLI commands only.

**Rationale:** Matches the user's existing workflow with skate. No daemon means no system service to install, no persistent resource consumption, no security surface for long-running processes. Users who want periodic sync can use `cron` or `systemd-timer`.

### DD-004: Case-insensitive keys

**Decision:** Keys are lowercased before storage, matching skate's behavior.

**Rationale:** Consistency with the reference implementation. Users don't need to remember exact casing.

---

## 10. Open Questions / Future Work

- **Encryption at rest?** — Could add `age`-based encryption before S3 upload (`clio push --encrypt`)
- **Key expiration / TTL?** — `clio set foo bar --ttl 24h`
- **JSON output?** — `clio list --json` for scripting
- **Watch mode?** — `clio watch @DB` to tail changes (like `watch ls`)
- **Multi-region sync?** — Push to multiple S3 endpoints for redundancy
