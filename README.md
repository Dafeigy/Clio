# styx

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
```

Or build from source:

```bash
git clone https://github.com/cybersh1t/styx.git
cd styx
cargo build --release
# binary at target/release/styx
```

## Quick Start

```bash
# Store a value
styx set api-key sk-abc123

# Store in a named database
styx set api-key@work sk-xyz789

# Retrieve a value
styx get api-key

# Pipe a file into a key
cat ~/.ssh/id_rsa.pub | styx set ssh-key

# List all keys in the default database
styx list

# List all databases
styx list-dbs

# Delete a key
styx delete old-key@work
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
| `styx set KEY [VALUE]` | `put` | Set a key; reads stdin if VALUE omitted |
| `styx get KEY` | — | Get a key's value |
| `styx delete KEY` | `del`, `rm` | Delete a key |
| `styx list [@DB]` | `ls` | List key-value pairs |
| `styx list-dbs` | `ls-db` | List all databases |
| `styx delete-db @DB` | `del-db`, `rm-db` | Delete an entire database |
| `styx push [@DB]` | — | Upload local DB to S3 |
| `styx pull [@DB]` | — | Download remote DB from S3 |
| `styx sync` | — | Bidirectional sync of all DBs |
| `styx sync-status` | — | Show local vs remote diff |
| `styx init-config` | — | Create a config file template |

### List Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--reverse` | `-r` | Reverse lexicographic order |
| `--keys-only` | `-k` | Only print keys |
| `--values-only` | `-v` | Only print values |
| `--delimiter` | `-d` | Delimiter (default: tab) |
| `--show-binary` | `-b` | Print binary values |

## Cross-Device Sync

Styx syncs databases to S3-compatible object storage. Each database is stored as a single file; the sync protocol is full-file upload/download with SHA-256 change detection.

### Configuration

Create a config file with `styx init-config`:

```bash
styx init-config
# → ~/.config/styx/config.toml
```

Then edit the file and uncomment the fields you need:

```toml
# ~/.config/styx/config.toml
[s3]
endpoint = "https://s3.amazonaws.com"   # or your S3-compatible endpoint
bucket = "my-styx-data"
#prefix = "styx/"                        # optional, default: styx/
#region = "us-east-1"                    # optional, default: us-east-1
access_key = "AKIA..."
secret_key = "..."
```

Alternatively, you can still use environment variables (which take precedence over the config file):

```bash
export STYX_S3_ENDPOINT="https://s3.amazonaws.com"
export STYX_S3_BUCKET="my-styx-data"
export STYX_S3_ACCESS_KEY="AKIA..."
export STYX_S3_SECRET_KEY="..."
```

### Usage

```bash
# Push a database to S3
styx push work

# Pull a database from S3 (overwrites local)
styx pull work

# Bidirectional sync (push local-only, pull remote-only)
styx sync

# Show what's changed
styx sync-status
```

### Conflict Resolution

If both local and remote have changed since the last sync:

```bash
styx push work --force   # overwrite remote with local
styx pull work --force   # overwrite local with remote
```

## Data Location

Databases are stored as `.redb` files:

```
~/.local/share/styx/
├── default.redb
├── work.redb
├── secrets.redb
└── .sync-manifest.json
```

Override with `STYX_DATA_DIR`:

```bash
export STYX_DATA_DIR=/path/to/custom/dir
```

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full architecture design, crate layout, and design decisions.

## License

MIT
