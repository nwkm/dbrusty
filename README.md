# DBRUSTY

A production-quality keyboard-driven terminal database client built with Rust and Ratatui.

Inspired by TablePlus, Lazygit, k9s, and DataGrip — designed for speed, extensibility, and a seamless terminal-native workflow.

## Status

**Current milestone: M6/30 — Database Explorer & Query Editor.** PostgreSQL and SQLite are fully supported. MySQL, MariaDB, SQL Server, MongoDB, and Redis providers are defined but not yet implemented (the architecture is ready — implement the trait).

## Features

### Core
- **Keyboard-first** navigation with mouse support
- **Async architecture** — queries never block the UI
- **Multiple simultaneous connections** with connection pooling
- **OS keychain** credential storage (macOS Keychain, Linux Secret Service, Windows Credential Manager)
- **TOML configuration** at `~/.config/dbrusty/config.toml`
- **Structured logging** with in-app log viewer
- **Cross-platform** — macOS, Linux, Windows

### Database Providers
| Provider | Status |
|----------|--------|
| PostgreSQL | Full support (tables, columns, indexes, views, sequences, triggers, functions, procedures, extensions, custom types, transactions, server info) |
| SQLite | Full support (tables, columns, indexes, views, transactions, server info) |
| MySQL | Not yet implemented |
| MariaDB | Not yet implemented |
| SQL Server | Not yet implemented |
| MongoDB | Not yet implemented |
| Redis | Not yet implemented |

### Database Explorer
- Tree view: Server → Databases → Schemas → Tables/Views/Indexes/Functions/Procedures/Triggers/Sequences
- Lazy-loading columns on expand
- Expand/collapse with keyboard and mouse double-click
- Detail panel showing column info (type, PK, nullable, default)

### Query Editor
- Split layout: editor (25%) / results (75%)
- **Table double-click** in Explorer automatically generates and runs `SELECT * FROM "table" LIMIT 1000 OFFSET 0`
- Text editing with cursor navigation (Home, End, arrows)
- Results display with column headers, row highlighting, and scrollable viewport
- Cell selection with arrow keys / Tab
- Cell editing via popup dialog
- Copy cell value to clipboard (Y key)
- Horizontal (Ctrl+B/F) and vertical (Ctrl+U/D) scrolling

### Inspector Pane
- Column-by-column detail of selected row with type badges
- Inline editing (click value, Enter save, Esc cancel, Alt+Z undo)
- Toggleable visibility (Alt+H)
- Keyboard navigation (j/k scroll, g/G top/bottom, Home/End in edit)

### Connections
- Connection list with database-type icons and status indicators
- Add/Edit/Delete connections
- Connection form with 7 fields and database type cycling
- Password saved to OS keychain on connect
- Double-click to connect

### Pages
| Key | Page | Status |
|-----|------|--------|
| 1 | Dashboard | ✓ |
| 2 | Connections | ✓ |
| 3 | Database Explorer | ✓ |
| 4 | Query Editor | ✓ |
| 5 | Query History | Placeholder |
| 6 | Favorites | Placeholder |
| 7 | Schema Browser | Placeholder |
| 8 | Logs | ✓ |

## Quick Start

```bash
# Build and run
cargo run --release
```

### Prerequisites

