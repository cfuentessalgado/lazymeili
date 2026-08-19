pub mod components;
pub mod screens;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{App, Overlay, Route};

pub const ACCENT: Color = Color::Cyan;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    draw_tabs(frame, app, areas[0]);
    draw_screen(frame, app, areas[1]);
    draw_status(frame, app, areas[2]);
    if let Some(overlay) = &app.overlay {
        draw_overlay(frame, overlay);
    }
}

fn draw_tabs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let selected = Route::ALL
        .iter()
        .position(|route| *route == app.route)
        .unwrap_or(0);
    let tabs = Tabs::new(Route::ALL.iter().map(|route| route.title()))
        .select(selected)
        .block(Block::default().borders(Borders::ALL).title(" mtui "))
        .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .divider("│");
    frame.render_widget(tabs, area);
}

fn draw_screen(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.loading {
        frame.render_widget(
            Paragraph::new("Loading…")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }
    match app.route {
        Route::Applications => components::table(
            frame,
            area,
            "Applications",
            &["Name", "URL", "Credential"],
            app.config
                .applications
                .iter()
                .map(|item| {
                    vec![
                        item.name.clone(),
                        item.url.clone(),
                        if item.has_api_key {
                            "keychain/vault".into()
                        } else {
                            "none".into()
                        },
                    ]
                })
                .collect(),
            app.selected,
        ),
        Route::Dashboard => {
            let rows = app
                .displayed_indexes()
                .into_iter()
                .map(|index| {
                    let stat = app
                        .stats
                        .as_ref()
                        .and_then(|stats| stats.indexes.get(&index.uid));
                    vec![
                        index.uid.clone(),
                        index.primary_key.clone().unwrap_or_else(|| "—".into()),
                        stat.map_or_else(|| "—".into(), |s| s.number_of_documents.to_string()),
                        stat.map_or_else(
                            || "—".into(),
                            |s| {
                                if s.is_indexing {
                                    "indexing".into()
                                } else {
                                    "ready".into()
                                }
                            },
                        ),
                    ]
                })
                .collect();
            let version = app
                .version
                .as_ref()
                .map_or("unknown", |version| version.pkg_version.as_str());
            let database_size = app.stats.as_ref().map_or(0, |stats| stats.database_size);
            let features = format!(
                "db:{database_size}B hybrid:{} threshold:{}",
                if app.capabilities.hybrid_search {
                    "yes"
                } else {
                    "no"
                },
                if app.capabilities.ranking_score_threshold {
                    "yes"
                } else {
                    "no"
                }
            );
            components::table(
                frame,
                area,
                &format!("Indices — Meilisearch {version} — {features}"),
                &["UID", "Primary key", "Documents", "State"],
                rows,
                app.selected,
            );
        }
        Route::Documents => {
            let rows = app
                .search
                .hits
                .iter()
                .map(|hit| {
                    let object = hit.as_object();
                    let first = object.and_then(|map| map.iter().next()).map_or_else(
                        || "—".into(),
                        |(key, value)| format!("{key}={}", compact(value)),
                    );
                    vec![first, compact(hit)]
                })
                .collect();
            components::table(
                frame,
                area,
                &format!(
                    "Documents — {} hits in {} ms",
                    app.search.estimated_total_hits, app.search.processing_time_ms
                ),
                &["First field", "JSON preview"],
                rows,
                app.selected,
            );
        }
        Route::Settings => frame.render_widget(
            Paragraph::new(serde_json::to_string_pretty(&app.settings).unwrap_or_default())
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Index settings JSON — e opens $VISUAL/$EDITOR "),
                ),
            area,
        ),
        Route::Tasks => components::table(
            frame,
            area,
            &format!(
                "Tasks — {} total",
                app.task_total.map_or_else(|| "?".into(), |v| v.to_string())
            ),
            &["UID", "Index", "Type", "Status", "Duration"],
            app.tasks
                .iter()
                .map(|task| {
                    vec![
                        task.uid.to_string(),
                        task.index_uid.clone().unwrap_or_else(|| "—".into()),
                        task.kind.clone(),
                        task.status.clone(),
                        task.duration.clone().unwrap_or_else(|| elapsed(task)),
                    ]
                })
                .collect(),
            app.selected,
        ),
        Route::Keys => components::table(
            frame,
            area,
            &format!(
                "API keys — {} total",
                app.key_total.map_or_else(|| "?".into(), |v| v.to_string())
            ),
            &[
                "Name",
                "UID (not credential)",
                "Indexes",
                "Actions",
                "Expires",
            ],
            app.keys
                .iter()
                .map(|key| {
                    vec![
                        key.name.clone().unwrap_or_default(),
                        key.uid.clone(),
                        key.indexes.join(","),
                        key.actions.join(","),
                        key.expires_at.clone().unwrap_or_else(|| "never".into()),
                    ]
                })
                .collect(),
            app.selected,
        ),
    }
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let screen_keys = match app.route {
        Route::Applications => {
            "↑/k ↓/j select  Enter connect  n new  e edit  d remove  r reconnect"
        }
        Route::Dashboard => {
            "↑/k ↓/j select  Enter documents  / filter  n new  e edit primary key  d delete  PgUp/PgDn page"
        }
        Route::Documents => {
            "↑/k ↓/j select  Enter JSON  / search  n upload  e edit  d delete  r refresh"
        }
        Route::Settings => "Enter view JSON  e edit  r refresh",
        Route::Tasks => {
            "↑/k ↓/j select  Enter details  / filter  d cancel  r refresh  PgUp/PgDn page"
        }
        Route::Keys => {
            "↑/k ↓/j select  Enter details  n new  e edit  d delete  y yank new key  r refresh"
        }
    };
    let global_keys = "a applications  h/l or Tab/Shift-Tab screens  s settings  t tasks  K keys  D dump  ? help  q quit";
    let notice = app.notice.as_deref().unwrap_or("");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(screen_keys),
            Line::from(global_keys),
            Line::from(notice),
        ])
        .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn draw_overlay(frame: &mut Frame<'_>, overlay: &Overlay) {
    let area = centered(75, 75, frame.area());
    frame.render_widget(Clear, area);
    match overlay {
        Overlay::KeyForm(form) => draw_key_form(frame, area, form),
        Overlay::Help => {
            let items = [
                "Arrow keys or h/j/k/l  Navigate",
                "Tab / Shift-Tab       Change screen",
                "Enter                  Open or confirm",
                "n / e / d              New, edit, delete/cancel",
                "y                      Yank a newly created key",
                "/                      Search or filter",
                "r / PageUp/PageDown     Refresh / page",
                "a                      Select application",
                "s / t / K / D          Settings / tasks / keys / dump",
                "Esc                    Close",
                "q / Ctrl-C             Quit",
            ];
            frame.render_widget(
                List::new(items.map(ListItem::new)).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Help — Esc to close "),
                ),
                area,
            );
        }
        Overlay::Message { title, body } | Overlay::Confirm { title, body } => frame.render_widget(
            Paragraph::new(body.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title(title.as_str())),
            area,
        ),
        Overlay::Input {
            title,
            value,
            secret,
        } => {
            let shown = if *secret {
                "•".repeat(value.chars().count())
            } else {
                value.clone()
            };
            frame.render_widget(
                Paragraph::new(shown).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {title} — Enter to save ")),
                ),
                centered(75, 20, frame.area()),
            );
        }
    }
}

