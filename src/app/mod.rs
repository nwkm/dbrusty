use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_METADATA_TIMEOUT: Duration = Duration::from_secs(15);
const QUERY_TIMEOUT: Duration = Duration::from_secs(600);

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

use crate::completion::CompletionState;
use crate::config::AppConfig;
use crate::database::metadata::MetadataCache;
use crate::database::pool::ConnectionPool;
use crate::database::provider::DatabaseProvider;
use crate::database::result::{QueryResult, Value};
use crate::database::types::{ColumnInfo, TableInfo, ViewInfo};
use crate::models::connection::{ConnectionConfig, DatabaseType};
use crate::providers::mariadb::MariadbProvider;
use crate::providers::mongodb::MongoProvider;
use crate::providers::mysql::MysqlProvider;
use crate::providers::postgres::PostgresProvider;
use crate::providers::redis::RedisProvider;
use crate::providers::sqlite::SqliteProvider;
use crate::storage;
use crate::ui;
use crate::widgets::input::InputField;
use crate::widgets::mongo_doc_list::MongoDocListState;
use crate::widgets::tree::TreeState;
use ratatui_textarea::TextArea;

enum BackgroundEvent {
    Connected {
        id: Uuid,
        provider: Box<dyn DatabaseProvider>,
        conn_name: String,
        databases: Vec<String>,
        schemas_data: Vec<(String, Vec<String>)>,
        tables_data: Vec<(String, Vec<TableInfo>)>,
        views_data: Vec<(String, Vec<ViewInfo>)>,
        sequences_data: Vec<(String, Vec<crate::database::types::SequenceInfo>)>,
        triggers_data: Vec<(String, Vec<crate::database::types::TriggerInfo>)>,
        functions_data: Vec<(String, Vec<crate::database::types::FunctionInfo>)>,
        warnings: Vec<String>,
    },
    ConnectFailed {
        conn_name: String,
        error: String,
    },
    QueryDone {
        provider_id: Uuid,
        provider: Box<dyn DatabaseProvider>,
        result: QueryResult,
    },
    QueryFailed {
        provider_id: Uuid,
        provider: Box<dyn DatabaseProvider>,
        error: String,
    },
    QueryAborted {
        provider_id: Uuid,
        provider: Box<dyn DatabaseProvider>,
        reason: String,
    },
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
    Search,
    Visual,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormTab {
    Form,
    Uri,
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
    pub form_tab: FormTab,
    pub uri_input: String,
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
            form_tab: FormTab::Form,
            uri_input: String::new(),
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
            form_tab: FormTab::Form,
            uri_input: conn.connection_string(password).unwrap_or_default(),
        }
    }

    pub fn field_value(&self, idx: usize) -> &str {
        match idx {
            0 => &self.name,
            1 => self.db_type.display_name(),
            2 => &self.host,
            3 => &self.port,
            4 => &self.database,
            5 => &self.username,
            6 => &self.password,
            _ => "",
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

    pub fn build_uri(&self) -> String {
        let pw = if self.password.is_empty() {
            String::new()
        } else {
            self.password.clone()
        };
        match self.db_type {
            DatabaseType::Postgres => {
                format!(
                    "postgres://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                )
            }
            DatabaseType::MySQL => {
                format!(
                    "mysql://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                )
            }
            DatabaseType::MariaDB => {
                format!(
                    "mariadb://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                )
            }
            DatabaseType::SQLite => {
                format!("sqlite://{}", self.database)
            }
            DatabaseType::SQLServer => {
                format!(
                    "sqlserver://{}:{}@{}:{}/{}",
                    self.username, pw, self.host, self.port, self.database
                )
            }
            DatabaseType::MongoDB => {
                if self.username.is_empty() && pw.is_empty() {
                    format!("mongodb://{}:{}/{}", self.host, self.port, self.database)
                } else {
                    format!(
                        "mongodb://{}:{}@{}:{}/{}",
                        self.username, pw, self.host, self.port, self.database
                    )
                }
            }
            DatabaseType::Redis => {
                let db = if self.database.is_empty() {
                    "0".to_string()
                } else {
                    self.database.clone()
                };
                if pw.is_empty() {
                    format!("redis://{}:{}/{}", self.host, self.port, db)
                } else {
                    format!("redis://:{}@{}:{}/{}", pw, self.host, self.port, db)
                }
            }
        }
    }
}

fn parse_connection_uri(
    uri: &str,
) -> Result<(DatabaseType, String, u16, String, String, String), String> {
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or_else(|| "Invalid URI: missing ://".to_string())?;

    let db_type = match scheme.to_lowercase().as_str() {
        "postgres" | "postgresql" => DatabaseType::Postgres,
        "mysql" => DatabaseType::MySQL,
        "mariadb" => DatabaseType::MariaDB,
        "sqlite" => DatabaseType::SQLite,
        "mongodb" => DatabaseType::MongoDB,
        "redis" => DatabaseType::Redis,
        "sqlserver" => DatabaseType::SQLServer,
        _ => return Err(format!("Unknown database scheme: {}", scheme)),
    };

    if db_type == DatabaseType::SQLite {
        return Ok((
            db_type,
            String::new(),
            0,
            rest.to_string(),
            String::new(),
            String::new(),
        ));
    }

    let (auth_and_host, database) = if let Some(idx) = rest.find('/') {
        (&rest[..idx], &rest[idx + 1..])
    } else {
        (rest, "")
    };

    let (userinfo, hostport) = if let Some(idx) = auth_and_host.rfind('@') {
        (&auth_and_host[..idx], &auth_and_host[idx + 1..])
    } else {
        ("", auth_and_host)
    };

    let (username, password) = if let Some(idx) = userinfo.find(':') {
        (userinfo[..idx].to_string(), userinfo[idx + 1..].to_string())
    } else if !userinfo.is_empty() {
        (userinfo.to_string(), String::new())
    } else {
        (String::new(), String::new())
    };

    let (host, port) = if let Some(idx) = hostport.rfind(':') {
        let port_str = &hostport[idx + 1..];
        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("Invalid port: {}", port_str))?;
        (hostport[..idx].to_string(), port)
    } else {
        let host = hostport.to_string();
        let port = if host.is_empty() {
            db_type.default_port()
        } else {
            db_type.default_port()
        };
        (host, port)
    };

    let host = if host.is_empty() {
        "localhost".into()
    } else {
        host
    };

    Ok((
        db_type,
        host,
        port,
        database.to_string(),
        username,
        password,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaBrowserCategory {
    Tables,
    Views,
    Indexes,
    Sequences,
    Triggers,
    Functions,
    Procedures,
}

impl SchemaBrowserCategory {
    pub fn label(&self) -> &'static str {
        match self {
            SchemaBrowserCategory::Tables => "Tables",
            SchemaBrowserCategory::Views => "Views",
            SchemaBrowserCategory::Indexes => "Indexes",
            SchemaBrowserCategory::Sequences => "Sequences",
            SchemaBrowserCategory::Triggers => "Triggers",
            SchemaBrowserCategory::Functions => "Functions",
            SchemaBrowserCategory::Procedures => "Procedures",
        }
    }

    pub fn all() -> &'static [SchemaBrowserCategory] {
        &[
            SchemaBrowserCategory::Tables,
            SchemaBrowserCategory::Views,
            SchemaBrowserCategory::Indexes,
            SchemaBrowserCategory::Sequences,
            SchemaBrowserCategory::Triggers,
            SchemaBrowserCategory::Functions,
            SchemaBrowserCategory::Procedures,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaBrowserMode {
    Browse,
    CreatingTable,
    DroppingTable(String),
    RenamingTable(String),
    AlteringTable(String),
    ImportingSchema,
    SchemaDiff,
}

#[derive(Debug, Clone)]
pub struct SchemaBrowserState {
    pub mode: SchemaBrowserMode,
    pub selected_category: usize,
    pub selected_item: usize,
    pub items: Vec<String>,
    pub ddl_text: String,
    pub scroll: usize,
    pub create_name: String,
    pub create_field: usize,
    pub create_error: Option<String>,
    pub item_scroll: usize,
    // Alter table
    pub alter_action: usize,
    pub alter_column_name: String,
    pub alter_column_type: String,
    pub alter_focus: usize,
    // Import schema
    pub import_sql: String,
    pub import_cursor: usize,
    // Schema diff
    pub diff_snapshot_a: String,
    pub diff_snapshot_b: String,
    pub diff_result: String,
    // Export / Reverse engineer
    pub export_ddl: String,
}

impl SchemaBrowserState {
    pub fn new() -> Self {
        Self {
            mode: SchemaBrowserMode::Browse,
            selected_category: 0,
            selected_item: 0,
            items: Vec::new(),
            ddl_text: String::new(),
            scroll: 0,
            create_name: String::new(),
            create_field: 0,
            create_error: None,
            item_scroll: 0,
            alter_action: 0,
            alter_column_name: String::new(),
            alter_column_type: String::new(),
            alter_focus: 0,
            import_sql: String::new(),
            import_cursor: 0,
            diff_snapshot_a: String::new(),
            diff_snapshot_b: String::new(),
            diff_result: String::new(),
            export_ddl: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuAction {
    ViewDdl,
    AlterTable,
    DropTable,
    RenameTable,
    CreateTable,
    GenerateDdl,
    SchemaDiff,
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub visible: bool,
    pub actions: Vec<ContextMenuAction>,
    pub selected: usize,
    pub x: u16,
    pub y: u16,
}

impl ContextMenu {
    pub fn new() -> Self {
        Self {
            visible: false,
            actions: Vec::new(),
            selected: 0,
            x: 0,
            y: 0,
        }
    }
}

pub enum PendingAction {
    None,
    Connect(ConnectionConfig),
    Disconnect(Uuid),
    LoadColumns {
        table: String,
    },
    CacheColumns {
        table: String,
    },
    OpenQuery(String),
    RunQuery(String),
    CancelQuery,
    GenerateDdl(String),
    ExportResults {
        format: crate::export::ExportFormat,
        filename: String,
    },
}

#[derive(Debug, Clone)]
pub struct ExportDialog {
    pub format: usize,
    pub filename: String,
    pub cursor: usize,
}

impl ExportDialog {
    pub fn new() -> Self {
        Self {
            format: 0,
            filename: String::from("export"),
            cursor: 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportFileDialog {
    pub path: String,
    pub cursor: usize,
    pub preview: Option<crate::import::ImportPreview>,
    pub confirmed: bool,
}

impl ImportFileDialog {
    pub fn new() -> Self {
        Self {
            path: String::new(),
            cursor: 0,
            preview: None,
            confirmed: false,
        }
    }
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
    pub config: AppConfig,
    pub should_quit: bool,
    pub current_page: Page,
    pub input_mode: InputMode,
    pub focus: Focus,
    pub status_message: String,
    pub key_hints: String,
    pub pool: ConnectionPool,
    pub connections: Vec<ConnectionConfig>,
    pub active_db_type_cache: Option<DatabaseType>,
    pub conn_list_selected: usize,
    pub conn_list_scroll: usize,
    pub conn_page_mode: ConnPageMode,
    pub conn_form: Option<ConnectionFormState>,
    pub pending_action: PendingAction,
    pub query_cancel_tx: Option<watch::Sender<bool>>,
    pub disconnecting: HashSet<Uuid>,
    pub explorer_tree: TreeState,
    pub metadata_cache: MetadataCache,
    pub sidebar_rect: Rect,
    pub main_rect: Rect,
    pub inspector_rect: Rect,
    pub last_click: Option<(u16, u16, Instant)>,
    pub log_entries: VecDeque<String>,
    pub log_scroll: usize,
    pub log_filter_active: bool,
    pub log_filter_input: String,
    pub log_visible_height: usize,
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
    pub show_query_editor: bool,
    pub result_filter_active: bool,
    pub result_filter_input: String,
    pub result_filter_cursor: usize,
    pub result_filtered_indices: Vec<usize>,
    pub result_filter_highlights: Vec<Vec<Vec<u32>>>,
    pub inspector_scroll: usize,
    pub inspector_edit_col: Option<usize>,
    pub inspector_edit_value: String,
    pub inspector_edit_cursor: usize,
    pub inspector_edit_scroll_h: u16,
    pub inspector_saved_value: Option<(usize, usize, String)>,
    pub cell_edit_mode: CellEditMode,
    pub cell_edit_input: InputField,
    pub cell_edit_focus: CellEditFocus,
    pub query_editor: TextArea<'static>,
    pub editing_query: bool,
    pub query_editor_focus: QueryEditorFocus,
    pub completion_state: CompletionState,
    pub result_scroll_v: usize,
    pub result_scroll_h: u16,
    pub json_tree_state: TreeState,
    pub json_tree_cell: (usize, usize),
    pub mongo_raw_documents: Option<Vec<bson::Document>>,
    pub mongo_doc_state: MongoDocListState,
    pub mongo_edit_path: Option<String>,
    pub pending_j: bool,
    pub pending_d: bool,
    pub is_busy: bool,
    background_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
    // M17: Schema management
    pub schema_browser: SchemaBrowserState,
    pub context_menu: ContextMenu,
    pub export_sql: String,
    // M18: Import/Export
    pub export_dialog: Option<ExportDialog>,
    pub import_dialog: Option<ImportFileDialog>,
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
        self.pool
            .active_id
            .and_then(|id| {
                self.connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.db_type.clone())
            })
            .or(self.active_db_type_cache.clone())
    }

    pub fn is_mongo_query(&self) -> bool {
        self.mongo_raw_documents.is_some()
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.log_entries.len() >= 500 {
            self.log_entries.pop_front();
        }
        self.status_message.clone_from(&msg);
        self.log_entries.push_back(msg);
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
        let total = if self.has_result_filter() {
            self.result_filtered_indices.len()
        } else {
            self.query_data.len()
        };
        if total > 0 {
            self.result_scroll_v = self.result_scroll_v.min(total.saturating_sub(1));
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
            key_hints: String::from(
                " Ctrl+Q:Quit | 1-8:Pages | ?:Help | Tab:Focus | Alt-H:Inspector | Alt-I:Editor",
            ),
            pool: ConnectionPool::new(),
            connections,
            active_db_type_cache: None,
            conn_list_selected: 0,
            conn_list_scroll: 0,
            conn_page_mode: ConnPageMode::Browse,
            conn_form: None,
            pending_action: PendingAction::None,
            query_cancel_tx: None,
            disconnecting: HashSet::new(),
            explorer_tree: TreeState::new(Vec::new()),
            metadata_cache: MetadataCache::new(),
            sidebar_rect: Rect::default(),
            main_rect: Rect::default(),
            inspector_rect: Rect::default(),
            last_click: None,
            log_entries: VecDeque::new(),
            log_scroll: 0,
            log_filter_active: false,
            log_filter_input: String::new(),
            log_visible_height: 20,
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
            show_query_editor: false,
            result_filter_active: false,
            result_filter_input: String::new(),
            result_filter_cursor: 0,
            result_filtered_indices: Vec::new(),
            result_filter_highlights: Vec::new(),
            inspector_scroll: 0,
            inspector_edit_col: None,
            inspector_edit_value: String::new(),
            inspector_edit_cursor: 0,
            inspector_edit_scroll_h: 0,
            inspector_saved_value: None,
            cell_edit_mode: CellEditMode::Inactive,
            cell_edit_input: InputField::new(""),
            cell_edit_focus: CellEditFocus::Editor,
            query_editor: {
                let mut ta = TextArea::default();
                ta.set_placeholder_text("Enter SQL query...");
                ta
            },
            editing_query: false,
            query_editor_focus: QueryEditorFocus::Editor,
            completion_state: CompletionState::new(),
            result_scroll_v: 0,
            result_scroll_h: 0,
            json_tree_state: TreeState::new(Vec::new()),
            json_tree_cell: (usize::MAX, usize::MAX),
            mongo_raw_documents: None,
            mongo_doc_state: MongoDocListState::new(),
            mongo_edit_path: None,
            pending_j: false,
            pending_d: false,
            is_busy: false,
            export_dialog: None,
            import_dialog: None,
            background_rx: {
                let (_tx, rx) = mpsc::unbounded_channel();
                rx
            },
            schema_browser: SchemaBrowserState::new(),
            context_menu: ContextMenu::new(),
            export_sql: String::new(),
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

            self.check_background_results();
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

    fn spawn_connect(&mut self, conn: ConnectionConfig) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.background_rx = rx;
        let conn_name = conn.name.clone();

        let mut provider: Box<dyn DatabaseProvider> = match conn.db_type {
            DatabaseType::Postgres => Box::new(PostgresProvider::new()),
            DatabaseType::SQLite => Box::new(SqliteProvider::new()),
            DatabaseType::MySQL => Box::new(MysqlProvider::new()),
            DatabaseType::MariaDB => Box::new(MariadbProvider::new()),
            DatabaseType::MongoDB => Box::new(MongoProvider::new()),
            DatabaseType::Redis => Box::new(RedisProvider::new()),
            DatabaseType::SQLServer => {
                self.set_status(format!(
                    "\"{}\" is {} (not yet supported).",
                    conn.name,
                    conn.db_type.display_name()
                ));
                return;
            }
        };

        self.is_busy = true;
        self.set_status(format!("Connecting to \"{}\"...", conn_name));

        tokio::spawn(async move {
            let connect_result =
                tokio::time::timeout(CONNECT_TIMEOUT, provider.connect(&conn)).await;

            match connect_result {
                Ok(Ok(())) => {
                    let mut warnings: Vec<String> = Vec::new();

                    // Run a metadata call under a timeout so a wedged server cannot
                    // hang the connection phase forever. Warnings are surfaced to
                    // the user instead of silently failing.
                    async fn timed<T>(
                        warnings: &mut Vec<String>,
                        label: &str,
                        fut: impl std::future::Future<
                            Output = crate::database::provider::DatabaseResult<T>,
                        >,
                    ) -> Option<T> {
                        match tokio::time::timeout(CONNECT_METADATA_TIMEOUT, fut).await {
                            Ok(Ok(v)) => Some(v),
                            Ok(Err(e)) => {
                                warnings.push(format!("Failed to load {}: {}", label, e));
                                None
                            }
                            Err(_) => {
                                warnings.push(format!(
                                    "Timed out loading {} after {}s",
                                    label,
                                    CONNECT_METADATA_TIMEOUT.as_secs()
                                ));
                                None
                            }
                        }
                    }

                    let mut tables_data = Vec::new();
                    let mut views_data = Vec::new();
                    let mut sequences_data: Vec<(
                        String,
                        Vec<crate::database::types::SequenceInfo>,
                    )> = Vec::new();
                    let mut triggers_data: Vec<(String, Vec<crate::database::types::TriggerInfo>)> =
                        Vec::new();
                    let mut functions_data: Vec<(
                        String,
                        Vec<crate::database::types::FunctionInfo>,
                    )> = Vec::new();
                    let mut schemas_data: Vec<(String, Vec<String>)> = Vec::new();

                    let databases = timed(&mut warnings, "databases", provider.get_databases())
                        .await
                        .unwrap_or_default();

                    for db in &databases {
                        let mut schemas = timed(
                            &mut warnings,
                            &format!("schemas for {}", db),
                            provider.get_schemas(),
                        )
                        .await
                        .unwrap_or_default();
                        if schemas.is_empty() {
                            schemas = vec![db.clone()];
                        }
                        schemas_data.push((db.clone(), schemas.clone()));

                        for schema in &schemas {
                            let schema_key = format!("{}.{}", db, schema);
                            if let Some(tables) = timed(
                                &mut warnings,
                                &format!("tables for {}", schema_key),
                                provider.get_tables(schema),
                            )
                            .await
                            {
                                tables_data.push((schema_key.clone(), tables));
                            }
                            if let Some(views) = timed(
                                &mut warnings,
                                &format!("views for {}", schema_key),
                                provider.get_views(schema),
                            )
                            .await
                            {
                                views_data.push((schema_key.clone(), views));
                            }
                            if let Some(sequences) = timed(
                                &mut warnings,
                                &format!("sequences for {}", schema_key),
                                provider.get_sequences(schema),
                            )
                            .await
                            {
                                sequences_data.push((schema_key.clone(), sequences));
                            }
                            if let Some(triggers) = timed(
                                &mut warnings,
                                &format!("triggers for {}", schema_key),
                                provider.get_triggers(schema),
                            )
                            .await
                            {
                                triggers_data.push((schema_key.clone(), triggers));
                            }
                            if let Some(functions) = timed(
                                &mut warnings,
                                &format!("functions for {}", schema_key),
                                provider.get_functions(schema),
                            )
                            .await
                            {
                                functions_data.push((schema_key.clone(), functions));
                            }
                        }
                    }

                    let _ = tx.send(BackgroundEvent::Connected {
                        id: conn.id,
                        provider,
                        conn_name,
                        databases,
                        schemas_data,
                        tables_data,
                        views_data,
                        sequences_data,
                        triggers_data,
                        functions_data,
                        warnings,
                    });
                }
                Ok(Err(e)) => {
                    let _ = tx.send(BackgroundEvent::ConnectFailed {
                        conn_name,
                        error: e.to_string(),
                    });
                }
                Err(_) => {
                    let _ = tx.send(BackgroundEvent::ConnectFailed {
                        conn_name,
                        error: format!(
                            "Connection timed out after {} seconds",
                            CONNECT_TIMEOUT.as_secs()
                        ),
                    });
                }
            }
        });
    }

    fn spawn_query(&mut self, sql: String) {
        let active_id = match self.pool.active_id {
            Some(id) => id,
            None => {
                self.set_status("No active connection");
                return;
            }
        };
        let provider = match self.pool.take(active_id) {
            Some(p) => p,
            None => {
                self.set_status("Provider not available");
                return;
            }
        };

        let (tx, rx) = mpsc::unbounded_channel();
        self.background_rx = rx;

        // Watch channel lets the UI cancel an in-flight query. A new query
        // replaces the previous cancellation handle.
        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.query_cancel_tx = Some(cancel_tx);

        self.is_busy = true;
        self.set_status("Running query...");

        enum QueryOutcome {
            Done(QueryResult),
            Cancelled,
            Failed(String),
        }

        tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;
            let outcome = tokio::time::timeout(QUERY_TIMEOUT, async {
                tokio::select! {
                    _ = cancel_rx.changed() => QueryOutcome::Cancelled,
                    res = provider.execute_query(&sql) => match res {
                        Ok(result) => QueryOutcome::Done(result),
                        Err(e) => QueryOutcome::Failed(e.to_string()),
                    },
                }
            })
            .await;

            match outcome {
                Ok(QueryOutcome::Done(result)) => {
                    let _ = tx.send(BackgroundEvent::QueryDone {
                        provider_id: active_id,
                        provider,
                        result,
                    });
                }
                Ok(QueryOutcome::Cancelled) => {
                    let _ = tx.send(BackgroundEvent::QueryAborted {
                        provider_id: active_id,
                        provider,
                        reason: "cancelled".to_string(),
                    });
                }
                Ok(QueryOutcome::Failed(e)) => {
                    let _ = tx.send(BackgroundEvent::QueryFailed {
                        provider_id: active_id,
                        provider,
                        error: e,
                    });
                }
                Err(_) => {
                    let _ = tx.send(BackgroundEvent::QueryAborted {
                        provider_id: active_id,
                        provider,
                        reason: format!("timed out after {}s", QUERY_TIMEOUT.as_secs()),
                    });
                }
            }
        });
    }

    async fn process_pending_actions(&mut self) {
        let action = std::mem::replace(&mut self.pending_action, PendingAction::None);
        match action {
            PendingAction::Connect(conn) => {
                if self.is_busy {
                    self.set_status("Already busy, please wait...");
                    self.pending_action = PendingAction::Connect(conn);
                    return;
                }
                self.spawn_connect(conn);
            }
            PendingAction::LoadColumns { table } => {
                self.load_columns(&table).await;
            }
            PendingAction::CacheColumns { table } => {
                self.cache_columns(&table).await;
            }
            PendingAction::OpenQuery(query) => {
                self.query_text.clone_from(&query);
                let lines: Vec<String> = query.lines().map(String::from).collect();
                self.query_editor = TextArea::from(lines);
                self.current_page = Page::QueryEditor;
                self.query_editor_focus = QueryEditorFocus::Results;
                if self.is_busy {
                    self.set_status("Already busy, please wait...");
                    self.pending_action = PendingAction::OpenQuery(query);
                    return;
                }
                self.spawn_query(query);
            }
            PendingAction::RunQuery(sql) => {
                if self.is_busy {
                    self.set_status("Already busy, please wait...");
                    self.pending_action = PendingAction::RunQuery(sql);
                    return;
                }
                self.spawn_query(sql);
            }
            PendingAction::CancelQuery => {
                if let Some(tx) = &self.query_cancel_tx {
                    let _ = tx.send(true);
                    self.set_status("Cancelling query...");
                } else if self.is_busy {
                    self.set_status("Connection in progress; it will time out automatically.");
                } else {
                    self.set_status("No query to cancel.");
                }
            }
            PendingAction::GenerateDdl(schema) => {
                if self.is_busy {
                    self.set_status("Already busy, please wait...");
                    self.pending_action = PendingAction::GenerateDdl(schema);
                    return;
                }
                let active_id = match self.pool.active_id {
                    Some(id) => id,
                    None => {
                        self.set_status("No active connection.");
                        return;
                    }
                };
                let provider = match self.pool.take(active_id) {
                    Some(p) => p,
                    None => {
                        self.set_status("Provider not available.");
                        return;
                    }
                };
                self.is_busy = true;
                self.set_status("Generating schema DDL...");
                match provider.get_full_schema_ddl(&schema).await {
                    Ok(ddl) => {
                        self.schema_browser.export_ddl = ddl;
                        self.schema_browser.ddl_text = self.schema_browser.export_ddl.clone();
                        self.schema_browser.mode = SchemaBrowserMode::Browse;
                        self.set_status("Schema DDL generated.");
                    }
                    Err(e) => {
                        self.set_status(format!("Failed to generate DDL: {}", e));
                    }
                }
                self.pool.give_back(active_id, provider);
                self.is_busy = false;
            }
            PendingAction::Disconnect(id) => {
                let name = self
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                if let Some(mut provider) = self.pool.take(id) {
                    match provider.disconnect().await {
                        Ok(()) => {
                            self.after_disconnect(id);
                            self.set_status(format!("Disconnected from \"{}\"", name));
                        }
                        Err(e) => {
                            // Keep the connection registered so a failed disconnect
                            // does not silently lose the provider.
                            self.pool.register(id, provider);
                            self.pool.set_active(id);
                            self.set_status(format!("Error disconnecting: {}", e));
                        }
                    }
                } else {
                    // The provider is checked out for an in-flight query; queue the
                    // disconnect so the connection is not re-activated on completion.
                    self.disconnecting.insert(id);
                    self.set_status(format!(
                        "Will disconnect \"{}\" after the running query finishes.",
                        name
                    ));
                }
            }
            PendingAction::ExportResults { format, filename } => {
                if self.query_columns.is_empty() || self.query_data.is_empty() {
                    self.set_status("No data to export.");
                    return;
                }
                match crate::export::export_data(
                    format,
                    &self.query_columns,
                    &self.query_data,
                    &filename,
                ) {
                    Ok(()) => {
                        self.set_status(format!(
                            "Exported {} rows to {}",
                            self.query_data.len(),
                            filename
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("Export error: {}", e));
                    }
                }
            }
            PendingAction::None => {}
        }
    }

    fn check_background_results(&mut self) {
        loop {
            let event = match self.background_rx.try_recv() {
                Ok(event) => event,
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.is_busy = false;
                    break;
                }
            };

            self.is_busy = false;

            match event {
                BackgroundEvent::Connected {
                    id,
                    provider,
                    conn_name,
                    databases,
                    schemas_data,
                    tables_data,
                    views_data,
                    sequences_data,
                    triggers_data,
                    functions_data,
                    warnings,
                } => {
                    if self.disconnecting.remove(&id) {
                        // The user queued a disconnect while the connect was in
                        // flight; drop the fresh connection instead of activating it.
                        self.after_disconnect(id);
                        self.set_status(format!("Disconnected from \"{}\"", conn_name));
                        return;
                    }
                    self.pool.register(id, provider);
                    self.pool.set_active(id);
                    self.active_db_type_cache = self
                        .connections
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.db_type.clone());
                    self.query_text.clear();
                    self.query_editor = TextArea::default();

                    self.explorer_tree = crate::ui::sidebar::build_initial_tree(
                        &conn_name,
                        &databases,
                        &schemas_data,
                        &tables_data,
                        &views_data,
                    );

                    for (schema_key, tables) in tables_data {
                        self.metadata_cache.set_tables(schema_key, tables);
                    }
                    for (schema_key, views) in views_data {
                        self.metadata_cache.set_views(schema_key, views);
                    }
                    for (schema_key, sequences) in sequences_data {
                        self.metadata_cache.set_sequences(schema_key, sequences);
                    }
                    for (schema_key, triggers) in triggers_data {
                        self.metadata_cache.set_triggers(schema_key, triggers);
                    }
                    for (schema_key, functions) in functions_data {
                        self.metadata_cache.set_functions(schema_key, functions);
                    }

                    // Update last_connected and persist
                    if let Some(saved) = self.connections.iter_mut().find(|c| c.id == id) {
                        saved.last_connected = Some(chrono::Utc::now());
                        self.persist_connections();
                    }
                    self.current_page = Page::Explorer;
                    self.focus = Focus::Sidebar;
                    self.editing_query = false;
                    self.set_status(format!("Connected to \"{}\"", conn_name));
                    if !warnings.is_empty() {
                        let count = warnings.len();
                        tracing::warn!(
                            "{} metadata warning(s) during connect: {:?}",
                            count,
                            warnings
                        );
                        self.set_status(format!("Connected with {} metadata warning(s).", count));
                    }
                }
                BackgroundEvent::ConnectFailed { conn_name, error } => {
                    if let Some(id) = self
                        .connections
                        .iter()
                        .find(|c| c.name == conn_name)
                        .map(|c| c.id)
                    {
                        self.disconnecting.remove(&id);
                    }
                    self.set_status(format!("Failed to connect to \"{}\": {}", conn_name, error));
                }
                BackgroundEvent::QueryDone {
                    provider_id,
                    provider,
                    result,
                    ..
                } => {
                    self.finish_query(provider_id, provider);

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
                            if self.result_filter_active || !self.result_filter_input.is_empty() {
                                self.apply_result_filter();
                            }
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
                                    width = col_widths.get(i).copied().unwrap_or(0)
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
                                        width = col_widths.get(i).copied().unwrap_or(0)
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
                BackgroundEvent::QueryFailed {
                    provider_id,
                    provider,
                    error,
                    ..
                } => {
                    self.finish_query(provider_id, provider);
                    self.query_results = format!("Error: {}", error);
                    self.query_columns.clear();
                    self.query_data.clear();
                    self.query_data_typed.clear();
                    self.query_column_info.clear();
                    self.query_col_widths.clear();
                    self.mongo_raw_documents = None;
                    self.cell_edit_mode = CellEditMode::Inactive;
                    self.set_status(format!("Query failed: {}", error));
                }
                BackgroundEvent::QueryAborted {
                    provider_id,
                    provider,
                    reason,
                } => {
                    self.finish_query(provider_id, provider);
                    self.cell_edit_mode = CellEditMode::Inactive;
                    self.query_results = format!("Query aborted: {}", reason);
                    if reason == "cancelled" {
                        self.set_status("Query cancelled.");
                    } else {
                        self.set_status(format!("Query aborted: {}", reason));
                    }
                }
            }
        }
    }

    /// Returns a completed query's provider to the pool — unless the user asked
    /// to disconnect while the query was running, in which case the connection
    /// is dropped instead of being re-activated.
    fn finish_query(&mut self, provider_id: Uuid, provider: Box<dyn DatabaseProvider>) {
        self.query_cancel_tx = None;
        if self.disconnecting.remove(&provider_id) {
            drop(provider);
            self.pool.unregister(provider_id);
            self.active_db_type_cache = None;
            self.explorer_tree = TreeState::new(Vec::new());
        } else {
            self.pool.give_back(provider_id, provider);
        }
    }

    /// Clears UI state tied to an active connection that has been disconnected.
    fn after_disconnect(&mut self, id: Uuid) {
        self.disconnecting.remove(&id);
        self.pool.unregister(id);
        self.query_cancel_tx = None;
        self.active_db_type_cache = None;
        self.explorer_tree = TreeState::new(Vec::new());
        self.query_text.clear();
        self.query_editor = TextArea::default();
    }

    /// Persists the connection list off the async event loop.
    fn persist_connections(&self) {
        let conns = self.connections.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = storage::connections::save_connections(&conns) {
                tracing::warn!("Failed to save connections: {}", e);
            }
        });
    }

    #[allow(dead_code)]
    async fn connect_to(&mut self, conn: ConnectionConfig) {
        let mut provider: Box<dyn DatabaseProvider> = match conn.db_type {
            DatabaseType::Postgres => Box::new(PostgresProvider::new()),
            DatabaseType::SQLite => Box::new(SqliteProvider::new()),
            DatabaseType::MySQL => Box::new(MysqlProvider::new()),
            DatabaseType::MariaDB => Box::new(MariadbProvider::new()),
            DatabaseType::MongoDB => Box::new(MongoProvider::new()),
            DatabaseType::Redis => Box::new(RedisProvider::new()),
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
                self.query_editor = TextArea::default();

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
                            if let Ok(sequences) = provider.get_sequences(schema).await {
                                self.metadata_cache
                                    .set_sequences(schema_key.clone(), sequences);
                            }
                            if let Ok(triggers) = provider.get_triggers(schema).await {
                                self.metadata_cache
                                    .set_triggers(schema_key.clone(), triggers);
                            }
                            if let Ok(functions) = provider.get_functions(schema).await {
                                self.metadata_cache
                                    .set_functions(schema_key.clone(), functions);
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
                    self.persist_connections();
                }
                self.current_page = Page::Explorer;
                self.focus = Focus::Sidebar;
                self.editing_query = false;
                self.set_status(format!("Connected to \"{}\"", conn.name));
            }
            Err(e) => {
                self.set_status(format!("Failed to connect: {}", e));
            }
        }
    }

    async fn load_columns(&mut self, table: &str) {
        let active_id = match self.pool.active_id {
            Some(id) => id,
            None => {
                self.set_status("No active connection.");
                return;
            }
        };
        let provider = match self.pool.take(active_id) {
            Some(p) => p,
            None => {
                self.set_status("Provider not available.");
                return;
            }
        };
        match provider.get_columns(table).await {
            Ok(columns) => {
                let tree_id = format!("table:{}", table);
                let child_items: Vec<crate::widgets::tree::TreeItem> = columns
                    .iter()
                    .map(|c| {
                        crate::widgets::tree::TreeItem::leaf(
                            &format!("column:{}.{}", table, c.name),
                            &c.name,
                            4,
                        )
                    })
                    .collect();
                let item_count = child_items.len();
                self.metadata_cache.set_columns(table.to_string(), columns);
                self.explorer_tree.set_children(&tree_id, child_items);
                self.set_status(format!("Loaded {} items for \"{}\"", item_count, table));
            }
            Err(e) => {
                self.set_status(format!("Failed to load items: {}", e));
            }
        }
        self.pool.give_back(active_id, provider);
    }

    async fn cache_columns(&mut self, table: &str) {
        let active_id = match self.pool.active_id {
            Some(id) => id,
            None => return,
        };
        let provider = match self.pool.take(active_id) {
            Some(p) => p,
            None => return,
        };
        match provider.get_columns(table).await {
            Ok(columns) => {
                self.metadata_cache.set_columns(table.to_string(), columns);
            }
            Err(e) => {
                self.set_status(format!("Failed to load items: {}", e));
            }
        }
        self.pool.give_back(active_id, provider);
    }

    #[allow(dead_code)]
    async fn run_query(&mut self, sql: &str) {
        if self.pool.active_id.is_none() {
            self.set_status("No active connection");
            return;
        }
        match self.pool.get_active() {
            Some(provider) => match provider.execute_query(sql).await {
                Ok(result) => {
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
                            if self.result_filter_active || !self.result_filter_input.is_empty() {
                                self.apply_result_filter();
                            }
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
                                    width = col_widths.get(i).copied().unwrap_or(0)
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
                                        width = col_widths.get(i).copied().unwrap_or(0)
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
            KeyCode::Tab if self.cell_edit_mode == CellEditMode::Inactive => {
                self.cycle_focus();
                return;
            }
            KeyCode::BackTab if self.cell_edit_mode == CellEditMode::Inactive => {
                self.cycle_focus_reverse();
                return;
            }
            _ => {}
        }

        if self.input_mode == InputMode::Insert {
            // If completion popup is visible, handle navigation keys first
            if self.completion_state.visible {
                match key.code {
                    KeyCode::Up => {
                        self.completion_state.select_prev();
                        return;
                    }
                    KeyCode::Down => {
                        self.completion_state.select_next();
                        return;
                    }
                    KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                        self.completion_state.select_prev();
                        return;
                    }
                    KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                        self.completion_state.select_next();
                        return;
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        self.accept_completion();
                        return;
                    }
                    KeyCode::Esc => {
                        self.completion_state.cancel();
                        return;
                    }
                    _ => {}
                }
            }

            match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.pending_j = false;
                    self.completion_state.cancel();
                    self.sync_query_text();
                    self.apply_cursor_style();
                }
                KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                    self.input_mode = InputMode::Normal;
                    self.pending_j = false;
                    self.completion_state.cancel();
                    self.sync_query_text();
                    self.apply_cursor_style();
                    if !self.query_text.is_empty() {
                        let sql = self.query_text.clone();
                        self.pending_action = PendingAction::RunQuery(sql);
                    }
                }
                KeyCode::Char(' ') if key.modifiers == KeyModifiers::CONTROL => {
                    self.show_completion_popup();
                }
                KeyCode::Char('j') => {
                    if self.pending_j {
                        self.query_editor.delete_char();
                        self.pending_j = false;
                        self.input_mode = InputMode::Normal;
                        self.completion_state.cancel();
                        self.sync_query_text();
                        self.apply_cursor_style();
                    } else {
                        self.query_editor.insert_char('j');
                        self.pending_j = true;
                    }
                }
                KeyCode::Char(c) => {
                    self.pending_j = false;
                    self.query_editor.insert_char(c);
                    self.sync_query_text();
                    self.update_completion();
                }
                KeyCode::Backspace => {
                    self.pending_j = false;
                    self.query_editor.delete_char();
                    self.sync_query_text();
                    self.update_completion();
                }
                KeyCode::Delete => {
                    self.pending_j = false;
                    self.query_editor.delete_next_char();
                    self.sync_query_text();
                    self.update_completion();
                }
                KeyCode::Tab => {
                    self.pending_j = false;
                    self.query_editor.insert_tab();
                    self.sync_query_text();
                    self.update_completion();
                }
                KeyCode::Enter => {
                    self.pending_j = false;
                    self.query_editor.insert_newline();
                    self.sync_query_text();
                }
                KeyCode::Left => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::Back);
                }
                KeyCode::Right => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::Forward);
                }
                KeyCode::Up => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::Up);
                }
                KeyCode::Down => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::Down);
                }
                KeyCode::Home => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::Head);
                }
                KeyCode::End => {
                    self.pending_j = false;
                    self.query_editor
                        .move_cursor(ratatui_textarea::CursorMove::End);
                }
                _ => {
                    if self.pending_j {
                        self.pending_j = false;
                    }
                }
            }
            return;
        }

        // NORMAL mode
        if self.pending_d {
            self.pending_d = false;
            match key.code {
                KeyCode::Char('w') => {
                    self.query_editor.delete_next_word();
                    self.sync_query_text();
                    return;
                }
                KeyCode::Char('b') => {
                    self.query_editor.delete_word();
                    self.sync_query_text();
                    return;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('i') => {
                self.input_mode = InputMode::Insert;
                self.pending_j = false;
                self.apply_cursor_style();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::Back);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::Forward);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::Down);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::Up);
            }
            KeyCode::Char('H') => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::Head);
            }
            KeyCode::Char('L') => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::End);
            }
            KeyCode::Char('w') => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::WordForward);
            }
            KeyCode::Char('b') => {
                self.query_editor
                    .move_cursor(ratatui_textarea::CursorMove::WordBack);
            }
            KeyCode::Char('d') => {
                self.pending_d = true;
            }
            KeyCode::Char('u') => {
                self.query_editor.undo();
                self.sync_query_text();
            }
            KeyCode::Char('R') => {
                self.query_editor.redo();
                self.sync_query_text();
            }
            KeyCode::Char('Y') => {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let full_text = self.query_editor.lines().join("\n");
                    let _ = cb.set_text(&full_text);
                    self.set_status("Query copied to clipboard");
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                self.query_editor.delete_next_char();
                self.sync_query_text();
            }
            KeyCode::Backspace => {
                self.query_editor.delete_char();
                self.sync_query_text();
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                self.sync_query_text();
                if !self.query_text.is_empty() {
                    let sql = self.query_text.clone();
                    self.pending_action = PendingAction::RunQuery(sql);
                }
            }
            KeyCode::Char(' ') if key.modifiers == KeyModifiers::CONTROL => {
                self.show_completion_popup();
            }
            _ => {}
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
            KeyCode::Down | KeyCode::Char('j') => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    if let Some(next) = self.next_filtered_row(1) {
                        self.selected_row = next;
                    }
                } else if self.selected_row + 1 < self.query_data.len() {
                    self.selected_row += 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    if let Some(next) = self.next_filtered_row(-1) {
                        self.selected_row = next;
                    }
                } else if self.selected_row > 0 {
                    self.selected_row -= 1;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::PageUp => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    let cur_pos = self
                        .result_filtered_indices
                        .iter()
                        .position(|&i| i == self.selected_row)
                        .unwrap_or(0);
                    let new_pos = cur_pos.saturating_sub(data_height);
                    self.selected_row = self.result_filtered_indices[new_pos];
                } else {
                    self.selected_row = self.selected_row.saturating_sub(data_height);
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::PageDown => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    let cur_pos = self
                        .result_filtered_indices
                        .iter()
                        .position(|&i| i == self.selected_row)
                        .unwrap_or(0);
                    let new_pos = (cur_pos + data_height)
                        .min(self.result_filtered_indices.len().saturating_sub(1));
                    self.selected_row = self.result_filtered_indices[new_pos];
                } else {
                    self.selected_row = self
                        .selected_row
                        .saturating_add(data_height)
                        .min(self.query_data.len().saturating_sub(1));
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::Home => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    self.selected_row = self.result_filtered_indices[0];
                } else {
                    self.selected_row = 0;
                }
                self.ensure_cell_visible(data_height);
                self.rebuild_json_tree();
                true
            }
            KeyCode::End => {
                self.inspector_edit_col = None;
                if self.has_result_filter() {
                    self.selected_row = self.result_filtered_indices.last().copied().unwrap_or(0);
                } else {
                    self.selected_row = self.query_data.len().saturating_sub(1);
                }
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
            KeyCode::Char('e') if key.modifiers == KeyModifiers::ALT => {
                if self.query_data.is_empty() {
                    self.set_status("No results to export.");
                } else {
                    self.export_dialog = Some(ExportDialog::new());
                    self.update_key_hints();
                }
                true
            }
            _ => false,
        }
    }

    fn handle_result_filter_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
                self.result_filter_active = false;
                self.result_filter_input.clear();
                self.result_filtered_indices.clear();
                self.result_filter_highlights.clear();
                self.set_status("Result filter: closed");
                true
            }
            KeyCode::Esc => {
                self.result_filter_active = false;
                self.result_filter_input.clear();
                self.result_filtered_indices.clear();
                self.result_filter_highlights.clear();
                true
            }
            KeyCode::Enter => {
                self.result_filter_active = false;
                self.set_status(if self.result_filtered_indices.is_empty() {
                    "Result filter: accepted (no matches)".to_string()
                } else {
                    format!(
                        "Result filter: {} / {} results",
                        self.result_filtered_indices.len(),
                        self.query_data.len()
                    )
                });
                true
            }
            KeyCode::Backspace => {
                if !self.result_filter_input.is_empty() && self.result_filter_cursor > 0 {
                    let pos = self.result_filter_cursor.saturating_sub(1);
                    self.result_filter_input.remove(pos);
                    self.result_filter_cursor = pos;
                    self.apply_result_filter();
                }
                true
            }
            KeyCode::Left => {
                if self.result_filter_cursor > 0 {
                    self.result_filter_cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.result_filter_cursor < self.result_filter_input.len() {
                    self.result_filter_cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                self.result_filter_cursor = 0;
                true
            }
            KeyCode::End => {
                self.result_filter_cursor = self.result_filter_input.len();
                true
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.result_filter_input
                    .insert(self.result_filter_cursor, c);
                self.result_filter_cursor += 1;
                self.apply_result_filter();
                true
            }
            _ => false,
        }
    }

    fn handle_export_dialog_key(&mut self, key: &KeyEvent) {
        let dialog = match self.export_dialog.as_mut() {
            Some(d) => d,
            None => return,
        };
        match key.code {
            KeyCode::Esc => {
                self.export_dialog = None;
                self.update_key_hints();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if dialog.format > 0 {
                    dialog.format -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if dialog.format + 1 < crate::export::ExportFormat::all().len() {
                    dialog.format += 1;
                }
            }
            KeyCode::Enter => {
                let format = crate::export::ExportFormat::all()[dialog.format];
                let filename = dialog.filename.trim().to_string();
                if filename.is_empty() {
                    self.set_status("Filename cannot be empty.");
                    return;
                }
                let filename = if filename.contains('.') {
                    filename
                } else {
                    format!("{}.{}", filename, format.extension())
                };
                let format = crate::export::ExportFormat::all()[dialog.format];
                self.export_dialog = None;
                self.pending_action = PendingAction::ExportResults { format, filename };
                self.update_key_hints();
            }
            KeyCode::Backspace => {
                if dialog.cursor > 0 {
                    dialog.filename.remove(dialog.cursor - 1);
                    dialog.cursor -= 1;
                }
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                dialog.filename.insert(dialog.cursor, c);
                dialog.cursor += 1;
            }
            _ => {}
        }
    }

    fn handle_import_dialog_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.import_dialog = None;
                self.update_key_hints();
            }
            KeyCode::Enter => {
                let path = self
                    .import_dialog
                    .as_ref()
                    .map(|d| d.path.clone())
                    .unwrap_or_default();
                let path_trimmed = path.trim().to_string();
                if path_trimmed.is_empty() {
                    self.set_status("File path cannot be empty.");
                    return;
                }
                let format = match crate::import::ImportFormat::from_extension(&path_trimmed) {
                    Some(f) => f,
                    None => {
                        self.set_status("Unsupported file format. Use .csv, .json, or .sql");
                        return;
                    }
                };
                let table = self
                    .connections
                    .iter()
                    .find(|c| Some(c.id) == self.pool.active_id)
                    .map(|c| c.database.as_str())
                    .unwrap_or("table")
                    .to_string();
                match crate::import::import_data(format, &path_trimmed, &table) {
                    Ok(preview) => {
                        let stmt_count = preview.sql_statements.len();
                        let path_display = path_trimmed.clone();
                        if let Some(ref mut d) = self.import_dialog {
                            d.preview = Some(preview);
                        }
                        self.set_status(format!(
                            "Loaded {} statements from {}. Press Enter to confirm or Esc to cancel.",
                            stmt_count, path_display
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("Import error: {}", e));
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut d) = self.import_dialog {
                    if d.cursor > 0 {
                        d.path.remove(d.cursor - 1);
                        d.cursor -= 1;
                    }
                }
            }
            KeyCode::Left => {
                if let Some(ref mut d) = self.import_dialog {
                    if d.cursor > 0 {
                        d.cursor -= 1;
                    }
                }
            }
            KeyCode::Right => {
                if let Some(ref mut d) = self.import_dialog {
                    if d.cursor < d.path.len() {
                        d.cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Some(ref mut d) = self.import_dialog {
                    d.cursor = 0;
                }
            }
            KeyCode::End => {
                if let Some(ref mut d) = self.import_dialog {
                    d.cursor = d.path.len();
                }
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                if let Some(ref mut d) = self.import_dialog {
                    d.path.insert(d.cursor, c);
                    d.cursor += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_import_preview_key(&mut self, key: &KeyEvent) -> bool {
        let dialog = match self.import_dialog.as_mut() {
            Some(d) => d,
            None => return false,
        };
        match key.code {
            KeyCode::Esc => {
                self.import_dialog = None;
                self.update_key_hints();
                true
            }
            KeyCode::Enter => {
                if let Some(ref preview) = dialog.preview.clone() {
                    dialog.confirmed = true;
                    self.import_dialog = None;
                    let sql = preview.sql_statements.join("\n");
                    self.pending_action = PendingAction::RunQuery(sql);
                    self.set_status(format!(
                        "Executing {} SQL statements...",
                        preview.sql_statements.len()
                    ));
                }
                self.update_key_hints();
                true
            }
            _ => false,
        }
    }

    fn apply_result_filter(&mut self) {
        if self.result_filter_input.is_empty() {
            self.result_filtered_indices = (0..self.query_data.len()).collect();
            self.result_filter_highlights.clear();
            return;
        }
        self.result_filtered_indices = crate::search::result_filter::filter_results(
            &self.result_filter_input,
            &self.query_columns,
            &self.query_data,
        );
        self.result_filter_highlights = crate::search::result_filter::compute_highlights(
            &self.result_filter_input,
            &self.query_data,
            &self.result_filtered_indices,
        );
        if !self.result_filtered_indices.is_empty()
            && !self.result_filtered_indices.contains(&self.selected_row)
        {
            self.selected_row = self.result_filtered_indices[0];
        }
    }

    fn next_filtered_row(&self, direction: i32) -> Option<usize> {
        if self.result_filtered_indices.is_empty() {
            return None;
        }
        let current_pos = self
            .result_filtered_indices
            .iter()
            .position(|&i| i == self.selected_row)?;
        let new_pos = (current_pos as i32 + direction)
            .clamp(0, self.result_filtered_indices.len() as i32 - 1) as usize;
        Some(self.result_filtered_indices[new_pos])
    }

    fn has_result_filter(&self) -> bool {
        !self.result_filter_input.is_empty()
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
        self.set_status("Value updated".to_string());
    }

    fn mongo_mouse_to_line(&self, mouse_row: u16) -> Option<usize> {
        let combined_h = self.main_rect.height;
        let results_area_height = combined_h * 75 / 100;
        let results_area_y = self.main_rect.y + combined_h * 25 / 100;
        let inner_y = results_area_y + 1; // top border
        let inner_height = results_area_height.saturating_sub(2) as usize; // minus borders
        if mouse_row < inner_y {
            return None;
        }
        let relative_row = (mouse_row - inner_y) as usize;
        if relative_row >= inner_height {
            return None;
        }
        let docs = self.mongo_raw_documents.as_ref()?;
        let total = self.mongo_doc_state.cached_line_count(docs);
        if total == 0 {
            return None;
        }

        // Replicate renderer's view_offset logic so mouse hits the correct line
        let visible = inner_height.min(total);
        let mut effective_offset = self
            .mongo_doc_state
            .scroll_offset
            .min(total.saturating_sub(visible));
        let sel = self
            .mongo_doc_state
            .selected_line
            .min(total.saturating_sub(1));
        if sel < effective_offset {
            effective_offset = sel;
        } else if sel >= effective_offset + visible && sel >= visible {
            effective_offset = sel.saturating_add(1).saturating_sub(visible);
        }
        effective_offset = effective_offset.min(total.saturating_sub(visible));

        let line = effective_offset + relative_row;
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
                    " Esc:Cancel | Enter:Save | Alt+Z:Undo | Alt+H:Inspector | Alt+I:Editor"
                        .to_string();
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
                    " j/k:Scroll | g/G:Top/Bot | Click:Edit | Alt+Z:Undo | Alt+H:Inspector | Alt+I:Editor"
                        .to_string();
            }
        } else if self.export_dialog.is_some() {
            self.key_hints = " ←→:Format | Enter:Export | Esc:Cancel | Type:filename ".to_string();
        } else if let Some(ref dialog) = self.import_dialog {
            if dialog.preview.is_some() {
                self.key_hints = " Enter:Confirm Import | Esc:Cancel ".to_string();
            } else {
                self.key_hints = " Enter:Load File | Esc:Cancel | Type:path ".to_string();
            }
        } else if self.focus == Focus::Main
            && self.current_page == Page::QueryEditor
            && self.query_editor_focus == QueryEditorFocus::Results
        {
            self.key_hints =
                " Ctrl+Q:Quit | j/k:Nav | Alt+H:Inspector | Alt+I:Editor | Alt+F:Filter | Alt+E:Export"
                    .to_string();
        } else if self.current_page == Page::SchemaBrowser {
            if matches!(self.schema_browser.mode, SchemaBrowserMode::Browse) {
                self.key_hints =
                    " j/k:Categories | h/l:Items | g/G:Top/Bot | n:Create | d:Drop | r:Rename | a:Alter | i:Import | I:ImportFile | E:Export | s:Diff | Tab:Focus "
                        .to_string();
            } else {
                self.key_hints = " Esc:Cancel | Enter:Confirm ".to_string();
            }
        } else {
            self.key_hints =
                " Ctrl+Q:Quit | 1-8:Pages | ?:Help | Tab:Focus | Alt-H:Inspector | Alt-I:Editor"
                    .to_string();
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
        if self.has_result_filter() {
            if let Some(pos) = self
                .result_filtered_indices
                .iter()
                .position(|&i| i == self.selected_row)
            {
                if pos >= self.result_scroll_v + data_height {
                    self.result_scroll_v = pos.saturating_sub(data_height) + 1;
                } else if pos < self.result_scroll_v {
                    self.result_scroll_v = pos;
                }
            }
        } else if self.selected_row >= self.result_scroll_v + data_height {
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
        let results_area_y = if self.show_query_editor {
            self.main_rect.y + self.main_rect.height * 25 / 100
        } else {
            self.main_rect.y
        };
        let inner_x = self.main_rect.x + 1;
        let inner_y = results_area_y + 1;

        let filter_bar = if self.result_filter_active { 2 } else { 0 };
        let header_rows = 2u16;
        let data_y = inner_y + filter_bar + header_rows;

        if mouse_row < data_y {
            return None;
        }

        let row_offset = (mouse_row - data_y) as usize;
        let data_row = if self.has_result_filter() && !self.result_filtered_indices.is_empty() {
            let visible_pos = self.result_scroll_v + row_offset;
            if visible_pos >= self.result_filtered_indices.len() {
                return None;
            }
            self.result_filtered_indices[visible_pos]
        } else {
            let r = self.result_scroll_v + row_offset;
            if r >= self.query_data.len() {
                return None;
            }
            r
        };

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

        if key.code == KeyCode::Char('q') && key.modifiers == KeyModifiers::CONTROL {
            self.should_quit = true;
            return;
        }

        // Cancel an in-flight query with Esc or Ctrl+C while busy.
        if self.is_busy {
            let wants_cancel = key.code == KeyCode::Esc
                || (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL);
            if wants_cancel {
                self.pending_action = PendingAction::CancelQuery;
                return;
            }
        }

        if self.export_dialog.is_some() {
            self.handle_export_dialog_key(&key);
            return;
        }

        if let Some(ref mut dialog) = self.import_dialog {
            if dialog.preview.is_some() && !dialog.confirmed {
                // Preview shown, waiting for confirm
                if self.handle_import_preview_key(&key) {
                    return;
                }
            } else {
                self.handle_import_dialog_key(&key);
                return;
            }
        }

        if self.result_filter_active
            && self.current_page == Page::QueryEditor
            && self.focus == Focus::Main
            && self.query_editor_focus == QueryEditorFocus::Results
        {
            if self.handle_result_filter_key(&key) {
                return;
            }
        }

        if self.editing_query
            && self.current_page == Page::QueryEditor
            && self.query_editor_focus == QueryEditorFocus::Editor
        {
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

        if self.current_page == Page::SchemaBrowser
            && crate::ui::pages::schema_browser::handle_key(self, &key)
        {
            return;
        }

        if self.current_page == Page::Logs && crate::ui::pages::logs::handle_key(self, &key) {
            return;
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
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
                self.apply_cursor_style();
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor && !self.query_text.is_empty() {
                    let sql = self.query_text.clone();
                    self.pending_action = PendingAction::RunQuery(sql);
                }
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                if self.current_page == Page::QueryEditor {
                    self.enter_query_editor_normal_mode();
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
            KeyCode::Char('i') if key.modifiers == KeyModifiers::ALT => {
                self.show_query_editor = !self.show_query_editor;
                if !self.show_query_editor && self.query_editor_focus == QueryEditorFocus::Editor {
                    self.query_editor_focus = QueryEditorFocus::Results;
                }
                self.update_key_hints();
                self.set_status(if self.show_query_editor {
                    "Query Editor: shown"
                } else {
                    "Query Editor: hidden"
                });
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
                if self.current_page == Page::QueryEditor
                    && self.focus == Focus::Main
                    && self.query_editor_focus == QueryEditorFocus::Results
                {
                    self.result_filter_active = !self.result_filter_active;
                    if self.result_filter_active {
                        if self.result_filter_input.is_empty() {
                            self.result_filtered_indices = (0..self.query_data.len()).collect();
                        }
                        self.set_status(
                            "Result filter — type to filter, Esc to clear, Enter to accept",
                        );
                    } else {
                        self.result_filter_input.clear();
                        self.result_filtered_indices.clear();
                        self.result_filter_highlights.clear();
                        self.set_status("Result filter: closed");
                    }
                }
            }
            _ => {}
        }
        self.update_key_hints();
    }

    fn auto_load_selected_table_columns(&mut self) {
        if let Some(id) = self.explorer_tree.selected_id() {
            if let Some(name) = id.strip_prefix("table:") {
                if !id.ends_with(':') {
                    let table_name = name.to_string();
                    if self.metadata_cache.get_columns(&table_name).is_none() {
                        self.pending_action = PendingAction::CacheColumns { table: table_name };
                    }
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
                        if let Some(name) = id.strip_prefix("table:") {
                            if !id.ends_with(':') {
                                let table_name = name.to_string();
                                self.pending_action =
                                    PendingAction::LoadColumns { table: table_name };
                                self.explorer_tree.toggle_selected();
                            }
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
                    if let Some(name) = id.strip_prefix("table:") {
                        if !id.ends_with(':') {
                            let is_redis = self.active_db_type() == Some(DatabaseType::Redis);
                            if is_redis {
                                let parts: Vec<&str> = name.splitn(2, '.').collect();
                                if parts.len() == 2 {
                                    let db_prefix = parts[0];
                                    let key_type = parts[1];
                                    let db_num = db_prefix.trim_start_matches("db");
                                    let db_num = if db_num.is_empty() { "0" } else { db_num };
                                    let base_cmd = match key_type {
                                        "strings" => "SCAN 0 TYPE string COUNT 100".to_string(),
                                        "hashes" => "SCAN 0 TYPE hash COUNT 100".to_string(),
                                        "lists" => "SCAN 0 TYPE list COUNT 100".to_string(),
                                        "sets" => "SCAN 0 TYPE set COUNT 100".to_string(),
                                        "sorted_sets" => "SCAN 0 TYPE zset COUNT 100".to_string(),
                                        "streams" => "SCAN 0 TYPE stream COUNT 100".to_string(),
                                        _ => "SCAN 0 COUNT 100".to_string(),
                                    };
                                    let cmd = format!("SELECT {}\n{}", db_num, base_cmd);
                                    self.pending_action = PendingAction::OpenQuery(cmd);
                                }
                            } else {
                                let (db_name, short) = match name.find('.') {
                                    Some(pos) => (&name[..pos], &name[pos + 1..]),
                                    None => ("", name),
                                };
                                let query = if self.active_db_type() == Some(DatabaseType::MongoDB)
                                {
                                    if db_name.is_empty() {
                                        format!("{{ \"find\": \"{}\", \"limit\": 1000 }}\n", short)
                                    } else {
                                        format!(
                                            "{{ \"db\": \"{}\", \"find\": \"{}\", \"limit\": 1000 }}\n",
                                            db_name, short
                                        )
                                    }
                                } else {
                                    let db_type = self.active_db_type();
                                    let q = if db_type == Some(DatabaseType::MySQL)
                                        || db_type == Some(DatabaseType::MariaDB)
                                    {
                                        '`'
                                    } else {
                                        '"'
                                    };
                                    format!(
                                        "SELECT * FROM {q}{short}{q} LIMIT 1000 OFFSET 0;\n",
                                        q = q,
                                        short = short
                                    )
                                };
                                self.pending_action = PendingAction::OpenQuery(query);
                            }
                        }
                    } else if let Some(column_key) = id.strip_prefix("column:") {
                        let is_redis = self.active_db_type() == Some(DatabaseType::Redis);
                        if is_redis {
                            let parts: Vec<&str> = column_key.splitn(3, '.').collect();
                            if parts.len() == 3 {
                                let db_prefix = parts[0];
                                let key_type = parts[1];
                                let key_name = parts[2];
                                let base_cmd = match key_type {
                                    "strings" => format!("GET \"{}\"", key_name),
                                    "hashes" => format!("HGETALL \"{}\"", key_name),
                                    "lists" => format!("LRANGE \"{}\" 0 -1", key_name),
                                    "sets" => format!("SMEMBERS \"{}\"", key_name),
                                    "sorted_sets" => {
                                        format!("ZRANGE \"{}\" 0 -1 WITHSCORES", key_name)
                                    }
                                    "streams" => format!("XRANGE \"{}\" - +", key_name),
                                    _ => format!("GET \"{}\"", key_name),
                                };
                                let db_num = db_prefix.trim_start_matches("db");
                                let db_num = if db_num.is_empty() { "0" } else { db_num };
                                let cmd = format!("SELECT {}\n{}", db_num, base_cmd);
                                self.pending_action = PendingAction::OpenQuery(cmd);
                            } else {
                                self.current_page = Page::Explorer;
                                self.set_status("Viewing column details");
                            }
                        } else {
                            self.current_page = Page::Explorer;
                            self.set_status("Viewing column details");
                        }
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
                    let password =
                        conn.password
                            .clone()
                            .or_else(|| {
                                match storage::credentials::get_password(&conn.password_key()) {
                                    Ok(p) => p,
                                    Err(e) => {
                                        tracing::debug!(
                                            "Failed to load password from keychain: {}",
                                            e
                                        );
                                        None
                                    }
                                }
                            });
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
                    if form.form_tab == FormTab::Form {
                        form.active_field = (form.active_field + 1) % 7;
                    }
                }
                true
            }
            KeyCode::BackTab => {
                if let Some(form) = &mut self.conn_form {
                    if form.form_tab == FormTab::Form {
                        form.active_field = if form.active_field == 0 {
                            6
                        } else {
                            form.active_field - 1
                        };
                    }
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
                    if form.form_tab == FormTab::Form && form.active_field == 1 {
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
            KeyCode::Char(ch)
                if (ch == 'h' || ch == 'H') && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(form) = &mut self.conn_form {
                    if form.form_tab != FormTab::Form {
                        form.form_tab = FormTab::Form;
                        self.sync_connection_form();
                    }
                }
                true
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(form) = &mut self.conn_form {
                    if form.form_tab != FormTab::Form {
                        form.form_tab = FormTab::Form;
                        self.sync_connection_form();
                    }
                }
                true
            }
            KeyCode::Char(ch)
                if (ch == 'l' || ch == 'L') && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(form) = &mut self.conn_form {
                    if form.form_tab != FormTab::Uri {
                        form.form_tab = FormTab::Uri;
                        self.sync_connection_form();
                    }
                }
                true
            }
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return true;
                }
                if let Some(form) = &mut self.conn_form {
                    if form.form_tab == FormTab::Uri {
                        let mut field = crate::widgets::input::InputField::with_value(
                            "",
                            form.uri_input.clone(),
                        );
                        field.handle_key(key);
                        form.uri_input = field.value;
                        return true;
                    }
                    if form.active_field != 1 {
                        let val = form.field_value(form.active_field).to_string();
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
                        tokio::task::spawn_blocking(move || {
                            let _ = storage::credentials::delete_password(&pwd_key);
                        });
                        self.connections.remove(idx);
                        self.persist_connections();
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
                                if let Some(name) = id.strip_prefix("table:") {
                                    if !id.ends_with(':') {
                                        let is_redis =
                                            self.active_db_type() == Some(DatabaseType::Redis);
                                        if is_redis {
                                            self.pending_action = PendingAction::LoadColumns {
                                                table: name.to_string(),
                                            };
                                            self.explorer_tree.toggle_selected();
                                        } else {
                                            let (db_name, short) = match name.find('.') {
                                                Some(pos) => (&name[..pos], &name[pos + 1..]),
                                                None => ("", name),
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
                                                let db_type = self.active_db_type();
                                                let q = if db_type == Some(DatabaseType::MySQL)
                                                    || db_type == Some(DatabaseType::MariaDB)
                                                {
                                                    '`'
                                                } else {
                                                    '"'
                                                };
                                                format!(
                                                    "SELECT * FROM {q}{short}{q} LIMIT 1000 OFFSET 0;\n",
                                                    q = q,
                                                    short = short
                                                )
                                            };
                                            self.pending_action = PendingAction::OpenQuery(query);
                                        }
                                    } else {
                                        self.explorer_tree.toggle_selected();
                                    }
                                }
                            }
                        }
                    }
                } else if self.current_page == Page::QueryEditor {
                    let is_double = self.is_double_click(col, row);
                    let editor_bottom = if self.show_query_editor {
                        self.main_rect.y + self.main_rect.height * 25 / 100
                    } else {
                        self.main_rect.y
                    };
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
                        let editor_x = self.main_rect.x + 1;
                        let editor_y = self.main_rect.y + 1;
                        let editor_w = self.main_rect.width.saturating_sub(2);
                        let editor_h = editor_bottom
                            .saturating_sub(self.main_rect.y)
                            .saturating_sub(2);
                        let editor_inner = Rect::new(editor_x, editor_y, editor_w, editor_h);
                        if is_double {
                            self.editing_query = true;
                            let lines: Vec<String> =
                                self.query_text.lines().map(String::from).collect();
                            self.query_editor = TextArea::from(lines);
                            let cursor_row = row.saturating_sub(editor_inner.y);
                            let cursor_col = col.saturating_sub(editor_inner.x);
                            self.query_editor
                                .move_cursor(ratatui_textarea::CursorMove::Jump(
                                    cursor_row, cursor_col,
                                ));
                            self.input_mode = InputMode::Insert;
                            self.pending_j = false;
                            self.pending_d = false;
                        } else {
                            if self.editing_query {
                                let cursor_row = row.saturating_sub(editor_inner.y);
                                let cursor_col = col.saturating_sub(editor_inner.x);
                                self.query_editor
                                    .move_cursor(ratatui_textarea::CursorMove::Jump(
                                        cursor_row, cursor_col,
                                    ));
                            } else {
                                self.enter_query_editor_normal_mode();
                                let cursor_row = row.saturating_sub(editor_inner.y);
                                let cursor_col = col.saturating_sub(editor_inner.x);
                                self.query_editor
                                    .move_cursor(ratatui_textarea::CursorMove::Jump(
                                        cursor_row, cursor_col,
                                    ));
                            }
                        }
                    } else {
                        self.inspector_edit_col = None;
                        self.focus = Focus::Main;
                        self.update_key_hints();
                        self.query_editor_focus = QueryEditorFocus::Results;
                        if self.is_mongo_query() {
                            if let Some(line) = self.mongo_mouse_to_line(row) {
                                self.mongo_doc_state.selected_line = line;
                                if let Some(docs) = self.mongo_raw_documents.as_ref() {
                                    let total = self.mongo_doc_state.cached_line_count(docs);
                                    let visible_height = (self.main_rect.height * 75 / 100)
                                        .saturating_sub(2)
                                        as usize;
                                    self.mongo_doc_state.adjust_offset(visible_height, total);
                                }
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
        if let Some(form) = &mut self.conn_form {
            if form.form_tab == FormTab::Uri {
                match parse_connection_uri(&form.uri_input) {
                    Ok((db_type, host, port, database, username, password)) => {
                        if form.name.is_empty() {
                            let generated = if !username.is_empty() && !host.is_empty() {
                                format!("{}@{}", username, host)
                            } else if !host.is_empty() {
                                if host == "localhost" && !database.is_empty() {
                                    database.clone()
                                } else {
                                    host.clone()
                                }
                            } else if !database.is_empty() {
                                database.clone()
                            } else {
                                db_type.display_name().to_string()
                            };
                            form.name = generated;
                        }
                        form.db_type = db_type;
                        form.host = host;
                        form.port = port.to_string();
                        form.database = database;
                        form.username = username;
                        form.password = password;
                    }
                    Err(e) => {
                        form.error = Some(e);
                        return;
                    }
                }
            }
        }

        let config = match &self.conn_form {
            Some(f) => {
                if f.name.trim().is_empty() {
                    if let Some(f) = &mut self.conn_form {
                        f.error = Some("Name is required".to_string());
                    }
                    return;
                }
                let db_optional = matches!(f.db_type, DatabaseType::Redis | DatabaseType::MongoDB);
                if f.database.trim().is_empty() && !db_optional {
                    if let Some(f) = &mut self.conn_form {
                        f.error = Some("Database is required".to_string());
                    }
                    return;
                }
                f.to_config()
            }
            None => return,
        };
        let is_new = self.conn_form.as_ref().map_or(false, |f| f.is_new);
        let password_cleared = self
            .conn_form
            .as_ref()
            .map_or(false, |f| f.password.is_empty() && !f.is_new);

        if is_new {
            self.connections.push(config.clone());
        } else if let Some(existing) = self.connections.iter_mut().find(|c| c.id == config.id) {
            let prev_password_key = existing.password_key();
            *existing = config.clone();
            if prev_password_key != config.password_key() {
                tokio::task::spawn_blocking(move || {
                    let _ = storage::credentials::delete_password(&prev_password_key);
                });
            }
        }

        let password_key = config.password_key();
        if password_cleared {
            tokio::task::spawn_blocking(move || {
                let _ = storage::credentials::delete_password(&password_key);
            });
        } else if let Some(pw) = config.password.clone() {
            tokio::task::spawn_blocking(move || {
                if let Err(e) = storage::credentials::save_password(&password_key, &pw) {
                    tracing::debug!("Failed to save password to keychain: {}", e);
                }
            });
        }

        self.persist_connections();
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

    fn sync_connection_form(&mut self) {
        if let Some(form) = &mut self.conn_form {
            match form.form_tab {
                FormTab::Form => {
                    form.uri_input = form.build_uri();
                }
                FormTab::Uri => {
                    if let Ok((db_type, host, port, database, username, password)) =
                        parse_connection_uri(&form.uri_input)
                    {
                        if form.name.is_empty() {
                            let generated = if !username.is_empty() && !host.is_empty() {
                                format!("{}@{}", username, host)
                            } else if !host.is_empty() && host != "localhost" {
                                host.clone()
                            } else if !database.is_empty() {
                                database.clone()
                            } else {
                                String::new()
                            };
                            form.name = generated;
                        }
                        form.db_type = db_type;
                        form.host = host;
                        form.port = port.to_string();
                        form.database = database;
                        form.username = username;
                        form.password = password;
                    }
                }
            }
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
        if page == Page::QueryEditor {
            self.query_editor_focus = if self.show_query_editor {
                QueryEditorFocus::Editor
            } else {
                QueryEditorFocus::Results
            };
            self.editing_query = self.show_query_editor;
            self.input_mode = InputMode::Normal;
            self.completion_state.cancel();
            if self.show_query_editor {
                self.enter_query_editor_normal_mode();
            }
            self.apply_cursor_style();
        }
        self.update_key_hints();
        self.set_status(format!("Switched to {}", page.title()));
    }

    fn enter_query_editor_normal_mode(&mut self) {
        self.editing_query = true;
        self.input_mode = InputMode::Normal;
        self.pending_j = false;
        self.pending_d = false;
        self.completion_state.cancel();
        let lines: Vec<String> = self.query_text.lines().map(String::from).collect();
        self.query_editor = TextArea::from(lines);
        self.query_editor
            .move_cursor(ratatui_textarea::CursorMove::Bottom);
        self.query_editor
            .move_cursor(ratatui_textarea::CursorMove::End);
        self.apply_cursor_style();
    }

    fn sync_query_text(&mut self) {
        self.query_text = self.query_editor.lines().join("\n");
    }

    fn update_completion(&mut self) {
        let (cursor_row, cursor_col) = self.query_editor.cursor();
        let lines = self.query_editor.lines();
        let current_line = lines
            .get(cursor_row as usize)
            .map(|s| s.as_str())
            .unwrap_or("");
        let db_type = self.active_db_type();

        use crate::completion::mongo_provider::MongoCompletionProvider;
        use crate::completion::provider::CompletionProvider;
        use crate::completion::redis_provider::RedisCompletionProvider;
        use crate::completion::sql_provider::SqlCompletionProvider;

        let provider: Box<dyn CompletionProvider> = match db_type.as_ref() {
            Some(crate::models::connection::DatabaseType::MongoDB) => {
                Box::new(MongoCompletionProvider)
            }
            Some(crate::models::connection::DatabaseType::Redis) => {
                Box::new(RedisCompletionProvider)
            }
            _ => Box::new(SqlCompletionProvider),
        };

        self.completion_state.update(
            current_line,
            cursor_row as u16,
            cursor_col as u16,
            provider.as_ref(),
            &self.metadata_cache,
            db_type.as_ref(),
        );
    }

    fn show_completion_popup(&mut self) {
        self.update_completion();
        self.completion_state.visible = true;
    }

    fn accept_completion(&mut self) {
        let item = match self.completion_state.accept() {
            Some(item) => item,
            None => return,
        };

        let prefix_len = self.completion_state.prefix.len();
        for _ in 0..prefix_len {
            self.query_editor.delete_char();
        }

        for ch in item.label.chars() {
            self.query_editor.insert_char(ch);
        }

        self.sync_query_text();
    }

    fn apply_cursor_style(&self) {
        let style = if self.editing_query && self.input_mode == InputMode::Insert {
            SetCursorStyle::SteadyBar
        } else {
            SetCursorStyle::SteadyBlock
        };
        let _ = crossterm::execute!(std::io::stdout(), style);
    }

    fn cycle_focus(&mut self) {
        if self.current_page == Page::QueryEditor {
            match self.focus {
                Focus::Sidebar => {
                    self.focus = Focus::Main;
                    self.query_editor_focus = if self.show_query_editor {
                        QueryEditorFocus::Editor
                    } else {
                        QueryEditorFocus::Results
                    };
                    if self.show_query_editor {
                        self.enter_query_editor_normal_mode();
                    }
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
                        if self.show_query_editor {
                            self.query_editor_focus = QueryEditorFocus::Editor;
                            self.enter_query_editor_normal_mode();
                        } else {
                            self.focus = Focus::Sidebar;
                        }
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
