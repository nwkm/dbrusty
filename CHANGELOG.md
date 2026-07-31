# Changelog

All notable changes to Dbrusty are documented here.

---

## [Unreleased]

### Added
- **Enhanced Logs page** (Page 8)
  - Log level detection and color coding (ERROR=red, WARN=yellow, INFO=cyan, DEBUG=dark gray)
  - `[LEVEL]` tags on each log line for quick visual scanning
  - Scroll with `j`/`k`, jump to top with `g`/`G`, Page Up/Down support
  - Filter/search with `/` (type-ahead filter across all log entries)
  - Clear all logs with `c`
  - Scroll position indicator (percentage) in footer
  - Entry count display in header
  - Filter status shows matched/total count
- **Schema Browser enhancements** (Page 7)
  - Sequences category now shows sequence names with type/start/increment details
  - Triggers category now shows trigger names with table/event/timing/definition details
  - Functions category now shows function names with return type/language/args/definition details
  - Procedures category now shows procedure names with parameter details
  - Keyboard hints bar at bottom showing context-sensitive keybindings
  - All categories now populate from provider metadata during connection
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
- **Results pane fuzzy search** (`Alt+F` in Results focus)
  - Type-ahead filter using nucleo fuzzy matching across all cell values
  - Character-level match highlighting (yellow bold on normal rows, cyan on selected)
  - Filter bar shows `Result > ...` with match count, cursor navigation (Left/Right/Home/End)
  - `Esc` clears filter, `Enter` accepts and keeps filter active, `Alt+F` toggles
  - Filter re-applies automatically on new query results
  - Navigation (arrow keys, PageUp/Down, Home/End) stays within filtered row set
  - Filter input is scoped to Results focus — no key capture when other panes are focused
- **Query cancellation** — in-flight queries can be cancelled with `Esc` or `Ctrl+C` while the app is busy
  - Hung queries no longer deadlock the UI: every query runs under a 10-minute timeout backstop
  - The connection is returned to the pool on cancel/timeout/failure, not silently lost

### Changed
- `Ctrl+E` to enter editing mode starts in NORMAL mode (not INSERT)
- `Ctrl+R` to run query stays in editing mode (NORMAL) instead of exiting
- Query editor block title shows `[NORMAL]` or `[INSERT]` based on current mode
- Focus dispatch now routes to editor handler only when the editor pane is focused
- Query Editor layout uses `Min(0)` constraint so editor collapses when hidden (`Alt+I`)
- Key hints updated for `Alt+I` (toggle editor) and `Alt+F` (toggle filter)
- Results border title shows `[FILTER]` when filter input is active

### Fixed
- Keybindings on non-editor panes (Sidebar, Results, Inspector) work correctly when editor is active
- **Result filter no longer captures keys when focus is elsewhere** — filter input is scoped to Results pane only
- **Empty filter results show zero rows instead of falling back to full dataset** — filter remains active with "0 / N results" display
- **Filter matches against cell values only (not column names)** — prevents phantom matches from column name substrings
- **Filtered row navigation respects filtered set** — arrow keys, PageUp/Down, Home/End stay within filtered indices
- **Explorer table nodes only show expand arrow on explicit expand (Right key)** — navigating tables with j/k no longer calls `set_children` on the tree node, preventing toggle arrows from appearing on leaf table nodes; columns are still cached for the detail pane display
- **Switching to QueryEditor (4 key) no longer freezes keyboard when editor is hidden** — `switch_to(Page::QueryEditor)` only enters editing mode when `show_query_editor` is true; when the editor is hidden (`show_query_editor = false`), sidebar navigation keys continue to work
- **Connect metadata loading no longer hangs forever** — each metadata call (`get_databases`, `get_tables`, `get_views`, …) runs under a per-call timeout; failures and timeouts are surfaced as warnings instead of silently showing an empty tree
- **Disconnect during an in-flight query is now honored** — a queued disconnect drops the connection when the query completes instead of silently re-activating it; disconnecting during a connect is also handled
- **Column loading, schema DDL generation, and disconnect no longer hold a `&pool` borrow across `.await`** — the provider is checked out of the pool for the call and returned afterwards, avoiding UI freezes and `!Send` futures
- **Connection list persistence and keychain reads/writes moved off the event loop** (`tokio::task::spawn_blocking`) — keychain prompts and disk writes no longer block the async UI thread
- **Log entries now use `VecDeque` with O(1) trimming** instead of `Vec::remove(0)` O(n) shifts, and `set_status` no longer double-clones the message
- **Connection form `field_value()` returns `&str`** instead of cloning up to 7 strings per render
- **Timeout message matches the real 10-second connect timeout** (was "20 seconds")

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
