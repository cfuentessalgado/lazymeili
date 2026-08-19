use std::sync::Arc;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    action::Action,
    config::{Application, Config, ConfigStore, ConnectionColor, normalize_url},
    meili::{
        self, ApiKey, Capabilities, CreateKey, EnqueuedTask, HttpService, IndexInfo, SearchQuery,
        SearchResult, ServerVersion, Stats, Task, TaskFilter,
    },
    secrets::{SecretStore, Secrets},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Applications,
    Dashboard,
    Documents,
    Settings,
    Tasks,
    Keys,
}

impl Route {
    pub const ALL: [Self; 6] = [
        Self::Applications,
        Self::Dashboard,
        Self::Documents,
        Self::Settings,
        Self::Tasks,
        Self::Keys,
    ];
    pub const fn title(self) -> &'static str {
        match self {
            Self::Applications => "Applications",
            Self::Dashboard => "Indices",
            Self::Documents => "Documents",
            Self::Settings => "Settings",
            Self::Tasks => "Tasks",
            Self::Keys => "API keys",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Help,
    KeyForm(KeyFormState),
    ColorPicker {
        app_id: Uuid,
        cursor: usize,
    },
    Message {
        title: String,
        body: String,
    },
    Confirm {
        title: String,
        body: String,
    },
    Input {
        title: String,
        value: String,
        secret: bool,
    },
}

pub const API_KEY_ACTIONS: &[&str] = &[
    "*",
    "search",
    "documents.*",
    "documents.add",
    "documents.get",
    "documents.delete",
    "indexes.*",
    "indexes.create",
    "indexes.get",
    "indexes.update",
    "indexes.delete",
    "indexes.swap",
    "tasks.*",
    "tasks.get",
    "tasks.cancel",
    "tasks.delete",
    "settings.*",
    "settings.get",
    "settings.update",
    "stats.*",
    "stats.get",
    "metrics.*",
    "metrics.get",
    "dumps.*",
    "dumps.create",
    "snapshots.*",
    "snapshots.create",
    "version",
    "keys.get",
    "keys.create",
    "keys.update",
    "keys.delete",
    "experimental.get",
    "experimental.update",
    "export",
    "network.get",
    "network.update",
    "chatCompletions",
    "chats.*",
    "chats.get",
    "chats.delete",
    "chatsSettings.*",
    "chatsSettings.get",
    "chatsSettings.update",
    "webhooks.*",
    "webhooks.get",
    "webhooks.create",
    "webhooks.update",
    "webhooks.delete",
    "indexes.compact",
    "tasks.compact",
    "dynamicSearchRules.*",
    "dynamicSearchRules.get",
    "dynamicSearchRules.create",
    "dynamicSearchRules.update",
    "dynamicSearchRules.delete",
];

pub const KEY_PRESETS: &[&str] = &[
    "Full access",
    "Read-only observability",
    "Read-only documents and settings",
    "Search only (minimal)",
    "Custom",
];
pub const EXPIRY_PRESETS: &[&str] = &["30 days", "180 days", "365 days", "Never"];

const FULL_ACCESS_PRESET: &[&str] = &["*"];
const OBSERVABILITY_PRESET: &[&str] = &[
    "indexes.get",
    "tasks.get",
    "stats.get",
    "metrics.get",
    "version",
];
const READ_ONLY_CONTENT_PRESET: &[&str] = &[
    "search",
    "documents.get",
    "indexes.get",
    "tasks.get",
    "settings.get",
    "stats.get",
    "metrics.get",
    "version",
];
const MINIMAL_ACCESS_PRESET: &[&str] = &["search"];
const CUSTOM_PRESET: usize = 4;
const KEY_FORM_FIELDS: usize = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyFormState {
    pub focus: usize,
    pub preset_choice: usize,
    pub uid: String,
    pub name: String,
    pub description: String,
    pub indexes: String,
    pub actions: Vec<String>,
    pub action_cursor: usize,
    pub picking_actions: bool,
    pub expiry_choice: usize,
}

impl Default for KeyFormState {
    fn default() -> Self {
        Self {
            focus: 2,
            preset_choice: 0,
            uid: Uuid::new_v4().to_string(),
            name: String::new(),
            description: String::new(),
            indexes: "*".into(),
            actions: vec!["*".into()],
            action_cursor: 0,
            picking_actions: false,
            expiry_choice: 3,
        }
    }
}

impl KeyFormState {
    fn focused_value_mut(&mut self) -> &mut String {
        match self.focus {
            1 => &mut self.uid,
            2 => &mut self.name,
            3 => &mut self.description,
            4 => &mut self.indexes,
            _ => unreachable!("preset, actions, and expiry use selection controls"),
        }
    }

