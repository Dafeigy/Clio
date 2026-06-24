# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```bash
cargo build              # debug build
cargo build --release    # optimized build (strip + LTO + size-opt)
cargo test               # run all unit + integration tests
cargo test --test integration  # run only integration tests
cargo test test_set_and_get    # run a single test by name filter
```

Integration tests spawn the binary via `cargo test` (which sets `CARGO_BIN_EXE_clio`). Pass `cargo test -- --nocapture` to see test stdout/stderr.

## Project Overview

**Clio** is a personal CLI key-value store with cross-device S3 sync — an offline-first `set/get/delete/list` tool backed by [redb](https://github.com/cberner/redb) (pure-Rust embedded DB). Sync is explicit and opt-in: `push`, `pull`, `sync`, `sync-status`. Inspired by [charmbracelet/skate](https://github.com/charmbracelet/skate).

## Architecture

```
main.rs  →  cli/  →  store/  →  redb (embedded KV)
               ↘  sync/   →  rust-s3 (network sync)
```

- **`main.rs`** — Tokio async bootstrap. Parses CLI args manually for bare-invocation help, then dispatches to CLI modules.
- **`cli/`** — Each subcommand is a module: `set`, `get`, `delete`, `list`, `db` (list-dbs/delete-db), `sync` (push/pull/sync/sync-status), `config` (init-config), `help` (custom welcome page). `mod.rs` owns the clap `Cli`/`Command` derive structs and the `parse_key()` helper.
- **`store/`** — Thin wrapper over `redb::Database`. One `.redb` file per database under `~/.local/share/clio/`. Keys are raw `&[u8]` — the `KEY@DB` parsing happens at the CLI layer.
- **`sync/`** — Three files:
  - `backend.rs` — S3 client wrapper using `rust-s3`. Config resolution priority: env vars → config file → defaults.
  - `manifest.rs` — `SyncManifest` (metadata per DB: checksum, timestamp, size) + `SyncState` enum (LocalOnly, RemoteOnly, InSync, LocalNewer, RemoteNewer, Diverged) + `compare()` diff logic.
  - `mod.rs` — High-level sync operations: `push_db`, `pull_db`, `sync_all`, `sync_status`. Sync is full-file upload/download with SHA-256 change detection.
- **`config.rs`** — Loads `~/.config/clio/config.toml` (TOML). Auto-creates a template on first access if the file doesn't exist. `init-config` command force-writes a fresh template. `CLIO_CONFIG` env var overrides the path.
- **`util/`** — `paths.rs` (XDG data dir, DB file listing, `CLIO_DATA_DIR` override), `format.rs` (terminal-aware binary-safe output with truncation).

## Key Design Decisions

- **Full-file sync** (not per-key deltas) — upload/download entire `.redb` files. Databases are expected to be small (few MB).
- **Keys are case-insensitive** — lowercased before storage, matching the reference `skate` behavior.
- **No background daemon** — sync is explicitly triggered by the user. Use cron/systemd-timer for automation.
- **Config layer priority**: `CLIO_S3_*` env vars > `~/.config/clio/config.toml` > built-in defaults. Empty strings in the config file are treated as "not set".
- **Binary handling**: `--show-binary` flag forces raw bytes on stdout. When stdout is a TTY and `--show-binary` is not set, binary values are replaced with `(omitted binary data)`.
- **Stdin support**: `clio set KEY` without a VALUE argument reads the value from stdin.

## CLI Help

The app overrides clap's default help with a custom welcome page (`cli/help.rs`) for bare invocations and `--help`/`-h` with no subcommand.
