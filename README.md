# DBRUSTY

A production-quality keyboard-driven terminal database client built with Rust and Ratatui.

Inspired by TablePlus, Lazygit, k9s, and DataGrip — designed for speed, extensibility, and a seamless terminal-native workflow.

## Status

Early development — Milestone 6 of 30. SQLite support is functional. MySQL, PostgreSQL, MongoDB, Redis, and others are on the roadmap.

## Features

- **Keyboard-first** navigation with vim-style bindings
- **Async architecture** — queries never block the UI
- **Multiple simultaneous connections** with connection pooling
- **Saved connections** with OS keychain credential storage
- **Database Explorer** tree with lazy-loading
- **Configurable** via TOML at `~/.config/dbrusty/config.toml`
- **Cross-platform** — macOS, Linux, Windows

## Quick Start

```bash
# Build and run
cargo run --release

# Or with cargo-binstall
cargo binstall dbrusty
```

### Prerequisites

- Rust 1.81+
- SQLite (for current provider)

## Usage

| Key | Action |
|-----|--------|
| `1`-`7` | Switch pages |
| `q` | Quit |
| `Tab` | Cycle focus (Sidebar / Main / Inspector) |
| `?` | Help |

### Sidebar (Database Explorer)

| Key | Action |
|-----|--------|
| `j` / `Down` | Select next |
| `k` / `Up` | Select previous |
| `l` / `Right` / `Enter` | Expand / Load children |
| `h` / `Left` | Collapse / Go to parent |
| `g` | Go to first |
| `G` | Go to last |

### Connections Page

| Key | Action |
|-----|--------|
| `n` | New connection |
| `e` | Edit connection |
| `d` | Delete connection |
| `Enter` | Connect |
| `j`/`k` | Navigate list |

## Configuration

Config file: `~/.config/dbrusty/config.toml`

```toml
[theme]
name = "default"

[editor]
tab_size = 2
auto_save = true

[logging]
level = "info"
```

## Project Structure

```
src/
  app/          — Application state, event loop, key handling
  config/       — TOML configuration loading
  database/     — Database provider trait, metadata cache, pool, types
  models/       — Connection config, data models
  providers/    — Database driver implementations (sqlite, ...)
  storage/      — Connection persistence, OS keychain
  ui/           — Layout, pages, sidebar widgets
  widgets/      — Reusable UI components (input, tree)
```

## Architecture

Dbrusty uses a clean architecture with async Tokio runtime:

```
┌─────────────────────────────────────────────────────┐
│  Ratatui TUI (Frame → Layout → Pages / Widgets)     │
├─────────────────────────────────────────────────────┤
│  App State (Event Loop, Pending Actions, Key Handler)│
├─────────────────────────────────────────────────────┤
│  DatabaseProvider Trait (abstract SQL interface)     │
├─────────────────────────────────────────────────────┤
│  SqliteProvider │ PostgresProvider │ ... (future)    │
└─────────────────────────────────────────────────────┘
```

The `DatabaseProvider` trait at `src/database/provider.rs` defines the interface. New database backends implement this trait and register with the `ConnectionPool`.

## Build

```bash
cargo build              # debug
cargo build --release    # release
```

## Test

```bash
cargo test
```

## Lint

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Deployment

### Binary

```bash
cargo build --release
cp target/release/dbrusty /usr/local/bin/
```

### Cross-platform targets

```bash
rustup target add x86_64-pc-windows-gnu   # Windows
rustup target add aarch64-apple-darwin     # Apple Silicon
rustup target add x86_64-unknown-linux-gnu # Linux
cargo build --release --target <target>
```

## Roadmap

| Milestone | Feature |
|-----------|---------|
| M1-M5 | Scaffold, core architecture, config, SQLite provider, connection UI |
| **M6** | **Database Explorer tree** |
| M7 | Query Editor |
| M8 | Query Execution & Results |
| M9 | Data Editing |
| M10 | Schema Management |
| M11+ | MySQL, PostgreSQL, MongoDB, Redis providers |

See [CLAUDE.md](CLAUDE.md) for the full 30-milestone plan.

## License

MIT