    fn choose_preset(&mut self, choice: usize) {
        self.preset_choice = choice;
        let actions = match choice {
            0 => FULL_ACCESS_PRESET,
            1 => OBSERVABILITY_PRESET,
            2 => READ_ONLY_CONTENT_PRESET,
            3 => MINIMAL_ACCESS_PRESET,
            _ => return,
        };
        self.actions = actions.iter().map(|action| (*action).into()).collect();
    }
}

#[derive(Debug, Clone)]
pub enum EditorTarget {
    Search,
    Documents,
    Document,
    Settings,
    UpdateKey(String),
}

#[derive(Debug, Clone)]
pub enum Command {
    Connect {
        url: String,
        key: Option<String>,
    },
    RefreshDashboard,
    Search(SearchQuery),
    CreateIndex {
        uid: String,
        primary_key: Option<String>,
    },
    UpdatePrimaryKey {
        uid: String,
        primary_key: String,
    },
    DeleteIndex(String),
    AddDocuments(Value),
    UpdateDocuments(Value),
    DeleteDocument {
        id: String,
    },
    UpdateSettings(Value),
    FetchTasks(TaskFilter),
    CancelTask(u64),
    FetchKeys(usize),
    CreateKey(CreateKey),
    UpdateKey {
        uid: String,
        name: String,
        description: String,
    },
    DeleteKey(String),
    CreateDump,
}

#[derive(Debug)]
pub enum Message {
    Connected {
        service: Arc<HttpService>,
        version: ServerVersion,
        stats: Stats,
        indexes: Vec<IndexInfo>,
    },
    Dashboard {
        stats: Stats,
        indexes: Vec<IndexInfo>,
    },
    Search(SearchResult),
    Settings(Value),
    Tasks(meili::Page<Task>),
    Keys(meili::Page<ApiKey>),
    TaskQueued(EnqueuedTask),
    TaskFinished(Task),
    KeyCreated(ApiKey),
    KeyUpdated(ApiKey),
    KeyDeleted,
    Error(String),
}

#[derive(Debug, Clone)]
enum InputPurpose {
    AppName,
    AppUrl,
    AppKey,
    EditAppName(Uuid),
    EditAppUrl(Uuid),
    EditAppKey(Uuid, bool),
    DeleteApplication(Uuid, String),
    IndexUid,
    PrimaryKeyForCreate,
    UpdatePrimaryKey,
    DeleteIndex(String),
    IndexFilter,
    Search,
    UploadPath,
    DeleteDocument(String),
    TaskFilter,
    CancelTask(u64),
    DeleteKey(String),
    Dump,
}

#[derive(Debug, Default)]
struct Draft {
    name: String,
    url: String,
    uid: String,
}

pub struct App {
    pub running: bool,
    pub route: Route,
    pub selected: usize,
    pub offset: usize,
    pub loading: bool,
    pub overlay: Option<Overlay>,
    pub notice: Option<String>,
    pub config: Config,
    pub active: Option<Uuid>,
    pub service: Option<Arc<HttpService>>,
    pub version: Option<ServerVersion>,
    pub capabilities: Capabilities,
    pub stats: Option<Stats>,
    pub indexes: Vec<IndexInfo>,
    pub index_filter: String,
    pub selected_index: Option<String>,
    pub search_query: SearchQuery,
    pub search: SearchResult,
    pub settings: Value,
    pub task_filter: TaskFilter,
    pub tasks: Vec<Task>,
    pub task_total: Option<usize>,
    pub task_next: Option<u64>,
    pub keys: Vec<ApiKey>,
    pub key_total: Option<usize>,
    store: ConfigStore,
    secrets: Secrets,
    input: Option<InputPurpose>,
    pending_command: Option<Command>,
    draft: Draft,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("route", &self.route)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl App {
    #[must_use]
    pub fn new(config: Config, store: ConfigStore, secrets: Secrets) -> Self {
        Self {
            running: true,
            route: Route::Applications,
            selected: 0,
            offset: 0,
            loading: false,
            overlay: None,
            notice: None,
            active: config.selected,
            config,
            store,
            secrets,
            service: None,
            version: None,
            capabilities: Capabilities::default(),
            stats: None,
            indexes: Vec::new(),
            index_filter: String::new(),
            selected_index: None,
            search_query: SearchQuery {
                q: String::new(),
                offset: 0,
                limit: 20,
                ..SearchQuery::default()
            },
            search: SearchResult::default(),
            settings: Value::Null,
            task_filter: TaskFilter {
                limit: 20,
                ..TaskFilter::default()
            },
            tasks: Vec::new(),
            task_total: None,
            task_next: None,
            keys: Vec::new(),
            key_total: None,
            input: None,
            pending_command: None,
            draft: Draft::default(),
        }
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        matches!(
            self.overlay,
            Some(Overlay::Input { .. } | Overlay::KeyForm(_) | Overlay::ColorPicker { .. })
        )
    }

    pub fn startup_command(&mut self) -> Option<Command> {
        let id = self.active?;
        self.connect_command(id)
    }

    pub fn update(&mut self, action: Action) -> Vec<CommandOrEditor> {
        if matches!(action, Action::Quit) && self.overlay.is_none() {
            self.running = false;
            return Vec::new();
        }
        if matches!(self.overlay, Some(Overlay::KeyForm(_))) {
            return self.update_key_form(action);
        }
        if matches!(self.overlay, Some(Overlay::ColorPicker { .. })) {
            self.update_color_picker(action);
            return Vec::new();
        }
        if let Some(overlay) = &mut self.overlay {
            match action {
                Action::Escape => {
                    self.overlay = None;
                    self.input = None;
                    self.pending_command = None;
                }
                Action::Backspace => {
                    if let Overlay::Input { value, .. } = overlay {
                        value.pop();
                    }
                }
                Action::Input(c) => {
                    if let Overlay::Input { value, .. } = overlay {
                        value.push(c);
                    }
                }
                Action::Yank => {
                    if let Overlay::Message { title, body } = overlay
                        && title.starts_with("SECRET API KEY VALUE")
                    {
                        return vec![CommandOrEditor::Clipboard(body.clone())];
                    }
                }
                Action::Confirm if matches!(overlay, Overlay::Confirm { .. }) => {
                    self.overlay = None;
                    return self
                        .pending_command
                        .take()
                        .into_iter()
                        .map(|command| command.into())
                        .collect();
                }
                Action::Confirm => return self.finish_input(),
                _ => {}
            }
            return Vec::new();
        }
        match action {
            Action::Help => self.overlay = Some(Overlay::Help),
            Action::Next => self.next(),
            Action::Previous => self.selected = self.selected.saturating_sub(1),
            Action::Right => {
                self.change_route(1);
                if self.route != Route::Applications {
                    return self.refresh();
                }
            }
            Action::Left => {
                self.change_route(-1);
                if self.route != Route::Applications {
                    return self.refresh();
                }
            }
            Action::PageNext => {
                self.offset = self.offset.saturating_add(20);
                if self.route == Route::Tasks {
                    self.task_filter.from = self.task_next;
                }
                return self.refresh();
            }
            Action::PagePrevious => {
                self.offset = self.offset.saturating_sub(20);
                if self.route == Route::Tasks {
                    self.task_filter.from = None;
                }
                return self.refresh();
            }
            Action::Refresh => return self.refresh(),
            Action::Confirm => return self.open_selected(),
            Action::Create => return self.create(),
            Action::Edit => return self.edit(),
            Action::Delete => return self.delete(),
            Action::Color => self.pick_connection_color(),
            Action::Search => {
                if self.route == Route::Tasks {
                    self.prompt(
                        InputPurpose::TaskFilter,
                        "Filter: index=movies status=failed type=documentAdditionOrUpdate",
                        String::new(),
                        false,
                    );
                } else if self.route == Route::Dashboard {
                    self.prompt(
                        InputPurpose::IndexFilter,
                        "Filter indices by UID or field",
                        self.index_filter.clone(),
                        false,
                    );
                } else if self.route == Route::Documents {
                    let value = serde_json::to_value(&self.search_query).unwrap_or_default();
                    return vec![CommandOrEditor::Editor(EditorTarget::Search, value)];
                } else {
                    self.prompt(
                        InputPurpose::Search,
                        "Search query",
                        self.search_query.q.clone(),
                        false,
                    );
                }
            }
            Action::Applications => {
                self.route = Route::Applications;
                self.selected = self
                    .active
                    .and_then(|id| self.config.applications.iter().position(|app| app.id == id))
                    .unwrap_or(0);
                self.offset = 0;
            }
            Action::Settings => {
                if self.selected_index.is_some() {
                    self.route = Route::Settings;
                    return self.refresh();
                }
                self.notice = Some("Select an index before opening settings.".into());
            }
            Action::Tasks => {
                self.route = Route::Tasks;
                return self.refresh();
            }
            Action::Keys => {
                self.route = Route::Keys;
                return self.refresh();
            }
            Action::Dump => self.prompt(
                InputPurpose::Dump,
                "Type DUMP to create a server-side dump",
                String::new(),
                false,
            ),
            _ => {}
        }
        Vec::new()
    }

    fn update_key_form(&mut self, action: Action) -> Vec<CommandOrEditor> {
        let Some(Overlay::KeyForm(form)) = &mut self.overlay else {
            return Vec::new();
        };
        if form.picking_actions {
            match action {
                Action::Escape | Action::Left => form.picking_actions = false,
                Action::Right => {
                    form.picking_actions = false;
                    form.focus = 6;
                }
                Action::Next => {
                    form.action_cursor = (form.action_cursor + 1) % API_KEY_ACTIONS.len()
                }
                Action::Previous => {
                    form.action_cursor =
                        (form.action_cursor + API_KEY_ACTIONS.len() - 1) % API_KEY_ACTIONS.len();
                }
                Action::Confirm => {
                    form.picking_actions = false;
                    form.focus = 6;
                }
                Action::Input(' ') => {
                    form.preset_choice = CUSTOM_PRESET;
                    let selected = API_KEY_ACTIONS[form.action_cursor];
                    if selected == "*" {
                        form.actions.clear();
                        form.actions.push("*".into());
                    } else if let Some(position) =
                        form.actions.iter().position(|item| item == selected)
                    {
                        form.actions.remove(position);
                    } else {
                        form.actions.retain(|item| item != "*");
                        form.actions.push(selected.into());
                    }
                }
                _ => {}
            }
            return Vec::new();
        }
        match action {
            Action::Escape => self.overlay = None,
            Action::Next => form.focus = (form.focus + 1) % KEY_FORM_FIELDS,
            Action::Previous => {
                form.focus = (form.focus + KEY_FORM_FIELDS - 1) % KEY_FORM_FIELDS;
            }
            Action::Right if form.focus == 0 => {
                form.choose_preset((form.preset_choice + 1) % KEY_PRESETS.len());
            }
            Action::Left if form.focus == 0 => {
                form.choose_preset(
                    (form.preset_choice + KEY_PRESETS.len() - 1) % KEY_PRESETS.len(),
                );
            }
            Action::Right if form.focus == 6 => {
                form.expiry_choice = (form.expiry_choice + 1) % EXPIRY_PRESETS.len();
            }
            Action::Left if form.focus == 6 => {
                form.expiry_choice =
                    (form.expiry_choice + EXPIRY_PRESETS.len() - 1) % EXPIRY_PRESETS.len();
            }
            Action::Right => form.focus = (form.focus + 1) % KEY_FORM_FIELDS,
            Action::Left => {
                form.focus = (form.focus + KEY_FORM_FIELDS - 1) % KEY_FORM_FIELDS;
            }
            Action::Input(character) if (1..=4).contains(&form.focus) => {
                form.focused_value_mut().push(character);
            }
            Action::Backspace if (1..=4).contains(&form.focus) => {
                form.focused_value_mut().pop();
            }
            Action::Confirm if form.focus == 5 => form.picking_actions = true,
            Action::Confirm if form.focus < 6 => form.focus += 1,
            Action::Confirm => {
                let form = form.clone();
                if form.name.trim().is_empty() {
                    self.error("API key name is required");
                    return Vec::new();
                }
                if form.actions.is_empty() {
                    self.error("Select at least one API key action");
                    return Vec::new();
                }
                self.overlay = None;
                let mut indexes = form
                    .indexes
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if indexes.is_empty() {
                    indexes.push("*".into());
                }
                let expires_at = match form.expiry_choice {
                    0 => Some(
                        (chrono::Utc::now() + chrono::Duration::days(30))
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    ),
                    1 => Some(
                        (chrono::Utc::now() + chrono::Duration::days(180))
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    ),
                    2 => Some(
                        (chrono::Utc::now() + chrono::Duration::days(365))
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    ),
                    _ => None,
                };
                return vec![
                    Command::CreateKey(CreateKey {
                        uid: nonempty(form.uid),
                        name: form.name,
                        description: form.description,
                        indexes,
                        actions: form.actions,
                        expires_at,
                    })
                    .into(),
                ];
            }
            _ => {}
        }
        Vec::new()
    }

    fn pick_connection_color(&mut self) {
        if self.route != Route::Applications {
            return;
        }
        let Some(app) = self.config.applications.get(self.selected) else {
            return;
        };
        let cursor = ConnectionColor::ALL
            .iter()
            .position(|color| *color == app.color)
            .unwrap_or(0);
        self.overlay = Some(Overlay::ColorPicker {
            app_id: app.id,
            cursor,
        });
    }

    fn update_color_picker(&mut self, action: Action) {
        let Some(Overlay::ColorPicker { app_id, cursor }) = &mut self.overlay else {
            return;
        };
        match action {
            Action::Escape => self.overlay = None,
            Action::Next | Action::Right => {
                *cursor = (*cursor + 1) % ConnectionColor::ALL.len();
            }
            Action::Previous | Action::Left => {
                *cursor = cursor
                    .checked_sub(1)
                    .unwrap_or(ConnectionColor::ALL.len() - 1);
            }
            Action::Confirm => {
                let app_id = *app_id;
                let color = ConnectionColor::ALL[*cursor];
                if let Some(app) = self
                    .config
                    .applications
                    .iter_mut()
                    .find(|app| app.id == app_id)
                {
                    app.color = color;
                }
                match self.store.save(&self.config) {
                    Ok(()) => {
                        self.overlay = None;
                        self.notice = Some(format!("Connection color set to {}", color.label()));
                    }
                    Err(error) => self.error(error.to_string()),
                }
            }
            _ => {}
        }
    }

    fn prompt(&mut self, purpose: InputPurpose, title: &str, value: String, secret: bool) {
        self.input = Some(purpose);
        self.overlay = Some(Overlay::Input {
            title: title.into(),
            value,
            secret,
        });
    }

    fn finish_input(&mut self) -> Vec<CommandOrEditor> {
        let value = match self.overlay.take() {
            Some(Overlay::Input { value, .. }) => value,
            _ => return Vec::new(),
        };
        let Some(purpose) = self.input.take() else {
            return Vec::new();
        };
        match purpose {
            InputPurpose::AppName => {
                if value.trim().is_empty() {
                    self.error("Application name is required");
                } else {
                    self.draft.name = value;
                    self.prompt(
                        InputPurpose::AppUrl,
                        "Meilisearch URL",
                        "http://127.0.0.1:7700".into(),
                        false,
                    );
                }
            }
            InputPurpose::AppUrl => match normalize_url(&value) {
                Ok(url) => {
                    self.draft.url = url;
                    self.prompt(
                        InputPurpose::AppKey,
                        "API key (optional)",
                        String::new(),
                        true,
                    );
                }
                Err(error) => self.error(error.to_string()),
            },
            InputPurpose::AppKey => return self.save_application(value),
            InputPurpose::EditAppName(id) => {
                if value.trim().is_empty() {
                    self.error("Application name is required");
                } else {
                    self.draft.name = value;
                    let url = self
                        .config
                        .applications
                        .iter()
                        .find(|app| app.id == id)
                        .map_or_else(String::new, |app| app.url.clone());
                    self.prompt(InputPurpose::EditAppUrl(id), "Meilisearch URL", url, false);
                }
            }
            InputPurpose::EditAppUrl(id) => match normalize_url(&value) {
                Ok(url) => {
                    self.draft.url = url;
                    let has_key = self
                        .config
                        .applications
                        .iter()
                        .find(|app| app.id == id)
                        .is_some_and(|app| app.has_api_key);
                    self.prompt(
                        InputPurpose::EditAppKey(id, has_key),
                        "API key: empty keeps it; '-' removes it",
                        String::new(),
                        true,
                    );
                }
                Err(error) => self.error(error.to_string()),
            },
            InputPurpose::EditAppKey(id, has_key) => {
                self.save_edited_application(id, value, has_key)
            }
            InputPurpose::DeleteApplication(id, expected) => {
                if value == expected {
                    if let Err(error) = self
                        .secrets
                        .delete(id)
                        .and_then(|()| self.store.remove(&mut self.config, id))
                    {
                        self.error(error.to_string());
                    } else {
                        self.active = self.config.selected;
                        self.service = None;
                        self.notice = Some("Application removed".into());
                    }
                } else {
                    self.error("Name did not match");
                }
            }
            InputPurpose::IndexUid => {
                if value.is_empty() {
                    self.error("Index UID is required");
                } else {
                    self.draft.uid = value;
                    self.prompt(
                        InputPurpose::PrimaryKeyForCreate,
                        "Primary key (optional)",
                        String::new(),
                        false,
                    );
                }
            }
            InputPurpose::PrimaryKeyForCreate => {
                return vec![
                    Command::CreateIndex {
                        uid: self.draft.uid.clone(),
                        primary_key: nonempty(value),
                    }
                    .into(),
                ];
            }
            InputPurpose::UpdatePrimaryKey => {
                if let Some(uid) = self.selected_index.clone() {
                    return vec![
                        Command::UpdatePrimaryKey {
                            uid,
                            primary_key: value,
                        }
                        .into(),
                    ];
                }
            }
            InputPurpose::DeleteIndex(expected) => {
                if value == expected {
                    return vec![Command::DeleteIndex(expected).into()];
                } else {
                    self.error("Index UID did not match");
                }
            }
            InputPurpose::IndexFilter => {
                self.index_filter = value;
                self.selected = 0;
                self.offset = 0;
            }
            InputPurpose::Search => {
                self.search_query.q = value;
                self.search_query.offset = 0;
                return vec![Command::Search(self.search_query.clone()).into()];
            }
            InputPurpose::UploadPath => {
                if value.trim().is_empty() {
                    return vec![CommandOrEditor::Editor(EditorTarget::Documents, json!([]))];
                }
                match std::fs::read_to_string(value.trim())
                    .map_err(anyhow::Error::from)
                    .and_then(|text| serde_json::from_str::<Value>(&text).map_err(Into::into))
                {
                    Ok(documents) if documents.is_array() => {
                        return vec![Command::AddDocuments(documents).into()];
                    }
                    Ok(_) => self.error("Document file must contain a JSON array"),
                    Err(error) => self.error(format!("Cannot read document file: {error}")),
                }
            }
            InputPurpose::DeleteDocument(expected) => {
                if value == expected {
                    return vec![Command::DeleteDocument { id: expected }.into()];
                } else {
                    self.error("Document ID did not match");
                }
            }
            InputPurpose::TaskFilter => {
                self.parse_task_filter(&value);
                return vec![Command::FetchTasks(self.task_filter.clone()).into()];
            }
            InputPurpose::CancelTask(uid) => {
                if value == uid.to_string() {
                    return vec![Command::CancelTask(uid).into()];
                } else {
                    self.error("Task UID did not match");
                }
            }
            InputPurpose::DeleteKey(uid) => {
                if value == uid {
                    return vec![Command::DeleteKey(uid).into()];
                } else {
                    self.error("Key UID did not match");
                }
            }
            InputPurpose::Dump => {
                if value == "DUMP" {
                    return vec![Command::CreateDump.into()];
                } else {
                    self.error("Confirmation did not match");
                }
            }
        }
        Vec::new()
    }

    fn save_application(&mut self, key: String) -> Vec<CommandOrEditor> {
        let id = Uuid::new_v4();
        if !key.is_empty() {
            if let Err(error) = self.secrets.set(id, &key) {
                self.error(error.to_string());
                return Vec::new();
            }
        }
        let app = Application {
            id,
            name: self.draft.name.clone(),
            url: self.draft.url.clone(),
            has_api_key: !key.is_empty(),
            color: ConnectionColor::default(),
        };
        if let Err(error) = self.store.upsert(&mut self.config, app) {
            let _ = self.secrets.delete(id);
            self.error(error.to_string());
            return Vec::new();
        }
        self.config.selected = Some(id);
        let _ = self.store.save(&self.config);
        self.active = Some(id);
        vec![
            Command::Connect {
                url: self.draft.url.clone(),
                key: nonempty(key),
            }
            .into(),
        ]
    }

    fn save_edited_application(&mut self, id: Uuid, key: String, had_key: bool) {
        let has_api_key = if key == "-" {
            if let Err(error) = self.secrets.delete(id) {
                self.error(error.to_string());
                return;
            }
            false
        } else if key.is_empty() {
            had_key
        } else {
            if let Err(error) = self.secrets.set(id, &key) {
                self.error(error.to_string());
                return;
            }
            true
        };
        let color = self
            .config
            .applications
            .iter()
            .find(|app| app.id == id)
            .map_or_else(ConnectionColor::default, |app| app.color);
        let edited = Application {
            id,
            name: self.draft.name.clone(),
            url: self.draft.url.clone(),
            has_api_key,
            color,
        };
        if let Err(error) = self.store.upsert(&mut self.config, edited) {
            self.error(error.to_string());
            return;
        }
        if self.active == Some(id) {
            self.service = None;
        }
        self.route = Route::Applications;
        self.notice = Some("Application saved. Press Enter to connect.".into());
    }

    fn connect_command(&mut self, id: Uuid) -> Option<Command> {
        let app = self
            .config
            .applications
            .iter()
            .find(|app| app.id == id)?
            .clone();
        let key = if app.has_api_key {
            match self.secrets.get(id) {
                Ok(value) => value,
                Err(error) => {
                    self.error(error.to_string());
                    return None;
                }
            }
        } else {
            None
        };
        // Do not keep the previous client while a new connection is pending.
        // The UI must never identify a stale client as the active connection.
        self.service = None;
        self.version = None;
        self.loading = true;
        Some(Command::Connect { url: app.url, key })
    }

    fn next(&mut self) {
        let len = match self.route {
            Route::Applications => self.config.applications.len(),
            Route::Dashboard => self.displayed_indexes().len(),
            Route::Documents => self.search.hits.len(),
            Route::Tasks => self.tasks.len(),
            Route::Keys => self.keys.len(),
            Route::Settings => 1,
        };
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }
    fn change_route(&mut self, delta: isize) {
        let mut current = Route::ALL
            .iter()
            .position(|route| *route == self.route)
            .unwrap_or(0);
        loop {
            current = (current.cast_signed() + delta)
                .rem_euclid(Route::ALL.len().cast_signed())
                .cast_unsigned();
            let candidate = Route::ALL[current];
            if self.selected_index.is_some()
                || !matches!(candidate, Route::Documents | Route::Settings)
            {
                self.route = candidate;
                break;
            }
        }
        self.selected = 0;
        self.offset = 0;
    }

    fn open_selected(&mut self) -> Vec<CommandOrEditor> {
        match self.route {
            Route::Applications => {
                if let Some(id) = self
                    .config
                    .applications
                    .get(self.selected)
                    .map(|app| app.id)
                {
                    self.active = Some(id);
                    self.config.selected = Some(id);
                    let _ = self.store.save(&self.config);
                    return self
                        .connect_command(id)
                        .into_iter()
                        .map(Into::into)
                        .collect();
                }
            }
            Route::Dashboard => {
                if let Some(uid) = self
                    .displayed_indexes()
                    .get(self.selected)
                    .map(|index| index.uid.clone())
                {
                    self.selected_index = Some(uid);
                    self.route = Route::Documents;
                    return vec![Command::Search(self.search_query.clone()).into()];
                }
            }
            Route::Documents => {
                if let Some(doc) = self.search.hits.get(self.selected) {
                    self.overlay = Some(Overlay::Message {
                        title: "Document JSON".into(),
                        body: pretty(doc),
                    });
                }
            }
            Route::Tasks => {
                if let Some(task) = self.tasks.get(self.selected) {
                    self.overlay = Some(Overlay::Message {
                        title: format!("Task {}", task.uid),
                        body: pretty(task),
                    });
                }
            }
            Route::Keys => {
                if let Some(key) = self.keys.get(self.selected) {
                    self.overlay = Some(Overlay::Message {
                        title: format!("Key {}", key.uid),
                        body: pretty(key),
                    });
                }
            }
            Route::Settings => {
                self.overlay = Some(Overlay::Message {
                    title: "Index settings".into(),
                    body: pretty(&self.settings),
                })
            }
        }
        Vec::new()
    }

    fn create(&mut self) -> Vec<CommandOrEditor> {
        match self.route {
            Route::Applications => self.prompt(
                InputPurpose::AppName,
                "Application name",
                String::new(),
                false,
            ),
            Route::Dashboard => self.prompt(
                InputPurpose::IndexUid,
                "New index UID",
                String::new(),
                false,
            ),
            Route::Documents => self.prompt(
                InputPurpose::UploadPath,
                "JSON array file path (empty opens $VISUAL/$EDITOR)",
                String::new(),
                false,
            ),
            Route::Keys => self.overlay = Some(Overlay::KeyForm(KeyFormState::default())),
            _ => {}
        }
        Vec::new()
    }

    fn edit(&mut self) -> Vec<CommandOrEditor> {
        match self.route {
            Route::Applications => {
                if let Some(app) = self.config.applications.get(self.selected) {
                    self.prompt(
                        InputPurpose::EditAppName(app.id),
                        "Application name",
                        app.name.clone(),
                        false,
                    );
                }
            }
            Route::Dashboard => {
                if let Some(uid) = self
                    .displayed_indexes()
                    .get(self.selected)
                    .map(|index| index.uid.clone())
                {
                    self.selected_index = Some(uid);
                    self.prompt(
                        InputPurpose::UpdatePrimaryKey,
                        "New primary key",
                        String::new(),
                        false,
                    );
                }
            }
            Route::Documents => {
                if let Some(doc) = self.search.hits.get(self.selected) {
                    return vec![CommandOrEditor::Editor(EditorTarget::Document, doc.clone())];
                }
            }
            Route::Settings => {
                return vec![CommandOrEditor::Editor(
                    EditorTarget::Settings,
                    self.settings.clone(),
                )];
            }
            Route::Keys => {
                if let Some(key) = self.keys.get(self.selected) {
                    return vec![CommandOrEditor::Editor(
                        EditorTarget::UpdateKey(key.uid.clone()),
                        json!({"name":key.name,"description":key.description}),
                    )];
                }
            }
            Route::Tasks => {}
        }
        Vec::new()
    }

    fn delete(&mut self) -> Vec<CommandOrEditor> {
        match self.route {
            Route::Applications => {
                if let Some(app) = self.config.applications.get(self.selected) {
                    self.prompt(
                        InputPurpose::DeleteApplication(app.id, app.name.clone()),
                        &format!("Type {} to remove application", app.name),
                        String::new(),
                        false,
                    );
                }
            }
            Route::Dashboard => {
                if let Some(uid) = self
                    .displayed_indexes()
                    .get(self.selected)
                    .map(|index| index.uid.clone())
                {
                    self.prompt(
                        InputPurpose::DeleteIndex(uid.clone()),
                        &format!("Type {uid} to delete index and all data"),
                        String::new(),
                        false,
                    );
                }
            }
            Route::Documents => {
                if let Some(doc) = self.search.hits.get(self.selected) {
                    if let Some(id) = self.document_id(doc) {
                        self.prompt(
                            InputPurpose::DeleteDocument(id.clone()),
                            &format!("Type {id} to delete document"),
                            String::new(),
                            false,
                        );
                    } else {
                        self.error("Cannot find the index primary key in this document");
                    }
                }
            }
            Route::Tasks => {
                if let Some(task) = self.tasks.get(self.selected) {
                    if matches!(task.status.as_str(), "enqueued" | "processing")
                        && self.capabilities.task_cancel
                    {
                        self.prompt(
                            InputPurpose::CancelTask(task.uid),
                            &format!("Type {} to cancel task", task.uid),
                            String::new(),
                            false,
                        );
                    }
                }
            }
            Route::Keys => {
                if let Some(key) = self.keys.get(self.selected) {
                    self.prompt(
                        InputPurpose::DeleteKey(key.uid.clone()),
                        &format!("Type {} to delete API key", key.uid),
                        String::new(),
                        false,
                    );
                }
            }
            Route::Settings => {}
        }
        Vec::new()
    }

    fn document_id(&self, doc: &Value) -> Option<String> {
        let primary = self
            .indexes
            .iter()
            .find(|index| Some(&index.uid) == self.selected_index.as_ref())?
            .primary_key
            .as_ref()?;
        doc.get(primary).map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned)
        })
    }

    fn refresh(&mut self) -> Vec<CommandOrEditor> {
        if self.service.is_none() && self.route != Route::Applications {
            self.error("Select and connect to an application first");
            return Vec::new();
        }
        self.loading = true;
        match self.route {
            Route::Applications => self
                .active
                .and_then(|id| self.connect_command(id))
                .into_iter()
                .map(Into::into)
                .collect(),
            Route::Dashboard => vec![Command::RefreshDashboard.into()],
            Route::Documents => vec![Command::Search(self.search_query.clone()).into()],
            Route::Settings => self
                .selected_index
                .as_ref()
                .map_or_else(Vec::new, |_| vec![CommandOrEditor::FetchSettings]),
            Route::Tasks => vec![Command::FetchTasks(self.task_filter.clone()).into()],
            Route::Keys => vec![Command::FetchKeys(self.offset).into()],
        }
    }

    fn parse_task_filter(&mut self, value: &str) {
        self.task_filter.index_uids.clear();
        self.task_filter.statuses.clear();
        self.task_filter.types.clear();
        for part in value.split_whitespace() {
            if let Some((key, values)) = part.split_once('=') {
                let target = match key {
                    "index" => &mut self.task_filter.index_uids,
                    "status" => &mut self.task_filter.statuses,
                    "type" => &mut self.task_filter.types,
                    _ => continue,
                };
                target.extend(
                    values
                        .split(',')
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                );
            }
        }
    }
    fn error(&mut self, error: impl Into<String>) {
        self.overlay = Some(Overlay::Message {
            title: "Error".into(),
            body: error.into(),
        });
    }

    pub fn editor_result(&mut self, target: EditorTarget, value: Value) -> Vec<CommandOrEditor> {
        let command = match target {
            EditorTarget::Search => match serde_json::from_value::<SearchQuery>(value) {
                Ok(mut query) => {
                    if !self.capabilities.hybrid_search {
                        query.hybrid = None;
                    }
                    if !self.capabilities.ranking_score_threshold {
                        query.ranking_score_threshold = None;
                    }
                    self.search_query = query.clone();
                    Command::Search(query)
                }
                Err(error) => {
                    self.error(format!("Invalid search form: {error}"));
                    return Vec::new();
                }
            },
            EditorTarget::Documents => {
                if !value.is_array() {
                    self.error("Document upload JSON must be an array");
                    return Vec::new();
                }
                Command::AddDocuments(value)
            }
            EditorTarget::Document => {
                if !value.is_object() {
                    self.error("A document must be a JSON object");
                    return Vec::new();
                }
                Command::UpdateDocuments(json!([value]))
            }
            EditorTarget::Settings => {
                if !value.is_object() {
                    self.error("Settings must be a JSON object");
                    return Vec::new();
                }
                let before = redact_secrets(self.settings.clone());
                let after = redact_secrets(value.clone());
                let diff = similar::TextDiff::from_lines(&pretty(&before), &pretty(&after))
                    .unified_diff()
                    .header("current", "edited")
                    .to_string();
                self.pending_command = Some(Command::UpdateSettings(value));
                self.overlay = Some(Overlay::Confirm {
                    title: "Apply settings changes? — Enter confirms".into(),
                    body: diff,
                });
                return Vec::new();
            }
            EditorTarget::UpdateKey(uid) => {
                let name = value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let description = value
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                Command::UpdateKey {
                    uid,
                    name,
                    description,
                }
            }
        };
        vec![command.into()]
    }

    pub fn apply(&mut self, message: Message) {
        self.loading = false;
        match message {
            Message::Connected {
                service,
                version,
                stats,
                indexes,
            } => {
                self.capabilities = Capabilities::for_version(&version.pkg_version);
                self.version = Some(version);
                self.stats = Some(stats);
                self.indexes = indexes;
                self.service = Some(service);
                self.route = Route::Dashboard;
                self.selected = 0;
                self.notice = Some("Connected".into());
            }
            Message::Dashboard { stats, indexes } => {
                self.stats = Some(stats);
                self.indexes = indexes;
                self.clamp();
            }
            Message::Search(result) => {
                self.search = result;
                self.clamp();
            }
            Message::Settings(value) => self.settings = value,
            Message::Tasks(page) => {
                self.task_total = page.total;
                self.task_next = page.next;
                self.tasks = page.results;
                self.clamp();
            }
            Message::Keys(page) => {
                self.key_total = page.total;
                self.keys = page.results;
                self.clamp();
            }
            Message::TaskQueued(task) => {
                self.notice = Some(format!(
                    "Task {} queued; open Tasks to track it",
                    task.task_uid
                ));
            }
            Message::TaskFinished(task) => {
                self.notice = Some(format!("Task {} {}", task.uid, task.status));
                if task.status != "succeeded" {
                    self.overlay = Some(Overlay::Message {
                        title: format!("Task {} {}", task.uid, task.status),
                        body: pretty(&task),
                    });
                }
            }
            Message::KeyCreated(key) => {
                let secret = key
                    .key
                    .as_deref()
                    .unwrap_or("The server did not return the key value.");
                self.overlay = Some(Overlay::Message {
                    title: "SECRET API KEY VALUE — press y to copy; the UUID cannot authenticate"
                        .into(),
                    body: secret.into(),
                });
            }
            Message::KeyUpdated(key) => {
                if let Some(existing) = self.keys.iter_mut().find(|item| item.uid == key.uid) {
                    *existing = key;
                }
            }
            Message::KeyDeleted => {
                self.notice = Some("API key deleted".into());
            }
            Message::Error(error) => self.error(error),
        }
    }
    pub fn visible_indexes(&self) -> Vec<&IndexInfo> {
        let needle = self.index_filter.to_ascii_lowercase();
        self.indexes
            .iter()
            .filter(|index| {
                needle.is_empty()
                    || index.uid.to_ascii_lowercase().contains(&needle)
                    || self
                        .stats
                        .as_ref()
                        .and_then(|stats| stats.indexes.get(&index.uid))
                        .is_some_and(|stats| {
                            stats
                                .field_distribution
                                .keys()
                                .any(|field| field.to_ascii_lowercase().contains(&needle))
                        })
            })
            .collect()
    }

    pub fn displayed_indexes(&self) -> Vec<&IndexInfo> {
        self.visible_indexes()
            .into_iter()
            .skip(self.offset)
            .take(20)
            .collect()
    }

    pub fn timed_refresh(&mut self) -> Vec<CommandOrEditor> {
        if matches!(self.route, Route::Documents | Route::Tasks)
            && self.overlay.is_none()
            && !self.loading
        {
            self.refresh()
        } else {
            Vec::new()
        }
    }

    fn clamp(&mut self) {
        self.selected = self.selected.min(
            match self.route {
                Route::Applications => self.config.applications.len(),
                Route::Dashboard => self.displayed_indexes().len(),
                Route::Documents => self.search.hits.len(),
                Route::Tasks => self.tasks.len(),
                Route::Keys => self.keys.len(),
                Route::Settings => 1,
            }
            .saturating_sub(1),
        );
    }
}

