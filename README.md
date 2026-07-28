# DBRUSTY

[![Built with Ratatui](https://img.shields.io/badge/built_with-Ratatui-red.svg)](https://ratatui.rs)
[![Crate](https://img.shields.io/crates/v/dbrusty.svg)](https://crates.io/crates/dbrusty)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/nawin/dbrusty)](https://github.com/nawin/dbrusty/releases)

A production-quality keyboard-driven terminal database client built with Rust and Ratatui.

Inspired by TablePlus, Lazygit, k9s, and DataGrip — designed for speed, extensibility, and a seamless terminal-native workflow.

## Status

**Current milestone: M15/29 — SQL Server provider.** PostgreSQL, SQLite, MongoDB, MySQL, MariaDB, and Redis are all fully supported. SQL Server is defined but not yet implemented.

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
| PostgreSQL | Full support (tables, columns, indexes, views, sequences, triggers, functions, procedures, extensions, custom types, transactions, server info, chrono types) |
| SQLite | Full support (tables, columns, indexes, views, transactions, server info) |
| MongoDB | Full support (connect/ping, CRUD — insert/find/update/delete, aggregate, count, distinct, collStats, explain, collection/index metadata, explorer tree integration, JSON document viewer with inline editing, flat document list tree view) |
| MySQL | Full support (connect/ping, metadata queries, CRUD, transactions, server info) |
| MariaDB | Full support (connect/ping, metadata queries, CRUD, transactions, server info, sequences, custom types) |
| SQL Server | Not yet implemented |
| Redis | Full support (key browser by type, lazy-loaded via SCAN, TTL display, raw command execution, GET/HGETALL/LRANGE/SMEMBERS/ZRANGE/XRANGE/INFO parsing, connections page + explorer tree integration) |

### Database Explorer
- Tree view: Server → Databases → Schemas → Tables/Views/Indexes/Functions/Procedures/Triggers/Sequences
- Lazy-loading columns on expand
- Expand/collapse with keyboard and mouse double-click
- Detail panel showing column info (type, PK, nullable, default, FK)
- Double-click table name to auto-generate and run `SELECT * FROM "table" LIMIT 1000 OFFSET 0`

### Query Editor
- Split layout: editor (25%) / results (75%), toggleable with `Alt+I`
- **Table double-click** in Explorer automatically generates and runs `SELECT * FROM "table" LIMIT 1000 OFFSET 0`
- **Vi-style modal editing** with NORMAL and INSERT modes
- NORMAL mode: `h`/`l` cursor movement, `w`/`b` word navigation, `dw`/`db` word deletion, `H`/`L` line start/end, `u`/`R` undo/redo, `Y` copy query to clipboard, `x`/`Backspace` char deletion
- INSERT mode: standard text entry, `Esc`/`jj` to return to NORMAL, `Ctrl+R` to run query
- Cursor shape changes: block `■` in NORMAL, vertical bar `|` in INSERT
- Result pane with column headers, row highlighting, and scrollable viewport
- **Fuzzy search** in results (`Alt+F`): type-ahead filtering across all cell values with character-level match highlighting (yellow bold on matches)
- Cell selection with arrow keys / j/k / Tab
- Cell editing via popup dialog (Enter edit, Tab/Enter confirm, Esc cancel)
- Copy cell value to clipboard (Y key)
- Horizontal (Ctrl+B/F) and vertical (Ctrl+U/D) scrolling
- Row highlighting (yellow/bold-black) on selected row
- Dynamic key hints in status bar that update based on focus and edit mode

### Inspector Pane
- Column-by-column detail of selected row with type badges
- Inline editing (click value, Enter save, Esc cancel, Alt+Z undo)
- Collapsible JSON tree viewer for nested data
- Toggleable visibility (Alt+H)
- Keyboard navigation (j/k scroll, g/G top/bottom, Home/End in edit)

### Connections
- Connection list with database-type icons and status indicators
- Add/Edit/Delete connections with confirmation dialog
- Connection form with 7 fields and DB type cycling (port auto-fills)
- Password saved to OS keychain on connect
- Double-click to connect, Alt+Enter to disconnect

### MongoDB Document Viewer
- Flat document list with expandable/collapsible nested fields and arrays
- Expand all (o) / collapse all (O) per document
- Arrow-key navigation (j/k in 3-line steps)
- Inline value editing via Enter on any scalar field
- Type badges on every field
- Copy value to clipboard (Y)
- Syntax-highlighted value display (strings green, numbers blue, dates blue, ObjectId red)

### Pages
| Key | Page | Status |
|-----|------|--------|
| 1 | Dashboard | ✓ |
| 2 | Connections | ✓ |
| 3 | Database Explorer | ✓ |
| 4 | Query Editor | ✓ |
| 5 | Query History | ✓ |
| 6 | Favorites | ✓ |
| 7 | Schema Browser | ✓ |
| 8 | Logs | ✓ |
| 9 | Settings | Placeholder |
| `?` | Help | ✓ |
| - | About | Placeholder |

## Quick Start

```bash
# Build and run
cargo run --release
```

### Prerequisites

- Rust 1.81+
- PostgreSQL, SQLite, or MongoDB database (or use the built-in connection form to configure)

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1`–`8` | Switch pages |
| `Ctrl+Q` | Quit |
| `?` | Help |
| `Tab` | Cycle focus forward |
| `Shift+Tab` | Cycle focus reverse |
| `Alt+F` | Result filter (fuzzy search in results pane) |
| `Alt+H` | Toggle Inspector pane |
| `Alt+I` | Toggle Query Editor pane |

### Sidebar (Database Explorer)

| Key | Action |
|-----|--------|
| `j` / `Down` | Select next |
| `k` / `Up` | Select previous |
| `l` / `Right` | Expand / Load children |
| `h` / `Left` | Collapse / Go to parent |
| `g` | Go to first |
| `G` | Go to last |
| `Enter` / Double-click | Expand node / Load table columns / Run SELECT query on table |

### Query Editor — Vi Mode

The editor uses modal keybindings. Start editing with `Ctrl+E` or Tab to the editor pane.

#### NORMAL mode (default)

| Key | Action |
|-----|--------|
| `i` | Enter INSERT mode |
| `h` / `Left` | Move cursor left |
| `l` / `Right` | Move cursor right |
| `H` | Move to start of line |
| `L` | Move to end of line |
| `w` | Move forward by word |
| `b` | Move backward by word |
| `dw` | Delete word forward (consumes trailing space) |
| `db` | Delete word backward (consumes trailing space) |
| `x` | Delete char at cursor |
| `Backspace` | Delete char before cursor |
| `u` | Undo |
| `R` | Redo |
| `Y` | Copy query to clipboard |
| `Ctrl+R` | Run query |
| `Esc` | No-op |
| Tab / Shift+Tab | Cycle focus to other panes |
| Click | Move cursor to clicked position |
| Double-click | Enter INSERT mode at cursor position |

#### INSERT mode

| Key | Action |
|-----|--------|
| `Esc` | Return to NORMAL mode |
| `jj` | Return to NORMAL mode |
| `Ctrl+R` | Run query and return to NORMAL |
| Click | Move cursor to clicked position |

Cursor indicates current mode: block `■` in NORMAL, vertical bar `|` in INSERT.

### Query Editor — Results

| Key | Action |
|-----|--------|
| `j` / `Down` | Navigate rows down |
| `k` / `Up` | Navigate rows up |
| `Right` / `Left` | Next / previous column |
| `PageUp` / `PageDown` | Page up / down |
| `Home` / `End` | First / last row |
| `Alt+F` | Toggle fuzzy filter (type to filter, Esc clear, Enter accept) |
| `Enter` | Edit selected cell |
| `Y` / `y` | Copy cell value |
| `Esc` | Clear selection / Focus editor |
| `Ctrl+R` | Run query |
| `Ctrl+E` | Edit query text |
| `Ctrl+B` / `Ctrl+F` | Scroll results left/right |
| `Ctrl+U` / `Ctrl+D` | Scroll results up/down |

Fuzzy filter matches across all cell values with character-level highlighting (yellow bold). Active filter stays applied across queries; `Esc` clears, `Enter` accepts and exits edit mode, `Alt+F` toggles.

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

### MongoDB Document Viewer

| Key | Action |
|-----|--------|
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `l` | Expand nested field |
| `h` | Collapse / Go to parent |
| `Enter` | Edit scalar value |
| `g` | Go to first line |
| `G` | Go to last line |
| `Y` / `y` | Copy value |
| `o` | Expand all fields in current document |
| `O` | Collapse all fields in current document |
| `Esc` | Clear selection / Focus editor |

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
| `Alt+Enter` | Disconnect (when selected connection is connected) |

### Connection Form

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle fields (tab bar → form/URI fields) |
| `Left` / `Right` | Switch tab (on tab bar) / Change DB type (on Type field) |
| `Enter` | Submit |
| `Esc` | Cancel |

### Connection URI Formats

The URI tab in the Add/Edit Connection dialog accepts raw connection strings.
Paste a URI to auto-fill all connection fields.

| Database | URI Format |
|----------|------------|
| PostgreSQL | `postgres://user:password@host:5432/database` |
| MySQL | `mysql://user:password@host:3306/database` |
| MariaDB | `mariadb://user:password@host:3308/database` |
| SQLite | `sqlite:///absolute/path/to/database.db` |
| SQL Server | `sqlserver://user:password@host:1433/database` |
| MongoDB | `mongodb://host:27017/database` |
| Redis | `redis://host:6379/0` |

**Tips:**
- SQLite paths can be absolute (`sqlite:///home/user/data.db`) or relative (`sqlite://data/seed.db`)
- Redis db number is the path segment (`redis://host:6379/2` for db 2)
- Omit credentials for unauthenticated connections (`postgres://host:5432/db`)
- The connection name is auto-generated from the URI when submitting from the URI tab

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
  database/     — DatabaseProvider trait, metadata cache, pool, data types, result
  events/       — AppEvent enum (reserved for future event system)
  logging/      — Tracing initialization
  models/       — Connection config, data models
  providers/    — Database driver implementations (postgres/, sqlite/, mysql/, mariadb/, mongodb/)
  storage/      — Connection persistence, OS keychain
  ui/           — Layout, pages (dashboard, connections, explorer, query_editor, ...), sidebar, search
  widgets/      — Reusable UI components (input field, tree widget, mongo doc list)
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
│  PostgresProvider │ SqliteProvider │ MysqlProvider │ MariadbProvider │ MongoProvider │ ...  │
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

# MongoDB integration tests (requires MongoDB on localhost:27017)
cargo test --test mongodb -- --ignored
```

## Lint

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Roadmap

| Milestone | Feature | Key Deliverables | Status |
|-----------|---------|------------------|--------|
| M1 | Project scaffold | Cargo workspace, Tokio runtime, Ratatui/Crossterm init, page navigation, GitHub Actions, CI | ✓ |
| M2 | Core architecture | `DatabaseProvider` trait, `ConnectionPool`, event loop, focus system, key/mouse dispatch | ✓ |
| M3 | Configuration & storage | TOML config, XDG directories, connection persistence, OS keychain integration, structured logging | ✓ |
| M4 | SQLite provider | Connect/ping, metadata queries (tables, columns, indexes, views), transactions, server info | ✓ |
| M5 | Connection UI | Connection list with DB-type icons, add/edit/delete modals, status indicators, double-click connect | ✓ |
| M6 | Database Explorer | Tree view: Server → Databases → Schemas → Tables/Views/Indexes/Functions/Procedures, lazy-loading columns on expand, sidebar detail panel with column info (type, PK, nullable, default), keyboard and mouse navigation | ✓ |
| M7 | Query execution & results | Split editor/results layout, SQL execution (Ctrl+R), SELECT/non-SELECT detection, column headers, row/cell selection, horizontal/vertical scrolling (Ctrl+B/F/U/D), copy cell (Y), mouse scroll | ✓ |
| M8 | Data editing | Grid cell editing via popup (Enter/Tab/Esc), Inspector pane (Alt+H) with inline editing, JSON tree viewer for nested data, row highlighting, cell edit with undo (Alt+Z), MongoDB document list inline editing | ✓ |
| M9 | PostgreSQL provider | Connect/ping, metadata (tables, columns, indexes, views, sequences, triggers, functions, procedures, extensions, custom types), server info, transactions, chrono type support | ✓ |
| M10 | MongoDB provider | Connect/ping, CRUD operations (insertOne/Many, updateOne/Many, deleteOne/Many, find, aggregate, count, distinct), collection/column/index metadata, explorer tree integration, JSON document viewer | ✓ |
| M11 | Global search | Fuzzy-find across Explorer tree, table/column metadata, query text, result cells, with highlighted keywords, source-priority sorting, keyboard navigation (Ctrl+N/P) | ✓ |
| M12 | Vi-mode query editing | NORMAL/INSERT modal editing, word/line navigation, word deletion, undo/redo stacks, clipboard copy, Tab focus cycling, cursor style per mode, click-to-position, docker-compose dev databases | ✓ |
| M13 | MySQL provider | Connect/ping, metadata queries (tables, columns, indexes, views, triggers, functions, procedures), CRUD, transactions, cancel query, server info | ✓ |
| M14 | MariaDB provider | Connect/ping, metadata queries (tables, columns, indexes, views, triggers, functions, procedures, sequences, custom types), CRUD, transactions, cancel query, server info | ✓ |
| M15 | SQL Server provider | Connect/ping, metadata queries, CRUD, transactions | |
| M16 | Redis provider | Key browser, TTL, streams, hashes, sets, lists, sorted sets, pub/sub | ✓ |
| M17 | Schema management | CREATE/ALTER/DROP/RENAME tables, generate DDL, reverse engineer schema, schema diff | |
| M18 | Import / Export | CSV, JSON, SQL, Excel, Parquet, XML, YAML, compressed export | |
| M19 | Query history | History log, pinned queries, folders, tags, search, favorite, import/export | |
| M20 | Bookmarks & favorites | Bookmark rows, tables, queries, connections, schemas | |
| M21 | Themes | Dark theme (current), light theme, custom TOML themes, configurable colors | |
| M22 | Configurable keybindings | User-defined key mappings in TOML, keybinding presets | |
| M23 | Security | SSH tunnel, SSL/TLS, certificate management, read-only mode | |
| M24 | Performance | Virtual scrolling, infinite scroll, dataset virtualization, lazy metadata loading, caching, incremental rendering | |
| M25 | Plugin system | Database drivers, exporters, themes, editor extensions, macros, custom panels | |
| M26 | Testing & CI/CD | Unit tests on all modules, integration tests per provider (PostgreSQL, MongoDB), snapshot UI tests, performance benchmarks, cross-platform CI | |
| M27 | Packaging | AppImage, deb, rpm (Linux), DMG, Homebrew (macOS), MSI, portable EXE (Windows) | |
| M28 | Documentation | Architecture docs, developer guide, contribution guide, user guide, keybinding reference, plugin guide, API docs | |
| M29+ | Stable release | Polish, edge-case hardening, accessibility, public beta, v1.0 | |

See [CHANGELOG.md](CHANGELOG.md) for detailed per-release notes.

## License

MIT
