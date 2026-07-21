use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellEditMode {
    Inactive,
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellEditFocus {
    Editor,
    Cancel,
    Confirm,
}

use crate::config::AppConfig;
use crate::database::metadata::MetadataCache;
use crate::database::pool::ConnectionPool;
use crate::database::provider::DatabaseProvider;
use crate::database::result::Value;
use crate::database::types::ColumnInfo;
use crate::models::connection::{ConnectionConfig, DatabaseType};
use crate::providers::mariadb::MariadbProvider;
use crate::providers::mongodb::MongoProvider;
use crate::providers::mysql::MysqlProvider;
use crate::providers::postgres::PostgresProvider;
use crate::providers::sqlite::SqliteProvider;
use crate::storage;
use crate::ui;
use crate::widgets::input::InputField;
use crate::widgets::mongo_doc_list::MongoDocListState;
use crate::widgets::tree::TreeState;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Connections,
    Explorer,
    QueryEditor,
    History,
    Favorites,
    SchemaBrowser,
    Logs,
    Settings,
    Help,
    About,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Dashboard => "Dashboard",
            Page::Connections => "Connections",
            Page::Explorer => "Database Explorer",
            Page::QueryEditor => "Query Editor",
            Page::History => "Query History",
            Page::Favorites => "Favorites",
            Page::SchemaBrowser => "Schema Browser",
            Page::Logs => "Logs",
            Page::Settings => "Settings",
            Page::Help => "Help",
            Page::About => "About",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
    Search,
    Visual,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryEditorFocus {
    Editor,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnPageMode {
    Browse,
    Adding,
    Editing(Uuid),
    Deleting(usize),
}

#[derive(Debug, Clone)]
pub struct ConnectionFormState {
    pub is_new: bool,
    pub editing_id: Option<Uuid>,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub active_field: usize,
    pub error: Option<String>,
}

impl ConnectionFormState {
    pub fn new() -> Self {
        let env_host = std::env::var("DATABASE_HOST").ok();
        let has_env = env_host.is_some();
        Self {
            is_new: true,
            editing_id: None,
            name: if has_env {
                std::env::var("DATABASE_NAME").unwrap_or_default()
            } else {
                String::new()
            },
            db_type: if has_env {
                DatabaseType::Postgres
            } else {
                DatabaseType::SQLite
            },
            host: env_host.unwrap_or_else(|| "localhost".to_string()),
            port: std::env::var("DATABASE_PORT").unwrap_or_default(),
            database: if has_env {
                std::env::var("DATABASE_NAME").unwrap_or_default()
            } else {
                String::new()
            },
            username: std::env::var("DATABASE_USERNAME").unwrap_or_default(),
            password: std::env::var("DATABASE_PASSWORD").unwrap_or_default(),
            active_field: 0,
            error: None,
        }
    }

    pub fn from_config(conn: &ConnectionConfig, password: Option<&str>) -> Self {
        Self {
            is_new: false,
            editing_id: Some(conn.id),
            name: conn.name.clone(),
            db_type: conn.db_type.clone(),
            host: conn.host.clone(),
            port: conn.port.to_string(),
            database: conn.database.clone(),
            username: conn.username.clone(),
            password: password.unwrap_or("").to_string(),
            active_field: 0,
            error: None,
        }
    }

    pub fn field_value(&self, idx: usize) -> String {
        match idx {
            0 => self.name.clone(),
            1 => self.db_type.display_name().to_string(),
            2 => self.host.clone(),
            3 => self.port.clone(),
            4 => self.database.clone(),
            5 => self.username.clone(),
            6 => self.password.clone(),
            _ => String::new(),
        }
    }

    pub fn set_field(&mut self, idx: usize, value: String) {
        match idx {
            0 => self.name = value,
            1 => {}
            2 => self.host = value,
            3 => self.port = value,
            4 => self.database = value,
            5 => self.username = value,
            6 => self.password = value,
            _ => {}
        }
    }

    pub fn to_config(&self) -> ConnectionConfig {
        let port: u16 = self
            .port
            .parse()
            .unwrap_or_else(|_| self.db_type.default_port());
        let password = if self.password.is_empty() {
            None
        } else {
            Some(self.password.clone())
        };
        ConnectionConfig {
            id: self.editing_id.unwrap_or_else(Uuid::new_v4),
            name: self.name.clone(),
            db_type: self.db_type.clone(),
            host: self.host.clone(),
            port,
            database: self.database.clone(),
            username: self.username.clone(),
            group: None,
            is_favorite: false,
            ssl: false,
            created_at: chrono::Utc::now(),
            last_connected: None,
            password,
        }
    }
}

pub enum PendingAction {
    None,
    Connect(ConnectionConfig),
    Disconnect(Uuid),
    LoadColumns { table: String },
    OpenQuery(String),
    RunQuery(String),
}

const DB_TYPE_CYCLE: &[DatabaseType] = &[
    DatabaseType::Postgres,
    DatabaseType::MySQL,
    DatabaseType::MariaDB,
    DatabaseType::SQLite,
    DatabaseType::SQLServer,
    DatabaseType::MongoDB,
    DatabaseType::Redis,
];

pub struct App {
    #[allow(dead_code)]
    pub config: AppConfig,
    pub should_quit: bool,
    pub current_page: Page,
    #[allow(dead_code)]
    pub input_mode: InputMode,
    pub focus: Focus,
    pub status_message: String,
    pub key_hints: String,
    pub pool: ConnectionPool,
    pub connections: Vec<ConnectionConfig>,
    pub conn_list_selected: usize,
    pub conn_list_scroll: usize,
    pub conn_page_mode: ConnPageMode,
    pub conn_form: Option<ConnectionFormState>,
    pub pending_action: PendingAction,
    pub explorer_tree: TreeState,
    pub metadata_cache: MetadataCache,
    pub sidebar_rect: Rect,
    pub main_rect: Rect,
    pub inspector_rect: Rect,
    pub last_click: Option<(u16, u16, Instant)>,
    pub log_entries: Vec<String>,
    pub log_scroll: usize,
    pub query_text: String,
    pub query_results: String,
    pub query_columns: Vec<String>,
    pub query_data: Vec<Vec<String>>,
    pub query_data_typed: Vec<Vec<Value>>,
    pub query_column_info: Vec<ColumnInfo>,
    pub query_col_widths: Vec<usize>,
    pub selected_row: usize,
    pub selected_col: usize,
    pub show_inspector: bool,
    pub inspector_scroll: usize,
    pub inspector_edit_col: Option<usize>,
    pub inspector_edit_value: String,
    pub inspector_edit_cursor: usize,
    pub inspector_edit_scroll_h: u16,
    pub inspector_saved_value: Option<(usize, usize, String)>,
    pub cell_edit_mode: CellEditMode,
    pub cell_edit_input: InputField,
    pub cell_edit_focus: CellEditFocus,
    pub query_editor: InputField,
    pub editing_query: bool,
    pub query_editor_focus: QueryEditorFocus,
    pub result_scroll_v: usize,
    pub result_scroll_h: u16,
    pub global_search: Option<crate::ui::search::GlobalSearchState>,
    pub json_tree_state: TreeState,
    pub json_tree_cell: (usize, usize),
    pub mongo_raw_documents: Option<Vec<bson::Document>>,
    pub mongo_doc_state: MongoDocListState,
    pub mongo_edit_path: Option<String>,
}

fn mongo_value_strip_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

fn mongo_update_by_path(doc: &mut bson::Document, parts: &[&str], value: bson::Bson) {
    if parts.is_empty() {
        return;
    }
    let key = parts[0];
    if parts.len() == 1 {
        doc.insert(key, value);
    } else {
        let rest = &parts[1..];
        if rest[0].starts_with('[') && rest[0].ends_with(']') {
            let idx: usize = rest[0][1..rest[0].len() - 1].parse().unwrap_or(0);
            if let Some(bson::Bson::Array(arr)) = doc.get_mut(key) {
                if idx < arr.len() {
                    if rest.len() == 1 {
                        arr[idx] = value;
                    } else if let Some(nested) = arr[idx].as_document_mut() {
                        mongo_update_by_path(nested, &rest[1..], value);
                    }
                }
            }
        } else {
            if let Some(bson::Bson::Document(nested)) = doc.get_mut(key) {
                mongo_update_by_path(nested, rest, value);
            }
        }
    }
}

impl App {
    fn active_db_type(&self) -> Option<DatabaseType> {
        self.pool.active_id.and_then(|id| {
            self.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.db_type.clone())
        })
    }

    pub fn is_mongo_query(&self) -> bool {
        self.mongo_raw_documents.is_some()
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.log_entries.push(msg.clone());
        if self.log_entries.len() > 500 {
            self.log_entries.remove(0);
        }
        self.status_message = msg;
    }

    fn results_visible_rows(&self) -> usize {
        self.main_rect.height.saturating_sub(4) as usize
    }

    fn max_result_scroll_h(&self) -> u16 {
        let total: usize = self.query_col_widths.iter().copied().sum();
        let separators = self.query_col_widths.len().saturating_sub(1) * 3;
        (total + separators).saturating_sub(1) as u16
    }

    fn clamp_result_scroll(&mut self) {
        if !self.query_data.is_empty() {
            self.result_scroll_v = self
                .result_scroll_v
                .min(self.query_data.len().saturating_sub(1));
            let max_h = self.max_result_scroll_h();
            self.result_scroll_h = self.result_scroll_h.min(max_h);
        } else {
            let lines: Vec<&str> = self.query_results.lines().collect();
            let max_v = lines.len().saturating_sub(1);
            self.result_scroll_v = self.result_scroll_v.min(max_v);
            let max_h = lines
                .iter()
                .map(|l| l.len())
                .max()
                .unwrap_or(0)
                .saturating_sub(1) as u16;
            self.result_scroll_h = self.result_scroll_h.min(max_h);
        }
    }

    pub fn with_config(config: AppConfig) -> Self {
        let mut connections = storage::connections::load_connections().unwrap_or_default();
        // Restore passwords from keychain for all saved connections
        for conn in &mut connections {
            if conn.password.is_none() {
                conn.password = storage::credentials::get_password(&conn.password_key())
                    .ok()
                    .flatten()
                    .filter(|p| !p.is_empty());
            }
        }
        let mut app = Self {
            config,
            should_quit: false,
            current_page: Page::Connections,
            input_mode: InputMode::Normal,
            focus: Focus::Main,
            status_message: String::new(),
            key_hints: String::from(" q:Quit | 1-8:Pages | ?:Help | Tab:Focus | Alt-H:Inspector"),
            pool: ConnectionPool::new(),
            connections,
            conn_list_selected: 0,
            conn_list_scroll: 0,
            conn_page_mode: ConnPageMode::Browse,
            conn_form: None,
            pending_action: PendingAction::None,
            explorer_tree: TreeState::new(Vec::new()),
            metadata_cache: MetadataCache::new(),
            sidebar_rect: Rect::default(),
            main_rect: Rect::default(),
            inspector_rect: Rect::default(),
            last_click: None,
            log_entries: Vec::new(),
            log_scroll: 0,
            query_text: String::new(),
            query_results: String::new(),
            query_columns: Vec::new(),
            query_data: Vec::new(),
            query_data_typed: Vec::new(),
            query_column_info: Vec::new(),
            query_col_widths: Vec::new(),
            selected_row: 0,
            selected_col: 0,
            show_inspector: false,
            inspector_scroll: 0,
            inspector_edit_col: None,
            inspector_edit_value: String::new(),
            inspector_edit_cursor: 0,
            inspector_edit_scroll_h: 0,
            inspector_saved_value: None,
            cell_edit_mode: CellEditMode::Inactive,
            cell_edit_input: InputField::new(""),
            cell_edit_focus: CellEditFocus::Editor,
            query_editor: InputField::new("Enter SQL query..."),
            editing_query: false,
            query_editor_focus: QueryEditorFocus::Editor,
            result_scroll_v: 0,
            result_scroll_h: 0,
            global_search: None,
            json_tree_state: TreeState::new(Vec::new()),
            json_tree_cell: (usize::MAX, usize::MAX),
            mongo_raw_documents: None,
            mongo_doc_state: MongoDocListState::new(),
            mongo_edit_path: None,
        };
        app.set_status("Ready");
        app
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        let mut terminal = ratatui::init();
        let result = self.run_loop(&mut terminal).await;
        ratatui::restore();
        crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
        result
    }

    async fn run_loop(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
    ) -> color_eyre::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, self))?;

            self.process_pending_actions().await;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key),
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    Event::Resize(w, h) => self.handle_resize(w, h),
                    _ => {}
                }
                self.clamp_result_scroll();
            }
        }
        Ok(())
    }

    async fn process_pending_actions(&mut self) {
        let action = std::mem::replace(&mut self.pending_action, PendingAction::None);
        match action {
            PendingAction::Connect(conn) => {
                self.connect_to(conn).await;
            }
            PendingAction::LoadColumns { table } => {
                self.load_columns(&table).await;
            }
            PendingAction::OpenQuery(query) => {
                self.query_text.clone_from(&query);
                self.query_editor.set_value(&query);
                self.current_page = Page::QueryEditor;
                self.query_editor_focus = QueryEditorFocus::Results;
                self.run_query(&query).await;
            }
            PendingAction::RunQuery(sql) => {
                self.run_query(&sql).await;
            }
            PendingAction::Disconnect(id) => {
                if let Some(provider) = self.pool.get_mut(id) {
                    let name = self
                        .connections
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    if let Err(e) = provider.disconnect().await {
                        self.set_status(format!("Error disconnecting: {}", e));
                    } else {
                        self.pool.unregister(id);
                        self.explorer_tree = TreeState::new(Vec::new());
                        self.query_text.clear();
                        self.query_editor.set_value("");
                        self.set_status(format!("Disconnected from \"{}\"", name));
                    }
                }
            }
            PendingAction::None => {}
        }
    }

    async fn connect_to(&mut self, conn: ConnectionConfig) {
        let mut provider: Box<dyn DatabaseProvider> = match conn.db_type {
            DatabaseType::Postgres => Box::new(PostgresProvider::new()),
            DatabaseType::SQLite => Box::new(SqliteProvider::new()),
            DatabaseType::MySQL => Box::new(MysqlProvider::new()),
            DatabaseType::MariaDB => Box::new(MariadbProvider::new()),
            DatabaseType::MongoDB => Box::new(MongoProvider::new()),
            _ => {
                self.set_status(format!(
                    "\"{}\" is {} (not yet supported).",
                    conn.name,
                    conn.db_type.display_name()
                ));
                return;
            }
        };

        match provider.connect(&conn).await {
            Ok(()) => {
                self.pool.register(conn.id, provider);
                self.pool.set_active(conn.id);
                self.query_text.clear();
                self.query_editor.set_value("");

                if let Some(provider) = self.pool.get_active() {
                    let databases = provider.get_databases().await.unwrap_or_default();

                    let mut tables_data = Vec::new();
                    let mut views_data = Vec::new();
                    let mut schemas_data: Vec<(String, Vec<String>)> = Vec::new();

                    for db in &databases {
                        let mut schemas = provider.get_schemas().await.unwrap_or_default();
                        // No explicit schemas (e.g. MongoDB) → use db name as synthetic schema
                        if schemas.is_empty() {
                            schemas = vec![db.clone()];
                        }
                        schemas_data.push((db.clone(), schemas.clone()));

                        for schema in &schemas {
                            let schema_key = format!("{}.{}", db, schema);

                            if let Ok(tables) = provider.get_tables(schema).await {
                                self.metadata_cache
                                    .set_tables(schema_key.clone(), tables.clone());
                                tables_data.push((schema_key.clone(), tables));
                            }
                            if let Ok(views) = provider.get_views(schema).await {
                                self.metadata_cache
                                    .set_views(schema_key.clone(), views.clone());
                                views_data.push((schema_key.clone(), views));
                            }
                        }
                    }

                    self.explorer_tree = crate::ui::sidebar::build_initial_tree(
                        &conn.name,
                        &databases,
                        &schemas_data,
                        &tables_data,
                        &views_data,
                    );
                }

                // Update last_connected and persist
                if let Some(saved) = self.connections.iter_mut().find(|c| c.id == conn.id) {
                    saved.last_connected = Some(chrono::Utc::now());
                    if saved.password.is_none() && conn.password.is_some() {
                        saved.password = conn.password.clone();
                    }
                    let _ = storage::connections::save_connections(&self.connections);
                }
                self.set_status(format!("Connected to \"{}\"", conn.name));
            }
            Err(e) => {
                self.set_status(format!("Failed to connect: {}", e));
            }
        }
    }

    async fn load_columns(&mut self, table: &str) {
        if let Some(provider) = self.pool.get_active() {
            match provider.get_columns(table).await {
                Ok(columns) => {
                    self.metadata_cache.set_columns(table.to_string(), columns);
                    self.set_status(format!("Loaded columns for \"{}\"", table));
                }
                Err(e) => {
                    self.set_status(format!("Failed to load columns: {}", e));
                }
            }
        }
    }

    async fn run_query(&mut self, sql: &str) {
        if self.pool.active_id.is_none() {
            self.set_status("No active connection");
            return;
        }
        match self.pool.get_active() {
            Some(provider) => match provider.execute_query(sql).await {
                Ok(result) => {
                    // Compute column widths (sample first 1000 rows for performance)
                    let sample_size = 1000.min(result.rows.len());
                    let col_widths: Vec<usize> = result
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(ci, col)| {
                            let header_len = col.name.len();
                            let max_data = result.rows[..sample_size]
                                .iter()
                                .map(|row| row.get(ci).map(|v| v.to_string().len()).unwrap_or(0))
                                .max()
                                .unwrap_or(0);
                            header_len.max(max_data)
                        })
                        .collect();

                    if result.raw_documents.is_some() {
                        self.mongo_raw_documents = result.raw_documents.clone();
                        self.mongo_doc_state = MongoDocListState::new();
                        self.query_columns.clear();
                        self.query_data.clear();
                        self.query_data_typed.clear();
                        self.query_column_info.clear();
                        self.query_col_widths.clear();
                        self.query_results.clear();
                    } else {
                        self.mongo_raw_documents = None;
                        if result.is_select {
                            self.query_results.clear();
                            self.query_columns =
                                result.columns.iter().map(|c| c.name.clone()).collect();
                            self.query_data = result
                                .rows
                                .iter()
                                .map(|row| row.iter().map(|v| v.to_string()).collect())
                                .collect();
                            self.query_data_typed = result.rows.clone();
                            self.query_column_info = result.columns.clone();
                            self.query_col_widths = col_widths;
                        } else {
                            let mut output = String::new();
                            output.push_str(&format!("({:?})", result.duration));
                            output.push('\n');
                            for (i, col) in result.columns.iter().enumerate() {
                                if i > 0 {
                                    output.push_str(" | ");
                                }
                                output.push_str(&format!(
                                    "{:width$}",
                                    col.name,
                                    width = col_widths[i]
                                ));
                            }
                            output.push('\n');
                            for (i, w) in col_widths.iter().enumerate() {
                                if i > 0 {
                                    output.push_str("-+-");
                                }
                                for _ in 0..*w {
                                    output.push('-');
                                }
                            }
                            output.push('\n');
                            for row in &result.rows {
                                for (i, val) in row.iter().enumerate() {
                                    if i > 0 {
                                        output.push_str(" | ");
                                    }
                                    output.push_str(&format!(
                                        "{:width$}",
                                        val.to_string(),
                                        width = col_widths[i]
                                    ));
                                }
                                output.push('\n');
                            }
                            output.push_str(&format!("{} rows affected", result.rows_affected));
                            output.push('\n');
                            self.query_results = output;
                            self.query_columns.clear();
                            self.query_data.clear();
                            self.query_data_typed.clear();
                            self.query_column_info.clear();
                            self.query_col_widths.clear();
                        }
                    }
                    self.result_scroll_v = 0;
                    self.result_scroll_h = 0;
                    self.inspector_scroll = 0;
                    self.selected_row = 0;
                    self.selected_col = 0;
                    self.cell_edit_mode = CellEditMode::Inactive;
                    self.set_status(format!(
                        "Query returned {} rows in {:?}",
                        result.rows.len(),
                        result.duration
                    ));
                }
                Err(e) => {
                    self.query_results = format!("Error: {}", e);
                    self.query_columns.clear();
                    self.query_data.clear();
                    self.query_data_typed.clear();
                    self.query_column_info.clear();
                    self.query_col_widths.clear();
                    self.mongo_raw_documents = None;
                    self.cell_edit_mode = CellEditMode::Inactive;
                    self.set_status(format!("Query failed: {}", e));
                }
            },
            None => {
                self.set_status("Provider not available");
            }
        }
    }

    fn handle_query_edit_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.editing_query = false;
                self.query_text = self.query_editor.value.clone();
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                self.editing_query = false;
                self.query_text = self.query_editor.value.clone();
                if !self.query_text.is_empty() {
                    let sql = self.query_text.clone();
                    self.pending_action = PendingAction::RunQuery(sql);
                }
            }
            _ => {
                self.query_editor.handle_key(key);
            }
        }
    }

    fn handle_cell_edit_popup_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.cell_edit_focus = match self.cell_edit_focus {
                    CellEditFocus::Editor => CellEditFocus::Cancel,
                    CellEditFocus::Cancel => CellEditFocus::Confirm,
                    CellEditFocus::Confirm => CellEditFocus::Editor,
                };
                true
            }
            KeyCode::Up => {
                self.cell_edit_focus = match self.cell_edit_focus {
                    CellEditFocus::Editor => CellEditFocus::Confirm,
                    CellEditFocus::Cancel => CellEditFocus::Editor,
                    CellEditFocus::Confirm => CellEditFocus::Cancel,
                };
                true
            }
            KeyCode::Esc => {
                self.cell_edit_mode = CellEditMode::Inactive;
                self.set_status("Edit cancelled");
                true
            }
            KeyCode::Enter => {
                match self.cell_edit_focus {
                    CellEditFocus::Cancel => {
                        self.cell_edit_mode = CellEditMode::Inactive;
                        self.set_status("Edit cancelled");
                    }
                    CellEditFocus::Confirm => {
                        self.confirm_cell_edit();
                    }
                    CellEditFocus::Editor => {
                        self.cell_edit_focus = CellEditFocus::Confirm;
                    }
                }
                true
            }
            _ => {
                if self.cell_edit_focus == CellEditFocus::Editor {
                    self.cell_edit_input.handle_key(key);
                    return true;
                }
                false
            }
        }
    }

    fn handle_results_key(&mut self, key: &KeyEvent) -> bool {
        if self.current_page != Page::QueryEditor
            || self.focus != Focus::Main
            || self.query_editor_focus != QueryEditorFocus::Results
        {
            return false;
        }

        if self.cell_edit_mode == CellEditMode::Editing {
            self.handle_cell_edit_popup_key(key);
            return true;
        }

        if self.is_mongo_query() {
            return self.handle_mongo_doc_key(key);
        }

        if self.query_data.is_empty() {
            return false;
        }

        let data_height = self.results_visible_rows();

        match key.code {
            KeyCode::Right => {
                self.inspector_edit_col = None;
                if self.selected_col + 1 < self.query_columns.len() {
                    self.selected_col += 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Left => {
                self.inspector_edit_col = None;
                if self.selected_col > 0 {
                    self.selected_col -= 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Down => {
                self.inspector_edit_col = None;
                if self.selected_row + 1 < self.query_data.len() {
                    self.selected_row += 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Up => {
                self.inspector_edit_col = None;
                if self.selected_row > 0 {
                    self.selected_row -= 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::PageUp => {
                self.inspector_edit_col = None;
                self.selected_row = self.selected_row.saturating_sub(data_height);
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::PageDown => {
                self.inspector_edit_col = None;
                self.selected_row = self
                    .selected_row
                    .saturating_add(data_height)
                    .min(self.query_data.len().saturating_sub(1));
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Home => {
                self.inspector_edit_col = None;
                self.selected_row = 0;
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::End => {
                self.inspector_edit_col = None;
                self.selected_row = self.query_data.len().saturating_sub(1);
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Enter => {
                self.start_cell_edit();
                true
            }
            KeyCode::Char('y') | KeyCode::Char('Y') if key.modifiers == KeyModifiers::NONE => {
                self.copy_selected_cell();
                true
            }
            KeyCode::Esc => {
                if self.selected_row != 0 || self.selected_col != 0 {
                    self.selected_row = 0;
                    self.selected_col = 0;
                    self.ensure_cell_visible(data_height);
                    self.set_status("Selection cleared");
                } else {
                    self.query_editor_focus = QueryEditorFocus::Editor;
                    self.set_status("Focus: Editor");
                }
                true
            }
            _ => false,
        }
    }

    fn handle_mongo_doc_key(&mut self, key: &KeyEvent) -> bool {
        // If cell edit popup is active, delegate to the shared handler
        if self.cell_edit_mode == CellEditMode::Editing {
            let result = self.handle_cell_edit_popup_key(key);
            if self.cell_edit_mode == CellEditMode::Inactive {
                self.mongo_edit_path = None;
            }
            return result;
        }

        let docs = match self.mongo_raw_documents {
            Some(ref docs) => docs,
            None => return false,
        };
        let total_lines = self.mongo_doc_state.cached_line_count(docs);

        let visible_height = self.main_rect.height.saturating_sub(4) as usize;

        let step = 1usize;

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                for _ in 0..step {
                    self.mongo_doc_state
                        .select_down(total_lines, visible_height);
                }
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                for _ in 0..step {
                    self.mongo_doc_state.select_up();
                }
                true
            }
            KeyCode::Enter => {
                let lines = crate::widgets::mongo_doc_list::build_flat_list_ref(
                    docs,
                    &self.mongo_doc_state.expanded_fields,
                );
                if let Some(line) = lines.get(self.mongo_doc_state.selected_line) {
                    if !line.is_header && !line.is_separator && !line.is_expandable {
                        self.mongo_edit_path = Some(line.path_key.clone());
                        let display = mongo_value_strip_quotes(&line.value_display).to_string();
                        self.cell_edit_input = InputField::with_value("", display.into());
                        self.cell_edit_mode = CellEditMode::Editing;
                        self.cell_edit_focus = CellEditFocus::Editor;
                        self.set_status("Editing value");
                    }
                }
                true
            }
            KeyCode::Char('l') => {
                self.mongo_doc_state.toggle_selected(docs);
                true
            }
            KeyCode::Char('h') => {
                let lines = crate::widgets::mongo_doc_list::build_flat_list_ref(
                    docs,
                    &self.mongo_doc_state.expanded_fields,
                );
                if let Some(line) = lines.get(self.mongo_doc_state.selected_line) {
                    if !line.is_header && line.is_expandable {
                        if self
                            .mongo_doc_state
                            .expanded_fields
                            .contains(&line.path_key)
                        {
                            self.mongo_doc_state.expanded_fields.remove(&line.path_key);
                            true
                        } else {
                            if let Some(last_dot) = line.path_key.rfind('.') {
                                let parent_key = line.path_key[..last_dot].to_string();
                                if self.mongo_doc_state.expanded_fields.contains(&parent_key) {
                                    self.mongo_doc_state.expanded_fields.remove(&parent_key);
                                }
                            }
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            KeyCode::Char('g') => {
                self.mongo_doc_state.go_to_first();
                true
            }
            KeyCode::Char('G') => {
                self.mongo_doc_state.go_to_last(total_lines);
                true
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                self.mongo_doc_state.page_up(visible_height);
                true
            }
            KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                self.mongo_doc_state.page_down(visible_height, total_lines);
                true
            }
            KeyCode::Char('o') => {
                self.mongo_doc_state.expand_all(docs);
                true
            }
            KeyCode::Char('O') => {
                self.mongo_doc_state.collapse_all(docs);
                true
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(val) = self.mongo_doc_state.selected_value(docs) {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(val.clone());
                    }
                    self.set_status(format!("Copied: {}", val));
                }
                true
            }
            KeyCode::Esc => {
                self.mongo_doc_state.selected_line = 0;
                self.mongo_doc_state.scroll_offset = 0;
                self.query_editor_focus = QueryEditorFocus::Editor;
                self.set_status("Focus: Editor");
                true
            }
            _ => false,
        }
    }

    fn mongo_confirm_edit(&mut self) {
        let path = match self.mongo_edit_path.take() {
            Some(p) => p,
            None => return,
        };
        let new_val = self.cell_edit_input.value.clone();

        // Parse path: first component is doc index, rest is field path
        let dot = match path.find('.') {
            Some(p) => p,
            None => return,
        };
        let doc_idx: usize = match path[..dot].parse() {
            Ok(i) => i,
            Err(_) => return,
        };
        let field_path = &path[dot + 1..];

        let docs = match self.mongo_raw_documents.as_mut() {
            Some(d) => d,
            None => return,
        };
        if doc_idx >= docs.len() {
            return;
        }

        let parts: Vec<&str> = field_path.split('.').collect();
        mongo_update_by_path(
            &mut docs[doc_idx],
            &parts,
            bson::Bson::String(new_val.clone()),
        );
        self.set_status(format!("Value updated"));
    }

    fn mongo_mouse_to_line(&self, mouse_row: u16) -> Option<usize> {
        let combined_h = self.main_rect.height;
        let results_area_y = self.main_rect.y + combined_h * 25 / 100;
        let inner_y = results_area_y + 1; // border
        if mouse_row < inner_y {
            return None;
        }
        let relative_row = (mouse_row - inner_y) as usize;
        let line = self.mongo_doc_state.scroll_offset + relative_row;
        let docs = self.mongo_raw_documents.as_ref()?;
        let total = self.mongo_doc_state.cached_line_count(docs);
        if line < total {
            Some(line)
        } else {
            None
        }
    }

    fn copy_selected_cell(&mut self) {
        if self.selected_row < self.query_data.len() && self.selected_col < self.query_columns.len()
        {
            let val = &self.query_data[self.selected_row][self.selected_col];
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(val.clone());
            }
            self.set_status(format!("Copied: {}", val));
        }
    }

    fn handle_inspector_key(&mut self, key: &KeyEvent) -> bool {
        if let Some(edit_col) = self.inspector_edit_col {
            let visible_w = self.inspector_rect.width.saturating_sub(4) as usize;
            match key.code {
                KeyCode::Enter => {
                    if self.selected_row < self.query_data.len()
                        && edit_col < self.query_columns.len()
                    {
                        if self.inspector_saved_value.is_none() {
                            let original = self.query_data[self.selected_row][edit_col].clone();
                            self.inspector_saved_value =
                                Some((self.selected_row, edit_col, original));
                        }
                        self.query_data[self.selected_row][edit_col] =
                            self.inspector_edit_value.clone();
                        if self.selected_row < self.query_data_typed.len()
                            && edit_col < self.query_data_typed[self.selected_row].len()
                        {
                            let original = &self.query_data_typed[self.selected_row][edit_col];
                            match original {
                                Value::Json(_) => {
                                    self.query_data_typed[self.selected_row][edit_col] =
                                        Value::Json(self.inspector_edit_value.clone());
                                }
                                _ => {
                                    self.query_data_typed[self.selected_row][edit_col] =
                                        Value::Text(self.inspector_edit_value.clone());
                                }
                            }
                        }
                    }
                    self.inspector_edit_col = None;
                    self.update_key_hints();
                    self.set_status("Value updated");
                    return true;
                }
                KeyCode::Esc => {
                    self.inspector_edit_col = None;
                    self.update_key_hints();
                    self.set_status("Edit cancelled");
                    return true;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.inspector_edit_value
                        .insert(self.inspector_edit_cursor, c);
                    self.inspector_edit_cursor += 1;
                    self.ensure_inspector_cursor_visible(visible_w);
                    return true;
                }
                KeyCode::Backspace => {
                    if self.inspector_edit_cursor > 0 {
                        self.inspector_edit_cursor -= 1;
                        self.inspector_edit_value.remove(self.inspector_edit_cursor);
                        self.ensure_inspector_cursor_visible(visible_w);
                    }
                    return true;
                }
                KeyCode::Delete => {
                    if self.inspector_edit_cursor < self.inspector_edit_value.len() {
                        self.inspector_edit_value.remove(self.inspector_edit_cursor);
                    }
                    return true;
                }
                KeyCode::Left => {
                    if self.inspector_edit_cursor > 0 {
                        self.inspector_edit_cursor -= 1;
                    } else if self.inspector_edit_scroll_h > 0 {
                        self.inspector_edit_scroll_h =
                            self.inspector_edit_scroll_h.saturating_sub(1);
                    }
                    self.ensure_inspector_cursor_visible(visible_w);
                    return true;
                }
                KeyCode::Right => {
                    if self.inspector_edit_cursor < self.inspector_edit_value.len() {
                        self.inspector_edit_cursor += 1;
                    } else {
                        let max_scroll = self.inspector_edit_value.len().saturating_sub(visible_w);
                        if self.inspector_edit_scroll_h as usize + 3 <= max_scroll {
                            self.inspector_edit_scroll_h =
                                (self.inspector_edit_scroll_h as usize + 3).min(max_scroll) as u16;
                        }
                    }
                    self.ensure_inspector_cursor_visible(visible_w);
                    return true;
                }
                KeyCode::Home => {
                    self.inspector_edit_cursor = 0;
                    self.inspector_edit_scroll_h = 0;
                    return true;
                }
                KeyCode::End => {
                    self.inspector_edit_cursor = self.inspector_edit_value.len();
                    self.inspector_edit_scroll_h = self
                        .inspector_edit_value
                        .len()
                        .saturating_sub(visible_w)
                        .max(0) as u16;
                    return true;
                }
                _ => {}
            }
            return true;
        }

        // JSON tree navigation when selected cell is JSON
        let is_json_cell = self
            .query_data_typed
            .get(self.selected_row)
            .and_then(|r| r.get(self.selected_col))
            .and_then(|v| v.json_str())
            .map(|s| s.starts_with('{') || s.starts_with('['))
            .unwrap_or(false);

        if is_json_cell {
            self.rebuild_json_tree();
            let visible_height = self.inspector_rect.height.saturating_sub(2) as usize;
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.json_tree_state.select_down();
                    self.json_tree_state.adjust_offset(visible_height);
                    return true;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.json_tree_state.select_up();
                    self.json_tree_state.adjust_offset(visible_height);
                    return true;
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    self.json_tree_state.toggle_selected();
                    self.json_tree_state.adjust_offset(visible_height);
                    return true;
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    let id = self.json_tree_state.selected_id();
                    if let Some(ref id) = id {
                        if self.json_tree_state.is_item_expanded(id) {
                            self.json_tree_state.toggle_by_id(id);
                        } else {
                            self.json_tree_state.select_up();
                        }
                    }
                    self.json_tree_state.adjust_offset(visible_height);
                    return true;
                }
                KeyCode::Char('g') => {
                    self.json_tree_state.go_to_first();
                    self.json_tree_state.offset = 0;
                    return true;
                }
                KeyCode::Char('G') => {
                    self.json_tree_state.go_to_last();
                    return true;
                }
                _ => {}
            }
        }

        // Alt+Z undo
        if key.code == KeyCode::Char('z') && key.modifiers == KeyModifiers::ALT {
            if let Some((row, col, original)) = self.inspector_saved_value.take() {
                if row < self.query_data.len() && col < self.query_columns.len() {
                    self.query_data[row][col] = original.clone();
                    if row < self.query_data_typed.len() && col < self.query_data_typed[row].len() {
                        let current = &self.query_data_typed[row][col];
                        match current {
                            Value::Json(_) => {
                                self.query_data_typed[row][col] = Value::Json(original);
                            }
                            _ => {
                                self.query_data_typed[row][col] = Value::Text(original);
                            }
                        }
                    }
                }
                self.set_status("Value restored");
            }
            return true;
        }

        // Scroll keys when not editing
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let max_scroll = self
                    .query_columns
                    .len()
                    .saturating_sub(self.inspector_rect.height as usize / 3)
                    .saturating_sub(1)
                    .max(0);
                if self.inspector_scroll < max_scroll {
                    self.inspector_scroll += 1;
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.inspector_scroll = self.inspector_scroll.saturating_sub(1);
                true
            }
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                self.inspector_scroll = 0;
                true
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::NONE => {
                self.inspector_scroll = self
                    .query_columns
                    .len()
                    .saturating_sub(self.inspector_rect.height as usize / 3)
                    .saturating_sub(1)
                    .max(0);
                true
            }
            _ => false,
        }
    }

    fn update_key_hints(&mut self) {
        if self.focus == Focus::Inspector && self.current_page == Page::QueryEditor {
            if self.inspector_edit_col.is_some() {
                self.key_hints =
                    " Esc:Cancel | Enter:Save | Alt+Z:Undo | Alt+H:Inspector".to_string();
                return;
            }
            let is_json_cell = self
                .query_data_typed
                .get(self.selected_row)
                .and_then(|r| r.get(self.selected_col))
                .and_then(|v| v.json_str())
                .map(|s| s.starts_with('{') || s.starts_with('['))
                .unwrap_or(false);
            if is_json_cell {
                self.key_hints =
                    " j/k:Navigate | Enter:Toggle | h:Collapse | l:Expand | g/G:Top/Bot"
                        .to_string();
            } else {
                self.key_hints =
                    " j/k:Scroll | g/G:Top/Bot | Click:Edit | Alt+Z:Undo | Alt+H:Inspector"
                        .to_string();
            }
        } else {
            self.key_hints =
                " q:Quit | 1-8:Pages | ?:Help | Tab:Focus | Alt-H:Inspector".to_string();
        }
    }

    fn ensure_inspector_cursor_visible(&mut self, visible_w: usize) {
        let cursor = self.inspector_edit_cursor;
        let scroll = self.inspector_edit_scroll_h as usize;
        if cursor < scroll {
            self.inspector_edit_scroll_h = cursor as u16;
        } else if cursor >= scroll + visible_w {
            self.inspector_edit_scroll_h = (cursor.saturating_sub(visible_w) + 1) as u16;
        }
        let max_scroll = self.inspector_edit_value.len().saturating_sub(visible_w);
        if self.inspector_edit_scroll_h as usize > max_scroll {
            self.inspector_edit_scroll_h = max_scroll as u16;
        }
    }

    fn start_cell_edit(&mut self) {
        if self.selected_row < self.query_data.len() && self.selected_col < self.query_columns.len()
        {
            let val = &self.query_data[self.selected_row][self.selected_col];
            self.cell_edit_input = InputField::with_value("", val.clone());
            self.cell_edit_focus = CellEditFocus::Editor;
            self.cell_edit_mode = CellEditMode::Editing;
        }
    }

    fn confirm_cell_edit(&mut self) {
        if self.mongo_edit_path.is_some() {
            self.mongo_confirm_edit();
            return;
        }
        let new_val = self.cell_edit_input.value.clone();
        if self.selected_row < self.query_data.len() && self.selected_col < self.query_columns.len()
        {
            let old_val = self.query_data[self.selected_row][self.selected_col].clone();
            self.query_data[self.selected_row][self.selected_col] = new_val.clone();
            if self.selected_row < self.query_data_typed.len()
                && self.selected_col < self.query_data_typed[self.selected_row].len()
            {
                let typed = &self.query_data_typed[self.selected_row][self.selected_col];
                match typed {
                    Value::Json(_) => {
                        self.query_data_typed[self.selected_row][self.selected_col] =
                            Value::Json(new_val.clone());
                    }
                    _ => {
                        self.query_data_typed[self.selected_row][self.selected_col] =
                            Value::Text(new_val.clone());
                    }
                }
            }
            self.cell_edit_mode = CellEditMode::Inactive;
            self.set_status(format!(
                "Cell [{}, {}] updated: \"{}\" -> \"{}\"",
                self.selected_row, self.selected_col, old_val, new_val
            ));
        }
    }

    fn rebuild_json_tree(&mut self) {
        let cell = (self.selected_row, self.selected_col);
        if self.json_tree_cell == cell {
            return;
        }
        self.json_tree_cell = cell;
        if let Some(typed) = self
            .query_data_typed
            .get(self.selected_row)
            .and_then(|r| r.get(self.selected_col))
        {
            if let Some(json) = typed.json_str() {
                let items = crate::ui::layout::parse_json_tree(json);
                self.json_tree_state = TreeState::new(items);
                return;
            }
        }
        self.json_tree_state = TreeState::new(Vec::new());
    }

    fn ensure_cell_visible(&mut self, data_height: usize) {
        if self.selected_row >= self.result_scroll_v + data_height {
            self.result_scroll_v = self.selected_row.saturating_sub(data_height) + 1;
        } else if self.selected_row < self.result_scroll_v {
            self.result_scroll_v = self.selected_row;
        }

        let col_x: usize = self.query_col_widths[..self.selected_col]
            .iter()
            .copied()
            .sum::<usize>()
            + self.selected_col * 3;
        let view_width = self.main_rect.width.saturating_sub(2) as usize;

        if col_x < self.result_scroll_h as usize {
            self.result_scroll_h = col_x as u16;
        }
        let col_end = col_x + self.query_col_widths[self.selected_col];
        if col_end > self.result_scroll_h as usize + view_width && col_end > view_width {
            self.result_scroll_h = (col_end - view_width) as u16;
        }
    }

    fn mouse_to_cell(&self, mouse_col: u16, mouse_row: u16) -> Option<(usize, usize)> {
        if self.query_data.is_empty() || self.query_columns.is_empty() {
            return None;
        }
        let combined_h = self.main_rect.height;
        let results_area_y = self.main_rect.y + combined_h * 25 / 100;
        let inner_x = self.main_rect.x + 1;
        let inner_y = results_area_y + 1;

        let header_rows = 2u16;
        let data_y = inner_y + header_rows;

        if mouse_row < data_y {
            return None;
        }

        let row_offset = (mouse_row - data_y) as usize;
        let data_row = self.result_scroll_v + row_offset;
        if data_row >= self.query_data.len() {
            return None;
        }

        let mut char_x: i16 = inner_x as i16;
        for (ci, &w) in self.query_col_widths.iter().enumerate() {
            if ci > 0 {
                char_x += 3;
            }
            let cell_start = char_x - self.result_scroll_h as i16;
            let cell_end = cell_start + w as i16;
            let mc = mouse_col as i16;
            if mc >= cell_start && mc < cell_end {
                return Some((data_row, ci));
            }
            char_x += w as i16;
        }

        None
    }

    fn handle_key(&mut self, key: KeyEvent) {
        self.status_message.clear();

        if self.global_search.is_some() && self.global_search.as_ref().unwrap().is_open {
            if crate::ui::search::handle_global_search_key(self, &key) {
                return;
            }
        }

        if self.editing_query && self.current_page == Page::QueryEditor {
            self.handle_query_edit_key(&key);
            return;
        }

        if self.focus == Focus::Sidebar && self.handle_sidebar_key(&key) {
            return;
        }
        if self.current_page == Page::Connections && self.handle_conn_key(&key) {
            return;
        }
        if self.handle_results_key(&key) {
            return;
        }
        if self.focus == Focus::Inspector && self.current_page == Page::QueryEditor {
            if self.handle_inspector_key(&key) || self.inspector_edit_col.is_some() {
                return;
            }
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => {
                self.should_quit = true;
            }
            KeyCode::Char('1') => self.switch_to(Page::Dashboard),
            KeyCode::Char('2') => self.switch_to(Page::Connections),
            KeyCode::Char('3') => self.switch_to(Page::Explorer),
            KeyCode::Char('4') => self.switch_to(Page::QueryEditor),
            KeyCode::Char('5') => self.switch_to(Page::History),
            KeyCode::Char('6') => self.switch_to(Page::Favorites),
            KeyCode::Char('7') => self.switch_to(Page::SchemaBrowser),
            KeyCode::Char('8') => self.switch_to(Page::Logs),
            KeyCode::Char('?') => self.switch_to(Page::Help),
            KeyCode::Tab => self.cycle_focus(),
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                if self.current_page == Page::QueryEditor {
                    self.editing_query = false;
                }
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor && !self.query_text.is_empty() {
                    let sql = self.query_text.clone();
                    self.pending_action = PendingAction::RunQuery(sql);
                }
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.editing_query = true;
                    self.query_editor.set_value(&self.query_text);
                }
            }
            KeyCode::Char('b') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.result_scroll_h = self.result_scroll_h.saturating_sub(5);
                }
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.result_scroll_h = self.result_scroll_h.saturating_add(5);
                }
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.result_scroll_v = self.result_scroll_v.saturating_sub(5);
                }
            }
            KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.result_scroll_v = self.result_scroll_v.saturating_add(5);
                }
            }
            KeyCode::BackTab => {
                self.cycle_focus_reverse();
                self.set_status(format!("Focus: {:?}", self.focus));
            }
            KeyCode::Char('l') if key.modifiers == KeyModifiers::CONTROL => {
                self.cycle_focus();
                self.set_status(format!("Focus: {:?}", self.focus));
            }

            KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
                let mut state = crate::ui::search::GlobalSearchState::new();
                state.is_open = true;
                self.global_search = Some(state);
                crate::ui::search::rebuild_search_index(self);
                self.set_status("Global Search — type to filter, Enter to jump, Esc to close");
            }
            KeyCode::Char('h') if key.modifiers == KeyModifiers::ALT => {
                self.show_inspector = !self.show_inspector;
                self.inspector_scroll = 0;
                if self.show_inspector {
                    self.rebuild_json_tree();
                }
                if !self.show_inspector && self.focus == Focus::Inspector {
                    self.focus = Focus::Main;
                    self.query_editor_focus = QueryEditorFocus::Results;
                }
                self.update_key_hints();
                self.set_status(if self.show_inspector {
                    "Inspector: shown"
                } else {
                    "Inspector: hidden"
                });
            }
            _ => {}
        }
        self.update_key_hints();
    }

    fn auto_load_selected_table_columns(&mut self) {
        if let Some(id) = self.explorer_tree.selected_id() {
            if id.starts_with("table:") && !id.ends_with(':') {
                let table_name = id.strip_prefix("table:").unwrap().to_string();
                if self.metadata_cache.get_columns(&table_name).is_none() {
                    self.pending_action = PendingAction::LoadColumns { table: table_name };
                }
            }
        }
    }

    fn handle_sidebar_key(&mut self, key: &KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.explorer_tree.select_down();
                if self.current_page != Page::QueryEditor {
                    self.auto_load_selected_table_columns();
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.explorer_tree.select_up();
                if self.current_page != Page::QueryEditor {
                    self.auto_load_selected_table_columns();
                }
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let id = self.explorer_tree.selected_id();
                if let Some(ref id) = id {
                    let is_expanded = self.explorer_tree.is_item_expanded(id);
                    if !is_expanded {
                        if id.starts_with("table:") && !id.ends_with(':') {
                            let table_name = id.strip_prefix("table:").unwrap().to_string();
                            self.pending_action = PendingAction::LoadColumns { table: table_name };
                        } else {
                            self.explorer_tree.toggle_selected();
                        }
                    } else {
                        self.explorer_tree.toggle_selected();
                    }
                }
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                let id = self.explorer_tree.selected_id();
                if let Some(ref id) = id {
                    if self.explorer_tree.is_item_expanded(id) {
                        self.explorer_tree.toggle_selected();
                    } else {
                        self.explorer_tree.select_up();
                    }
                }
                true
            }
            KeyCode::Char('g') => {
                self.explorer_tree.go_to_first();
                true
            }
            KeyCode::Char('G') => {
                self.explorer_tree.go_to_last();
                true
            }
            KeyCode::Enter => {
                let id = self.explorer_tree.selected_id();
                if let Some(ref id) = id {
                    if id.starts_with("table:") && !id.ends_with(':') {
                        let table_name = id.strip_prefix("table:").unwrap();
                        let (db_name, short) = match table_name.find('.') {
                            Some(pos) => (&table_name[..pos], &table_name[pos + 1..]),
                            None => ("", table_name),
                        };
                        let query = if self.active_db_type() == Some(DatabaseType::MongoDB) {
                            if db_name.is_empty() {
                                format!("{{ \"find\": \"{}\", \"limit\": 1000 }}\n", short)
                            } else {
                                format!(
                                    "{{ \"db\": \"{}\", \"find\": \"{}\", \"limit\": 1000 }}\n",
                                    db_name, short
                                )
                            }
                        } else {
                            format!("SELECT * FROM \"{}\" LIMIT 1000 OFFSET 0;\n", short)
                        };
                        self.pending_action = PendingAction::OpenQuery(query);
                    } else if id.starts_with("column:") {
                        self.current_page = Page::Explorer;
                        self.set_status(format!("Viewing column details"));
                    } else if !id.ends_with(':') {
                        self.explorer_tree.toggle_selected();
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn handle_conn_key(&mut self, key: &KeyEvent) -> bool {
        match self.conn_page_mode {
            ConnPageMode::Browse => self.handle_conn_browse_key(key),
            ConnPageMode::Adding | ConnPageMode::Editing(_) => self.handle_conn_form_key(key),
            ConnPageMode::Deleting(_) => self.handle_conn_confirm_key(key),
        }
    }

    fn handle_conn_browse_key(&mut self, key: &KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.conn_list_selected + 1 < self.connections.len() {
                    self.conn_list_selected += 1;
                    self.adjust_conn_scroll();
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.conn_list_selected > 0 {
                    self.conn_list_selected -= 1;
                    self.adjust_conn_scroll();
                }
                true
            }
            KeyCode::Char('g') => {
                self.conn_list_selected = 0;
                self.conn_list_scroll = 0;
                true
            }
            KeyCode::Char('G') => {
                if !self.connections.is_empty() {
                    self.conn_list_selected = self.connections.len() - 1;
                    self.adjust_conn_scroll();
                }
                true
            }
            KeyCode::Char('n') => {
                self.conn_form = Some(ConnectionFormState::new());
                self.conn_page_mode = ConnPageMode::Adding;
                true
            }
            KeyCode::Char('e') => {
                if let Some(conn) = self.connections.get(self.conn_list_selected) {
                    let password = conn.password.clone();
                    self.conn_form =
                        Some(ConnectionFormState::from_config(conn, password.as_deref()));
                    self.conn_page_mode = ConnPageMode::Editing(conn.id);
                }
                true
            }
            KeyCode::Char('d') => {
                if !self.connections.is_empty() {
                    self.conn_page_mode = ConnPageMode::Deleting(self.conn_list_selected);
                }
                true
            }
            KeyCode::Enter if key.modifiers == KeyModifiers::ALT => {
                if let Some(conn) = self.connections.get(self.conn_list_selected) {
                    if self.pool.contains(conn.id) {
                        self.pending_action = PendingAction::Disconnect(conn.id);
                    } else {
                        self.set_status(format!("\"{}\" is not connected", conn.name));
                    }
                }
                true
            }
            KeyCode::Enter => {
                if let Some(conn) = self.connections.get(self.conn_list_selected).cloned() {
                    self.pending_action = PendingAction::Connect(conn);
                }
                true
            }
            _ => false,
        }
    }

    fn handle_conn_form_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Tab => {
                if let Some(form) = &mut self.conn_form {
                    form.active_field = (form.active_field + 1) % 7;
                }
                true
            }
            KeyCode::BackTab => {
                if let Some(form) = &mut self.conn_form {
                    form.active_field = if form.active_field == 0 {
                        6
                    } else {
                        form.active_field - 1
                    };
                }
                true
            }
            KeyCode::Enter => {
                self.conn_form_submit();
                true
            }
            KeyCode::Esc => {
                self.conn_page_mode = ConnPageMode::Browse;
                self.conn_form = None;
                true
            }
            KeyCode::Left | KeyCode::Right => {
                if let Some(form) = &mut self.conn_form {
                    if form.active_field == 1 {
                        let idx = DB_TYPE_CYCLE
                            .iter()
                            .position(|t| *t == form.db_type)
                            .unwrap_or(0);
                        let new_idx = match key.code {
                            KeyCode::Left => {
                                if idx == 0 {
                                    DB_TYPE_CYCLE.len() - 1
                                } else {
                                    idx - 1
                                }
                            }
                            _ => (idx + 1) % DB_TYPE_CYCLE.len(),
                        };
                        form.db_type = DB_TYPE_CYCLE[new_idx].clone();
                        if form.port.is_empty() {
                            form.port = form.db_type.default_port().to_string();
                        }
                    }
                }
                true
            }
            _ => {
                if let Some(form) = &mut self.conn_form {
                    if form.active_field != 1 {
                        let val = form.field_value(form.active_field);
                        let mut field = crate::widgets::input::InputField::with_value("", val);
                        field.handle_key(key);
                        form.set_field(form.active_field, field.value);
                    }
                }
                true
            }
        }
    }

    fn handle_conn_confirm_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                if let ConnPageMode::Deleting(idx) = self.conn_page_mode {
                    if idx < self.connections.len() {
                        let name = self.connections[idx].name.clone();
                        let pwd_key = self.connections[idx].password_key();
                        let _ = storage::credentials::delete_password(&pwd_key);
                        self.connections.remove(idx);
                        let _ = storage::connections::save_connections(&self.connections);
                        if self.conn_list_selected >= self.connections.len()
                            && !self.connections.is_empty()
                        {
                            self.conn_list_selected = self.connections.len() - 1;
                        }
                        self.set_status(format!("Deleted connection \"{}\"", name));
                    }
                }
                self.conn_page_mode = ConnPageMode::Browse;
                true
            }
            KeyCode::Esc => {
                self.conn_page_mode = ConnPageMode::Browse;
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        self.status_message.clear();

        let (col, row) = (mouse.column, mouse.row);

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.sidebar_rect.contains((col, row).into()) {
                    self.explorer_tree.select_down();
                } else if self.current_page == Page::QueryEditor
                    && self.main_rect.contains((col, row).into())
                {
                    if self.is_mongo_query() {
                        if let Some(docs) = self.mongo_raw_documents.as_ref() {
                            let total = self.mongo_doc_state.cached_line_count(docs);
                            for _ in 0..3 {
                                if self.mongo_doc_state.selected_line + 1 < total {
                                    self.mongo_doc_state.selected_line += 1;
                                }
                            }
                        }
                        self.mongo_doc_state.scroll_offset =
                            self.mongo_doc_state.scroll_offset.saturating_add(3);
                    } else {
                        self.result_scroll_v = self.result_scroll_v.saturating_add(1);
                    }
                } else if self.main_rect.contains((col, row).into())
                    && self.current_page == Page::Connections
                    && self.conn_list_selected + 1 < self.connections.len()
                {
                    self.conn_list_selected += 1;
                    self.adjust_conn_scroll();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.sidebar_rect.contains((col, row).into()) {
                    self.explorer_tree.select_up();
                } else if self.current_page == Page::QueryEditor
                    && self.main_rect.contains((col, row).into())
                {
                    if self.is_mongo_query() {
                        for _ in 0..3 {
                            if self.mongo_doc_state.selected_line > 0 {
                                self.mongo_doc_state.selected_line -= 1;
                            }
                        }
                        self.mongo_doc_state.scroll_offset =
                            self.mongo_doc_state.scroll_offset.saturating_sub(3);
                    } else {
                        self.result_scroll_v = self.result_scroll_v.saturating_sub(1);
                    }
                } else if self.main_rect.contains((col, row).into())
                    && self.current_page == Page::Connections
                    && self.conn_list_selected > 0
                {
                    self.conn_list_selected -= 1;
                    self.adjust_conn_scroll();
                }
            }
            MouseEventKind::ScrollLeft => {
                if self.current_page == Page::QueryEditor
                    && self.main_rect.contains((col, row).into())
                {
                    self.result_scroll_h = self.result_scroll_h.saturating_sub(5);
                }
            }
            MouseEventKind::ScrollRight => {
                if self.current_page == Page::QueryEditor
                    && self.main_rect.contains((col, row).into())
                {
                    self.result_scroll_h = self.result_scroll_h.saturating_add(5);
                }
            }
            MouseEventKind::Down(_) => {
                if self.sidebar_rect.contains((col, row).into()) {
                    let is_double = self.is_double_click(col, row);
                    self.focus = Focus::Sidebar;
                    if let Some(idx) = self.explorer_tree.hit_test(row, self.sidebar_rect) {
                        self.explorer_tree.selected = idx;
                        self.explorer_tree
                            .adjust_offset(self.sidebar_rect.height.saturating_sub(3) as usize);
                        if is_double {
                            let id = self.explorer_tree.selected_id();
                            if let Some(ref id) = id {
                                if id.starts_with("table:") && !id.ends_with(':') {
                                    let table_name = id.strip_prefix("table:").unwrap();
                                    let (db_name, short) = match table_name.find('.') {
                                        Some(pos) => (&table_name[..pos], &table_name[pos + 1..]),
                                        None => ("", table_name),
                                    };
                                    let query = if self.active_db_type()
                                        == Some(DatabaseType::MongoDB)
                                    {
                                        if db_name.is_empty() {
                                            format!(
                                                "{{ \"find\": \"{}\", \"limit\": 1000 }}\n",
                                                short
                                            )
                                        } else {
                                            format!("{{ \"db\": \"{}\", \"find\": \"{}\", \"limit\": 1000 }}\n", db_name, short)
                                        }
                                    } else {
                                        format!(
                                            "SELECT * FROM \"{}\" LIMIT 1000 OFFSET 0;\n",
                                            short
                                        )
                                    };
                                    self.pending_action = PendingAction::OpenQuery(query);
                                } else {
                                    self.explorer_tree.toggle_selected();
                                }
                            }
                        }
                    }
                } else if self.current_page == Page::QueryEditor {
                    let is_double = self.is_double_click(col, row);
                    let combined_h = self.main_rect.height;
                    let editor_bottom = self.main_rect.y + combined_h * 25 / 100;
                    let inspector_left = self.main_rect.x + self.main_rect.width;

                    if col >= inspector_left {
                        self.focus = Focus::Inspector;
                        self.update_key_hints();
                        self.inspector_edit_col = None;
                        let inner_y = self.inspector_rect.y + 1;
                        let inner_x = self.inspector_rect.x + 1;
                        let inner_h = self.inspector_rect.height.saturating_sub(2);
                        if row >= inner_y && row < inner_y + inner_h {
                            let field_offset = (row - inner_y) / 3;
                            let row_in_field = (row - inner_y) % 3;
                            let actual_col = self.inspector_scroll + field_offset as usize;
                            if row_in_field == 1 && actual_col < self.query_columns.len() {
                                let val = self
                                    .query_data
                                    .get(self.selected_row)
                                    .and_then(|r| r.get(actual_col))
                                    .cloned()
                                    .unwrap_or_default();
                                self.inspector_edit_col = Some(actual_col);
                                self.inspector_edit_value = val;
                                self.inspector_edit_scroll_h = 0;
                                let click_col_offset = col.saturating_sub(inner_x);
                                let cursor = click_col_offset as usize;
                                self.inspector_edit_cursor =
                                    cursor.min(self.inspector_edit_value.len());
                            }
                        }
                    } else if row < editor_bottom {
                        self.focus = Focus::Main;
                        self.update_key_hints();
                        self.query_editor_focus = QueryEditorFocus::Editor;
                        if is_double {
                            self.editing_query = true;
                            self.query_editor.set_value(&self.query_text);
                        }
                    } else {
                        self.inspector_edit_col = None;
                        self.focus = Focus::Main;
                        self.update_key_hints();
                        self.query_editor_focus = QueryEditorFocus::Results;
                        if self.is_mongo_query() {
                            if let Some(line) = self.mongo_mouse_to_line(row) {
                                self.mongo_doc_state.selected_line = line;
                                if is_double {
                                    // Trigger edit via Enter key event
                                    let docs = match self.mongo_raw_documents {
                                        Some(ref docs) => docs,
                                        None => return,
                                    };
                                    let lines = crate::widgets::mongo_doc_list::build_flat_list_ref(
                                        docs,
                                        &self.mongo_doc_state.expanded_fields,
                                    );
                                    if let Some(l) = lines.get(line) {
                                        if !l.is_header && !l.is_separator && !l.is_expandable {
                                            self.mongo_edit_path = Some(l.path_key.clone());
                                            let display =
                                                mongo_value_strip_quotes(&l.value_display)
                                                    .to_string();
                                            self.cell_edit_input =
                                                InputField::with_value("", display.into());
                                            self.cell_edit_mode = CellEditMode::Editing;
                                            self.cell_edit_focus = CellEditFocus::Editor;
                                            self.set_status("Editing value");
                                        }
                                    }
                                }
                            }
                        } else if let Some((cell_row, cell_col)) = self.mouse_to_cell(col, row) {
                            self.selected_row = cell_row;
                            self.selected_col = cell_col;
                            let data_height =
                                self.main_rect.height.saturating_sub(2).saturating_sub(2) as usize;
                            self.ensure_cell_visible(data_height);
                            if is_double {
                                self.start_cell_edit();
                            }
                        }
                    }
                } else if self.inspector_rect.contains((col, row).into()) {
                    self.focus = Focus::Inspector;
                    self.update_key_hints();
                } else if self.main_rect.contains((col, row).into()) {
                    let is_double = self.is_double_click(col, row);
                    if self.current_page == Page::Connections
                        && self.conn_page_mode == ConnPageMode::Browse
                    {
                        self.focus = Focus::Main;
                        let inner_top = self.main_rect.y + 1;
                        if row >= inner_top {
                            let list_row = (row - inner_top) as usize;
                            let idx = self.conn_list_scroll + list_row;
                            if idx < self.connections.len() {
                                self.conn_list_selected = idx;
                                if is_double {
                                    if let Some(conn) =
                                        self.connections.get(self.conn_list_selected).cloned()
                                    {
                                        self.pending_action = PendingAction::Connect(conn);
                                    }
                                }
                            }
                        }
                    } else {
                        self.focus = Focus::Main;
                    }
                }
            }
            _ => {}
        }
        self.update_key_hints();
        self.clamp_result_scroll();
    }

    fn is_double_click(&mut self, col: u16, row: u16) -> bool {
        let now = Instant::now();
        let is_double = self
            .last_click
            .map(|(c, r, t)| {
                c == col && r == row && now.duration_since(t) < Duration::from_millis(400)
            })
            .unwrap_or(false);
        self.last_click = Some((col, row, now));
        is_double
    }

    fn conn_form_submit(&mut self) {
        let form = match &self.conn_form {
            Some(f) => f.clone(),
            None => return,
        };

        if form.name.trim().is_empty() {
            if let Some(f) = &mut self.conn_form {
                f.error = Some("Name is required".to_string());
            }
            return;
        }
        if form.database.trim().is_empty() {
            if let Some(f) = &mut self.conn_form {
                f.error = Some("Database is required".to_string());
            }
            return;
        }

        let is_new = form.is_new;

        // Resolve password: use form password if non-empty, otherwise preserve existing
        let password = if !form.password.is_empty() {
            form.password.clone()
        } else if !is_new {
            self.connections
                .iter()
                .find(|c| c.id == form.editing_id.unwrap_or_default())
                .and_then(|c| c.password.clone())
                .or_else(|| {
                    storage::credentials::get_password(
                        &form
                            .editing_id
                            .map(|id| format!("dbrusty/{}", id))
                            .unwrap_or_default(),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        let mut config = form.to_config();
        if !password.is_empty() {
            config.password = Some(password.clone());
        }

        if is_new {
            self.connections.push(config.clone());
        } else if let Some(existing) = self.connections.iter_mut().find(|c| c.id == config.id) {
            let prev_password_key = existing.password_key();
            *existing = config.clone();
            if prev_password_key != config.password_key() {
                let _ = storage::credentials::delete_password(&prev_password_key);
            }
        }

        if !password.is_empty() {
            if let Err(e) = storage::credentials::save_password(&config.password_key(), &password) {
                tracing::warn!("Failed to save password to keychain: {}", e);
            }
        }

        let _ = storage::connections::save_connections(&self.connections);
        self.conn_page_mode = ConnPageMode::Browse;
        self.conn_form = None;

        if is_new {
            self.conn_list_selected = self.connections.len() - 1;
            self.adjust_conn_scroll();
            self.set_status(format!(
                "Added \"{}\" — press Enter to connect",
                config.name
            ));
        } else {
            self.set_status(format!("Updated connection \"{}\"", config.name));
        }
    }

    fn adjust_conn_scroll(&mut self) {
        let visible_height: usize = 20;
        if self.conn_list_selected < self.conn_list_scroll {
            self.conn_list_scroll = self.conn_list_selected;
        } else if self.conn_list_selected >= self.conn_list_scroll + visible_height {
            self.conn_list_scroll = self.conn_list_selected - visible_height + 1;
        }
    }

    fn handle_resize(&mut self, _w: u16, _h: u16) {
        self.set_status(format!("Resized to {}x{}", _w, _h));
    }

    fn switch_to(&mut self, page: Page) {
        if page == Page::Connections {
            self.conn_page_mode = ConnPageMode::Browse;
        }
        self.current_page = page;
        self.update_key_hints();
        self.set_status(format!("Switched to {}", page.title()));
    }

    fn cycle_focus(&mut self) {
        if self.current_page == Page::QueryEditor {
            match self.focus {
                Focus::Sidebar => {
                    self.focus = Focus::Main;
                    self.query_editor_focus = QueryEditorFocus::Editor;
                }
                Focus::Main => match self.query_editor_focus {
                    QueryEditorFocus::Editor => {
                        self.query_editor_focus = QueryEditorFocus::Results;
                    }
                    QueryEditorFocus::Results => {
                        if self.show_inspector {
                            self.focus = Focus::Inspector;
                            self.rebuild_json_tree();
                        } else {
                            self.focus = Focus::Sidebar;
                        }
                    }
                },
                Focus::Inspector => {
                    self.rebuild_json_tree();
                    self.focus = Focus::Sidebar;
                }
            }
        } else {
            self.focus = match self.focus {
                Focus::Sidebar => Focus::Main,
                Focus::Main => {
                    if self.show_inspector {
                        Focus::Inspector
                    } else {
                        Focus::Sidebar
                    }
                }
                Focus::Inspector => Focus::Sidebar,
            };
        }
        self.update_key_hints();
        self.set_status(format!("Focus: {:?}", self.focus));
    }

    fn cycle_focus_reverse(&mut self) {
        if self.current_page == Page::QueryEditor {
            match self.focus {
                Focus::Sidebar => {
                    if self.show_inspector {
                        self.focus = Focus::Inspector;
                        self.rebuild_json_tree();
                    } else {
                        self.focus = Focus::Main;
                        self.query_editor_focus = QueryEditorFocus::Results;
                    }
                }
                Focus::Inspector => {
                    self.focus = Focus::Main;
                    self.query_editor_focus = QueryEditorFocus::Results;
                }
                Focus::Main => match self.query_editor_focus {
                    QueryEditorFocus::Results => {
                        self.query_editor_focus = QueryEditorFocus::Editor;
                    }
                    QueryEditorFocus::Editor => {
                        self.focus = Focus::Sidebar;
                    }
                },
            }
        } else {
            self.focus = match self.focus {
                Focus::Sidebar => {
                    if self.show_inspector {
                        Focus::Inspector
                    } else {
                        Focus::Main
                    }
                }
                Focus::Main => Focus::Sidebar,
                Focus::Inspector => Focus::Main,
            };
        }
        self.update_key_hints();
        self.set_status(format!("Focus: {:?}", self.focus));
    }
}
