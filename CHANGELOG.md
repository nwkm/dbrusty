# Changelog

All notable changes to Dbrusty are documented here.

---

## [Unreleased]

### Added
- **Vi-style modal editing** in Query Editor with NORMAL/INSERT modes
  - NORMAL mode: `h`/`l` cursor movement, `w`/`b` word navigation, `H`/`L` line start/end, `dw`/`db` word deletion, `x`/`Backspace` char deletion, `u`/`R` undo/redo, `Y` copy query to clipboard
  - INSERT mode: text entry, `Esc`/`jj` to return to NORMAL, `Ctrl+R` to run query
  - Undo/redo stacks: per-INSERT-session snapshots, per-NORMAL-edit snapshots
  - Cursor style changes: block `■` in NORMAL, vertical bar `|` in INSERT
  - Click-to-position cursor in both NORMAL and INSERT modes
  - Double-click enters INSERT mode at cursor position
  - Tab/Shift+Tab cycle focus across panes from any mode (resets to NORMAL on editor re-entry)
  - Focus on Query Editor auto-enters NORMAL mode with cursor at content end (trailing spaces ignored)
- **Docker Compose** dev database setup (`docker-compose.yml`)
  - PostgreSQL 16, MySQL 8, MariaDB 11, MongoDB 7, Redis 7
  - Named volumes for persistent storage, health checks on all services
- **Seed script** (`scripts/seed.sh`) to build and run all seed binaries against Docker databases
  - Per-database skip flags (`--skip-postgres`, `--skip-mysql`, `--skip-mariadb`, `--skip-mongodb`)
- Connection success now switches to Explorer page with sidebar focus

### Changed
- `Ctrl+E` to enter editing mode starts in NORMAL mode (not INSERT)
- `Ctrl+R` to run query stays in editing mode (NORMAL) instead of exiting
- Query editor block title shows `[NORMAL]` or `[INSERT]` based on current mode
- Focus dispatch now routes to editor handler only when the editor pane is focused

### Fixed
- Keybindings on non-editor panes (Sidebar, Results, Inspector) work correctly when editor is active

---

## [0.2.0] - 2026-07-10

### Added
- Query Editor with split layout (25% editor / 75% results)
- SQL query execution (Ctrl+R) with SELECT vs non-SELECT auto-detection
- Results display with column headers, separators, and data rows
- Horizontal/vertical scrolling in results (Ctrl+B/F/U/D)
- Cell selection with arrow keys / Tab in results grid
- Cell editing via popup dialog (Enter to edit, Tab/Enter/Esc to confirm/cancel)
- Copy cell value to clipboard (Y key)
- Query editor text input (Ctrl+E to enter edit mode, Esc to exit)
- Connection form with 7 fields, DB type cycling, port auto-fill
- OS keychain integration for credential storage
- Delete connection with confirmation dialog
- Double-click to connect / edit cell / expand tree nodes
- Mouse support: click to select, scroll to navigate
- Focus system: Sidebar / Main / Editor / Results cycling (Tab, Ctrl+Ctrl+H/Backspace)
- Connection list with database-type icons and status indicators
- PostgreSQL provider: full metadata queries (tables, columns, indexes, views, sequences, triggers, functions, procedures, extensions, custom types)
- SQLite provider: full metadata queries (tables, columns, indexes, views, server info)
- Metadata cache to avoid redundant schema fetches
- Connection pool supporting multiple simultaneous connections
- In-memory log viewer (Logs page, capped at 500 entries)
- Help page with keybinding reference
- Connection auto-save on create/update
- Server info display (version, vendor, uptime)

### Changed
- Layout system: configurable sidebar/main/inspector columns
- Results rendering: optimized column-width computation

### Fixed
- Connection scroll position after deletion
- Pending action processing after connection form submit
- Password field handling in connection form

---

## [0.1.0] - 2026-06-28

### Added
- Project scaffold with Tokio async runtime
- Ratatui TUI framework integration
- Cross-platform terminal backend (Crossterm)
- Page navigation system (1-8 keys)
- Dashboard welcome page
- Connections browser page
- Database Explorer tree with lazy-loading
- Tree widget with expand/collapse, keyboard navigation (j/k/h/l/g/G), mouse scroll
- Sidebar/detail panel for selected tree nodes
- DatabaseProvider trait abstraction
- ConnectionConfig model with DatabaseType enum (Postgres, MySQL, MariaDB, SQLite, SQLServer, MongoDB, Redis)
- SQLite provider implementation
- PostgreSQL provider implementation
- Connection pool (HashMap-based)
- Config system (TOML, XDG directories)
- Connection persistence (TOML file)
- Credential storage module (keyring)
- Structured logging (tracing + color-eyre)
- Placeholder pages: History, Favorites, Schema Browser, Settings, About
