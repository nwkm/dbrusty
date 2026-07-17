use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::database::metadata::MetadataCache;
use crate::database::pool::ConnectionPool;
use crate::database::provider::DatabaseProvider;
use crate::models::connection::{ConnectionConfig, DatabaseType};
use crate::providers::postgres::PostgresProvider;
use crate::providers::sqlite::SqliteProvider;
use crate::storage;
use crate::ui;
use crate::widgets::input::InputField;
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
            name: if has_env { std::env::var("DATABASE_NAME").unwrap_or_default() } else { String::new() },
            db_type: if has_env { DatabaseType::Postgres } else { DatabaseType::SQLite },
            host: env_host.unwrap_or_else(|| "localhost".to_string()),
            port: std::env::var("DATABASE_PORT").unwrap_or_default(),
            database: if has_env { std::env::var("DATABASE_NAME").unwrap_or_default() } else { String::new() },
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
    pub query_editor: InputField,
    pub editing_query: bool,
}

impl App {
    pub fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.log_entries.push(msg.clone());
        if self.log_entries.len() > 500 {
            self.log_entries.remove(0);
        }
        self.status_message = msg;
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
            key_hints: String::from(" q:Quit | 1-8:Pages | ?:Help | Tab:Focus"),
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
            query_editor: InputField::new("Enter SQL query..."),
            editing_query: false,
        };
        app.set_status("Ready");
        app
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::EnableMouseCapture
        )?;
        let mut terminal = ratatui::init();
        let result = self.run_loop(&mut terminal).await;
        ratatui::restore();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture
        )?;
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
                self.set_status("Switched to Query Editor");
            }
            PendingAction::RunQuery(sql) => {
                self.run_query(&sql).await;
            }
            PendingAction::None => {}
        }
    }

    async fn connect_to(&mut self, conn: ConnectionConfig) {
        let mut provider: Box<dyn DatabaseProvider> = match conn.db_type {
            DatabaseType::Postgres => Box::new(PostgresProvider::new()),
            DatabaseType::SQLite => Box::new(SqliteProvider::new()),
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

                if let Some(provider) = self.pool.get_active() {
                    let databases = provider.get_databases().await.unwrap_or_default();

                    let mut tables_data = Vec::new();
                    let mut views_data = Vec::new();
                    let mut schemas_data: Vec<(String, Vec<String>)> = Vec::new();

                    for db in &databases {
                        let schemas = provider.get_schemas().await.unwrap_or_default();
                        schemas_data.push((db.clone(), schemas.clone()));

                        for schema in &schemas {
                            let schema_key = format!("{}.{}", db, schema);

                            if let Ok(tables) = provider.get_tables(schema).await {
                                self.metadata_cache.set_tables(schema_key.clone(), tables.clone());
                                tables_data.push((schema_key.clone(), tables));
                            }
                            if let Ok(views) = provider.get_views(schema).await {
                                self.metadata_cache.set_views(schema_key.clone(), views.clone());
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
                    let id = format!("table:{}", table);
                    let column_items: Vec<crate::widgets::tree::TreeItem> = columns
                        .iter()
                        .map(|col| {
                            let pk = if col.is_pk { " PK" } else { "" };
                            let label = format!("{} ({}){}", col.name, col.type_name, pk);
                            crate::widgets::tree::TreeItem::leaf(
                                &format!("column:{}.{}", table, col.name),
                                &label,
                                4,
                            )
                        })
                        .collect();

                    self.metadata_cache.set_columns(table.to_string(), columns);
                    self.explorer_tree.set_children(&id, column_items);
                    self.explorer_tree.toggle_by_id(&id);
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
                    let mut output = String::new();

                    // Duration at the top
                    output.push_str(&format!("({:?})", result.duration));
                    output.push('\n');

                    // Column headers
                    if !result.columns.is_empty() {
                        for (i, col) in result.columns.iter().enumerate() {
                            if i > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&col.name);
                        }
                        output.push('\n');
                        // Separator
                        for (i, col) in result.columns.iter().enumerate() {
                            if i > 0 {
                                output.push_str("-+-");
                            }
                            for _ in 0..col.name.len() {
                                output.push('-');
                            }
                        }
                        output.push('\n');
                    }

                    // Rows
                    for row in &result.rows {
                        for (i, val) in row.iter().enumerate() {
                            if i > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&val.to_string());
                        }
                        output.push('\n');
                    }

                    if !result.is_select {
                        output.push_str(&format!(
                            "{} rows affected",
                            result.rows_affected
                        ));
                        output.push('\n');
                    }
                    self.query_results = output;
                    self.set_status(format!(
                        "Query returned {} rows in {:?}",
                        result.rows.len(),
                        result.duration
                    ));
                }
                Err(e) => {
                    self.query_results = format!("Error: {}", e);
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

    fn handle_key(&mut self, key: KeyEvent) {
        self.status_message.clear();

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
            _ => {}
        }
    }

    fn handle_sidebar_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.explorer_tree.select_down();
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.explorer_tree.select_up();
                true
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
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
                } else if self.main_rect.contains((col, row).into())
                    && self.current_page == Page::Connections
                {
                    if self.conn_list_selected + 1 < self.connections.len() {
                        self.conn_list_selected += 1;
                        self.adjust_conn_scroll();
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if self.sidebar_rect.contains((col, row).into()) {
                    self.explorer_tree.select_up();
                } else if self.main_rect.contains((col, row).into())
                    && self.current_page == Page::Connections
                {
                    if self.conn_list_selected > 0 {
                        self.conn_list_selected -= 1;
                        self.adjust_conn_scroll();
                    }
                }
            }
            MouseEventKind::Down(_) => {
                if self.sidebar_rect.contains((col, row).into()) {
                    let is_double = self.is_double_click(col, row);
                    self.focus = Focus::Sidebar;
                    if let Some(idx) = self.explorer_tree.hit_test(row, self.sidebar_rect) {
                        self.explorer_tree.selected = idx;
                        self.explorer_tree.adjust_offset(
                            self.sidebar_rect.height.saturating_sub(3) as usize,
                        );
                        if is_double {
                            let id = self.explorer_tree.selected_id();
                            if let Some(ref id) = id {
                                if id.starts_with("table:") && !id.ends_with(':') {
                                    let table_name = id.strip_prefix("table:").unwrap().to_string();
                                    // Strip schema prefix if present (e.g. "public.users" -> "users")
                                    let short = table_name.rsplit('.').next().unwrap_or(&table_name);
                                    let query = format!("SELECT * FROM \"{}\";\n", short);
                                    self.pending_action = PendingAction::OpenQuery(query);
                                } else {
                                    self.explorer_tree.toggle_selected();
                                }
                            }
                        }
                    }
                } else if self.inspector_rect.contains((col, row).into()) {
                    self.focus = Focus::Inspector;
                } else if self.main_rect.contains((col, row).into()) {
                    let is_double = self.is_double_click(col, row);
                    if self.current_page == Page::QueryEditor && is_double {
                        let editor_bottom = self.main_rect.y + self.main_rect.height * 33 / 100;
                        if row < editor_bottom {
                            self.editing_query = true;
                            self.query_editor.set_value(&self.query_text);
                        }
                    } else if self.current_page == Page::Connections
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
    }

    fn is_double_click(&mut self, col: u16, row: u16) -> bool {
        let now = Instant::now();
        let is_double = self
            .last_click
            .map(|(c, r, t)| c == col && r == row && now.duration_since(t) < Duration::from_millis(400))
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
            self.set_status(format!("Added \"{}\" — press Enter to connect", config.name));
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
        self.set_status(format!("Switched to {}", page.title()));
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Main,
            Focus::Main => Focus::Inspector,
            Focus::Inspector => Focus::Sidebar,
        };
        self.set_status(format!("Focus: {:?}", self.focus));
    }
}
