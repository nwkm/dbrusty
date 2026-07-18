# Changelog

All notable changes to Dbrusty are documented here.

---

## [Unreleased]

### Added
- Inspector pane: toggleable (Alt+H) detail view for the selected row's columns with type badges, inline editing (click value, Enter save, Esc cancel, Alt+Z undo), horizontal scroll for long values, vertical scroll (j/k/g/G)
- Double-click table name in Explorer tree to execute `SELECT * FROM "table" LIMIT 1000 OFFSET 0` and display results in Query Editor
- Chrono date/time type support in PostgreSQL provider (NaiveDate, NaiveDateTime, DateTime<Utc>, NaiveTime decode to formatted text instead of null)
- Row highlighting (yellow background, bold black text) on selected row in results view
- Dynamic key hints in status bar that update based on focus and edit mode
- Inspector focus cycling (Tab through Sidebar/Main/Inspector); Inspector hidden from cycle when toggled off

### Fixed
- Missing `chrono` feature flag on sqlx dependency (PostgreSQL date/time columns returned null)
- Backspace key losing focus in Inspector edit mode
- Off-by-one cursor rendering in Inspector edit field
- Capital letters not inputtable in Inspector edit mode (SHIFT modifier now accepted)

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
