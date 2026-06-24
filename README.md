# clio

> A personal key-value store with cross-device S3 sync. Built in Rust, inspired by [charmbracelet/skate](https://github.com/charmbracelet/skate).

## Features

- **Simple CLI** — `set`, `get`, `delete`, `list` with `KEY@DB` namespacing
- **Embedded storage** — powered by [redb](https://github.com/cberner/redb), a pure-Rust ACID-compliant KV store
- **Cross-device sync** — push/pull/sync databases via any S3-compatible storage (AWS S3, Cloudflare R2, MinIO, Backblaze B2)
- **Pipe-friendly** — read values from stdin, write to stdout; works great in shell pipelines
- **Offline-first** — all reads/writes hit local storage; sync is explicit and opt-in

## Installation

```bash
cargo install --path .
sudo cp ~/.cargo/bin/clio /usr/bin/
```

Or build from source:

```bash
git clone https://github.com/cybersh1t/clio.git
cd clio
cargo build --release
# binary at target/release/clio
```

## Quick Start

```bash
# Store a value
clio set api-key sk-abc123

# Store in a named database
clio set api-key@work sk-xyz789

# Retrieve a value
clio get api-key

# Pipe a file into a key
cat ~/.ssh/id_rsa.pub | clio set ssh-key

# List all keys in the default database
clio list

# List all databases
clio list-dbs

# Delete a key
clio delete old-key@work
```

## Key Format

```
KEY@DB
```

- `KEY` — case-insensitive, stored lowercased
- `@DB` — optional database selector; defaults to `"default"`
- Examples: `foo`, `api-key@secrets`, `config@work`

## Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `clio set KEY [VALUE]` | `put` | Set a key; reads stdin if VALUE omitted |
| `clio get KEY` | — | Get a key's value |
| `clio delete KEY` | `del`, `rm` | Delete a key |
| `clio list [@DB]` | `ls` | List key-value pairs |
| `clio list-dbs` | `ls-db` | List all databases |
| `clio delete-db @DB` | `del-db`, `rm-db` | Delete an entire database |
| `clio push [@DB]` | — | Upload local DB to S3 |
| `clio pull [@DB]` | — | Download remote DB from S3 |
| `clio sync` | — | Bidirectional sync of all DBs |
| `clio sync-status` | — | Show local vs remote diff |
| `clio init-config` | — | Create a config file template |

### List Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--reverse` | `-r` | Reverse lexicographic order |
| `--keys-only` | `-k` | Only print keys |
| `--values-only` | `-v` | Only print values |
| `--delimiter` | `-d` | Delimiter (default: tab) |
| `--show-binary` | `-b` | Print binary values |

## Cross-Device Sync

Clio syncs databases to S3-compatible object storage. Each database is stored as a single file; the sync protocol is full-file upload/download with SHA-256 change detection.

### Configuration

Create a config file with `clio init-config`:

```bash
clio init-config
# → ~/.config/clio/config.toml
```

Then edit the file and uncomment the fields you need:

```toml
# ~/.config/clio/config.toml
[s3]
endpoint = "https://s3.amazonaws.com"   # or your S3-compatible endpoint
bucket = "my-clio-data"
#prefix = "clio/"                        # optional, default: clio/
#region = "us-east-1"                    # optional, default: us-east-1
access_key = "AKIA..."
secret_key = "..."
```

Alternatively, you can still use environment variables (which take precedence over the config file):

```bash
export CLIO_S3_ENDPOINT="https://s3.amazonaws.com"
export CLIO_S3_BUCKET="my-clio-data"
export CLIO_S3_ACCESS_KEY="AKIA..."
export CLIO_S3_SECRET_KEY="..."
```

### Usage

```bash
# Push a database to S3
clio push work

# Pull a database from S3 (overwrites local)
clio pull work

# Bidirectional sync (push local-only, pull remote-only)
clio sync

# Show what's changed
clio sync-status
```

### Conflict Resolution

If both local and remote have changed since the last sync:

```bash
clio push work --force   # overwrite remote with local
clio pull work --force   # overwrite local with remote
```

## Data Location

Databases are stored as `.redb` files:

```
~/.local/share/clio/
├── default.redb
├── work.redb
├── secrets.redb
└── .sync-manifest.json
```

Override with `CLIO_DATA_DIR`:

```bash
export CLIO_DATA_DIR=/path/to/custom/dir
```

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full architecture design, crate layout, and design decisions.

## License

MIT