#[derive(Debug, Clone)]
pub enum CommandOrEditor {
    Command(Command),
    Editor(EditorTarget, Value),
    Clipboard(String),
    FetchSettings,
}
impl From<Command> for CommandOrEditor {
    fn from(value: Command) -> Self {
        Self::Command(value)
    }
}

fn nonempty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
fn pretty(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "<cannot display>".into())
}
fn redact_secrets(mut value: Value) -> Value {
    fn visit(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if ["apikey", "api_key", "password", "secret", "token"]
                        .iter()
                        .any(|needle| key.to_ascii_lowercase().contains(needle))
                    {
                        *child = Value::String("[REDACTED]".into());
                    } else {
                        visit(child);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    visit(item);
                }
            }
            _ => {}
        }
    }
    visit(&mut value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_permission_presets_apply_expected_actions() {
        let mut form = KeyFormState::default();
        assert_eq!(form.actions, vec!["*"]);

        form.choose_preset(1);
        assert_eq!(
            form.actions,
            vec![
                "indexes.get",
                "tasks.get",
                "stats.get",
                "metrics.get",
                "version"
            ]
        );

        form.choose_preset(2);
        assert_eq!(
            form.actions,
            vec![
                "search",
                "documents.get",
                "indexes.get",
                "tasks.get",
                "settings.get",
                "stats.get",
                "metrics.get",
                "version"
            ]
        );

        form.choose_preset(3);
        assert_eq!(form.actions, vec!["search"]);
    }

    #[test]
    fn custom_preset_preserves_selected_actions() {
        let mut form = KeyFormState {
            actions: vec!["documents.get".into()],
            ..KeyFormState::default()
        };
        form.choose_preset(CUSTOM_PRESET);

        assert_eq!(form.actions, vec!["documents.get"]);
    }
}