- Rust 1.81+
- PostgreSQL or SQLite database (or use the built-in connection form to configure)

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1`–`8` | Switch pages |
| `q` | Quit |
| `?` | Help |
| `Tab` | Cycle focus forward |
| `Shift+Tab` | Cycle focus reverse |
| `Alt+H` | Toggle Inspector pane |

### Sidebar (Database Explorer)

| Key | Action |
|-----|--------|
| `j` / `Down` | Select next |
| `k` / `Up` | Select previous |
| `l` / `Right` | Expand / Load children |
| `h` / `Left` | Collapse / Go to parent |
| `g` | Go to first |
| `G` | Go to last |
| Double-click | Expand node / Load table columns / Run SELECT query on table |

### Query Editor — Results

| Key | Action |
|-----|--------|
| `Right` | Next column |
| `Left` | Previous column |
| `Up` / `Down` | Navigate rows |
| `Enter` | Edit selected cell |
| `Y` | Copy cell value |
| `Esc` | Clear selection / Focus editor |
| `Ctrl+R` | Run query |
| `Ctrl+E` | Edit query text |
| `Ctrl+B` / `Ctrl+F` | Scroll results left/right |
| `Ctrl+U` / `Ctrl+D` | Scroll results up/down |


### Inspector Pane

| Key | Action |
|-----|--------|
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `g` | Scroll to top |
| `G` | Scroll to bottom |
| Click value | Enter edit mode |
| Type / Backspace / Delete | Edit value |
| Left / Right / Home / End | Cursor navigation |
| `Enter` | Save edit |
| `Esc` | Cancel edit |
| `Alt+Z` | Undo last saved edit |

### Connections Page

| Key | Action |
|-----|--------|
| `j` / `Down` | Select next |
| `k` / `Up` | Select previous |
| `g` | Go to first |
| `G` | Go to last |
| `n` | New connection |
| `e` | Edit connection |
| `d` | Delete connection |
| `Enter` | Connect / Confirm delete |

### Connection Form

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle fields |
| `Left` / `Right` | Change DB type (on Type field) |
| `Enter` | Submit |
| `Esc` | Cancel |

## Configuration

Config file: `~/.config/dbrusty/config.toml`

```toml
theme = "dark"
default_fetch_size = 1000
autosave_interval = 300
log_level = "info"
```

## Project Structure

```
src/
  app/          — Application state, event loop, key/mouse handling
  config/       — TOML configuration loading
  database/     — DatabaseProvider trait, metadata cache, pool, data types
  events/       — AppEvent enum (reserved for future event system)
  logging/      — Tracing initialization
  models/       — Connection config, data models
  providers/    — Database driver implementations (postgres/, sqlite/)
  storage/      — Connection persistence, OS keychain
  ui/           — Layout, pages (dashboard, connections, explorer, query_editor, ...), sidebar widgets
  widgets/      — Reusable UI components (input field, tree widget)
```

## Architecture

Dbrusty uses a clean architecture with async Tokio runtime:

```
┌──────────────────────────────────────────────────────────┐
│  Ratatui TUI (Frame → Layout → Pages / Widgets)          │
├──────────────────────────────────────────────────────────┤
│  App State (Event Loop, Pending Actions, Key/Mouse Handler)│
├──────────────────────────────────────────────────────────┤
│  DatabaseProvider Trait (abstract SQL/NOSQL interface)    │
├──────────────────────────────────────────────────────────┤
│  PostgresProvider │ SqliteProvider │ ... (future)         │
└──────────────────────────────────────────────────────────┘
```

The `DatabaseProvider` trait at `src/database/provider.rs` defines the interface. New database backends implement this trait and register with the `ConnectionPool`.

## Build

```bash
cargo build              # debug
cargo build --release    # release
```

## Test

```bash
# Unit tests
cargo test

# PostgreSQL integration tests (requires DATABASE_USERNAME env)
DATABASE_USERNAME=postgres cargo test --test postgres -- --ignored
```

## Lint

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Roadmap

| Milestone | Feature | Status |
|-----------|---------|--------|
| M1–M5 | Scaffold, core architecture, config, SQLite provider, connection UI | ✓ |
| M6 | Database Explorer tree & Query Editor | **Current** |
| M7 | Query execution & results viewer | ✓ (merged into M6) |
| M8 | Data editing (grid edit, transactions) | Partial |
| M9 | Schema management (CREATE/ALTER/DROP) | |
| M10 | MySQL provider | |
| M11 | PostgreSQL provider | ✓ (merged into M6) |
| M12 | MongoDB provider | |
| M13 | Redis provider | |
| M14 | Global search | |
| M15 | Import / Export (CSV, JSON, SQL) | |
| M16 | Query history with search & tags | |
| M17 | Bookmarks & favorites | |
| M18 | Themes (dark, light, custom) | |
| M19 | Configurable keybindings | |
| M20 | Security (SSH tunnel, SSL/TLS) | |
| M21 | Performance (virtual scrolling, caching) | |
| M22 | Plugin system | |
| M23 | Testing & CI/CD | |
| M24 | Packaging (AppImage, DMG, MSI) | |
| M25 | Documentation & user guide | |
| M26+ | Stable release | |

See [CHANGELOG.md](CHANGELOG.md) for detailed per-release notes.

## License

MIT