fn draw_key_form(frame: &mut Frame<'_>, area: Rect, form: &crate::app::KeyFormState) {
    if form.picking_actions {
        draw_action_picker(frame, area, form);
        return;
    }
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Create API key — Tab/↑/↓ fields, Enter next/submit, Esc cancel ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(inner);
    let selected_actions = if form.actions.is_empty() {
        "None".into()
    } else {
        form.actions.join(", ")
    };
    let fields = [
        (
            "Permission preset (←/→ changes preset)",
            crate::app::KEY_PRESETS[form.preset_choice],
        ),
        ("UID (identifier only — NOT the API key)", form.uid.as_str()),
        ("Name", form.name.as_str()),
        ("Description", form.description.as_str()),
        ("Indexes (comma-separated or *)", form.indexes.as_str()),
        ("Actions (Enter opens selector)", selected_actions.as_str()),
        (
            "Expiry (←/→ changes preset)",
            crate::app::EXPIRY_PRESETS[form.expiry_choice],
        ),
    ];
    for (index, (label, value)) in fields.into_iter().enumerate() {
        let style = if index == form.focus {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(value).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .title(format!(" {label} ")),
            ),
            rows[index],
        );
    }
    frame.render_widget(
        Paragraph::new(
            "The UUID cannot authenticate. The secret API key appears once after creation; press y there.",
        )
        .style(Style::default().fg(Color::DarkGray)),
        rows[7],
    );
}

fn draw_action_picker(frame: &mut Frame<'_>, area: Rect, form: &crate::app::KeyFormState) {
    let areas = Layout::vertical([Constraint::Min(5), Constraint::Length(2)]).split(area);
    let visible = usize::from(areas[0].height.saturating_sub(2)).max(1);
    let start = form
        .action_cursor
        .saturating_sub(visible / 2)
        .min(crate::app::API_KEY_ACTIONS.len().saturating_sub(visible));
    let items = crate::app::API_KEY_ACTIONS
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, action)| {
            let mark = if form.actions.iter().any(|selected| selected == action) {
                "[x]"
            } else {
                "[ ]"
            };
            let style = if index == form.action_cursor {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{mark} {action}")).style(style)
        });
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Select actions — ↑/↓ move, Space toggle, Enter confirm, Esc back "),
        ),
        areas[0],
    );
    let current = crate::app::API_KEY_ACTIONS[form.action_cursor];
    frame.render_widget(
        Paragraph::new(format!("{current}: {}", action_help(current)))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::DarkGray)),
        areas[1],
    );
}

fn action_help(action: &str) -> &'static str {
    if action == "*" {
        return "all permissions; avoid this for production keys";
    }
    if action == "search" {
        return "run search, multi-search, facet search, and similar-document queries";
    }
    if action.starts_with("documents") {
        return "read, add, update, or delete index documents";
    }
    if action.starts_with("indexes") {
        return "inspect or change indexes and their primary keys";
    }
    if action.starts_with("tasks") {
        return "inspect, cancel, delete, or compact asynchronous tasks";
    }
    if action.starts_with("settings") {
        return "inspect or change index search settings";
    }
    if action.starts_with("keys") {
        return "inspect or manage API keys; use with care";
    }
    if action.starts_with("dumps") || action.starts_with("snapshots") {
        return "create server-side backups";
    }
    if action.starts_with("stats") || action.starts_with("metrics") {
        return "read instance statistics or metrics";
    }
    if action.starts_with("chats") || action == "chatCompletions" {
        return "use or manage chat workspaces";
    }
    if action.starts_with("webhooks") {
        return "inspect or manage task webhooks";
    }
    if action.starts_with("network") {
        return "inspect or change network topology";
    }
    if action.starts_with("dynamicSearchRules") {
        return "inspect or manage dynamic search rules";
    }
    "grant access to the named Meilisearch API capability"
}

fn compact(value: &serde_json::Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 90 {
        format!("{}…", text.chars().take(89).collect::<String>())
    } else {
        text
    }
}
fn elapsed(task: &crate::meili::Task) -> String {
    if matches!(task.status.as_str(), "enqueued" | "processing") {
        task.started_at
            .as_ref()
            .or(task.enqueued_at.as_ref())
            .map_or_else(|| "running".into(), |time| format!("since {time}"))
    } else {
        "—".into()
    }
}
fn centered(x: u16, y: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - y) / 2),
        Constraint::Percentage(y),
        Constraint::Percentage((100 - y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - x) / 2),
        Constraint::Percentage(x),
        Constraint::Percentage((100 - x) / 2),
    ])
    .split(v[1])[1]
}
