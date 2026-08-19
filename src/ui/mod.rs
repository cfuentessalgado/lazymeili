pub mod components;
pub mod screens;
pub mod theme;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{App, Overlay, Route};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let accent = active_accent(app);
    frame.render_widget(Block::default().style(theme::base()), frame.area());
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    draw_tabs(frame, app, areas[0], accent);
    draw_screen(frame, app, areas[1], accent);
    draw_status(frame, app, areas[2], accent);
    if let Some(overlay) = &app.overlay {
        draw_overlay(frame, overlay, accent);
    }
}

fn draw_tabs(frame: &mut Frame<'_>, app: &App, area: Rect, accent: ratatui::style::Color) {
    let selected = Route::ALL
        .iter()
        .position(|route| *route == app.route)
        .unwrap_or(0);
    let tabs = Tabs::new(Route::ALL.iter().map(|route| route.title()))
        .select(selected)
        .style(Style::new().fg(theme::MUTED).bg(theme::SURFACE))
        .block(
            panel(
                Line::from(vec![
                    Span::styled(" LazyMeili ", theme::title(accent)),
                    Span::styled("• Meilisearch control ", theme::muted().bg(theme::SURFACE)),
                ]),
                accent,
            )
            .title(connection_indicator(app)),
        )
        .highlight_style(theme::selected(accent))
        .divider(Span::styled(" │ ", Style::new().fg(theme::BORDER)));
    frame.render_widget(tabs, area);
}

fn draw_screen(frame: &mut Frame<'_>, app: &App, area: Rect, accent: ratatui::style::Color) {
    if app.loading {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("◆ ", Style::new().fg(accent)),
                Span::styled("Loading…", Style::new().fg(theme::TEXT)),
            ]))
            .style(theme::panel())
            .alignment(Alignment::Center)
            .block(panel(" Working ", accent)),
            area,
        );
        return;
    }
    match app.route {
        Route::Applications => components::table(
            frame,
            area,
            "Applications",
            &["Name", "URL", "Color", "Credential", "Connection"],
            app.config
                .applications
                .iter()
                .map(|item| {
                    let is_active = app.active == Some(item.id);
                    let connection = if !is_active {
                        "saved"
                    } else if app.service.is_some() {
                        "connected"
                    } else if app.loading {
                        "connecting"
                    } else {
                        "disconnected"
                    };
                    vec![
                        if is_active {
                            format!("● {}", item.name)
                        } else {
                            item.name.clone()
                        },
                        item.url.clone(),
                        item.color.label().into(),
                        if item.has_api_key {
                            "keychain/vault".into()
                        } else {
                            "none".into()
                        },
                        connection.into(),
                    ]
                })
                .collect(),
            app.selected,
            accent,
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
                accent,
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
                accent,
            );
        }
        Route::Settings => frame.render_widget(
            Paragraph::new(serde_json::to_string_pretty(&app.settings).unwrap_or_default())
                .wrap(Wrap { trim: false })
                .style(theme::panel())
                .block(panel(
                    " Index settings JSON — e opens $VISUAL/$EDITOR ",
                    accent,
                )),
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
            accent,
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
            accent,
        ),
    }
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect, accent: ratatui::style::Color) {
    let screen_keys: &[(&str, &str)] = match app.route {
        Route::Applications => &[
            ("↑/↓", "select"),
            ("Enter", "connect"),
            ("n", "new"),
            ("e", "edit"),
            ("c", "color"),
            ("d", "remove"),
            ("r", "reconnect"),
        ],
        Route::Dashboard => &[
            ("↑/↓", "select"),
            ("Enter", "documents"),
            ("/", "filter"),
            ("n", "new"),
            ("e", "primary key"),
            ("d", "delete"),
            ("Pg±", "page"),
        ],
        Route::Documents => &[
            ("↑/↓", "select"),
            ("Enter", "JSON"),
            ("/", "search"),
            ("n", "upload"),
            ("e", "edit"),
            ("d", "delete"),
            ("r", "refresh"),
        ],
        Route::Settings => &[("Enter", "view JSON"), ("e", "edit"), ("r", "refresh")],
        Route::Tasks => &[
            ("↑/↓", "select"),
            ("Enter", "details"),
            ("/", "filter"),
            ("d", "cancel"),
            ("r", "refresh"),
            ("Pg±", "page"),
        ],
        Route::Keys => &[
            ("↑/↓", "select"),
            ("Enter", "details"),
            ("n", "new"),
            ("e", "edit"),
            ("d", "delete"),
            ("y", "yank"),
            ("r", "refresh"),
        ],
    };
    let global_keys = &[
        ("Tab", "screens"),
        ("a", "apps"),
        ("s", "settings"),
        ("t", "tasks"),
        ("K", "keys"),
        ("D", "dump"),
        ("?", "help"),
        ("q", "quit"),
    ];
    let notice = app.notice.as_deref().unwrap_or("");
    let notice_style = if notice.to_ascii_lowercase().contains("error")
        || notice.to_ascii_lowercase().contains("failed")
    {
        Style::new().fg(theme::DANGER).bg(theme::BG)
    } else {
        Style::new().fg(theme::SUCCESS).bg(theme::BG)
    };
    frame.render_widget(
        Paragraph::new(vec![
            hint_line(screen_keys, accent),
            hint_line(global_keys, accent),
            Line::from(vec![
                Span::styled(" STATUS ", theme::key(accent)),
                Span::raw(" "),
                Span::styled(
                    if notice.is_empty() { "Ready" } else { notice },
                    notice_style,
                ),
            ]),
        ])
        .style(theme::muted()),
        area,
    );
}

fn draw_overlay(frame: &mut Frame<'_>, overlay: &Overlay, accent: ratatui::style::Color) {
    let area = centered(75, 75, frame.area());
    frame.render_widget(Clear, area);
    match overlay {
        Overlay::KeyForm(form) => draw_key_form(frame, area, form, accent),
        Overlay::ColorPicker { cursor, .. } => draw_color_picker(frame, area, *cursor, accent),
        Overlay::Help => {
            let items = [
                "Arrow keys or h/j/k/l  Navigate",
                "Tab / Shift-Tab       Change screen",
                "Enter                  Open or confirm",
                "n / e / d              New, edit, delete/cancel",
                "c                      Assign a connection color",
                "y                      Yank a newly created key",
                "/                      Search or filter",
                "r / PageUp/PageDown     Refresh / page",
                "a                      Select application",
                "s / t / K / D          Settings / tasks / keys / dump",
                "Esc                    Close",
                "q / Ctrl-C             Quit",
            ];
            frame.render_widget(
                List::new(items.map(ListItem::new))
                    .style(theme::panel())
                    .block(panel(" Help — Esc to close ", accent)),
                area,
            );
        }
        Overlay::Message { title, body } | Overlay::Confirm { title, body } => frame.render_widget(
            Paragraph::new(body.as_str())
                .style(theme::panel())
                .wrap(Wrap { trim: false })
                .block(panel(format!(" {title} "), accent)),
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
                Paragraph::new(shown)
                    .style(theme::panel())
                    .block(panel(format!(" {title} — Enter to save "), accent)),
                centered(75, 20, frame.area()),
            );
        }
    }
}

fn draw_color_picker(
    frame: &mut Frame<'_>,
    area: Rect,
    cursor: usize,
    accent: ratatui::style::Color,
) {
    let area = centered(55, 75, area);
    let items = crate::config::ConnectionColor::ALL
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let assigned = theme::connection_color(*color);
            let marker = if index == cursor { " ▸ " } else { "   " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::new().fg(assigned)),
                Span::styled("  ", Style::new().bg(assigned)),
                Span::styled(format!("  {}", color.label()), Style::new().fg(theme::TEXT)),
            ]))
            .style(if index == cursor {
                Style::new()
                    .bg(theme::SURFACE_ALT)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::panel()
            })
        });
    frame.render_widget(
        List::new(items).style(theme::panel()).block(panel(
            " Connection color — ↑/↓ choose, Enter save, Esc cancel ",
            accent,
        )),
        area,
    );
}

fn draw_key_form(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &crate::app::KeyFormState,
    accent: ratatui::style::Color,
) {
    if form.picking_actions {
        draw_action_picker(frame, area, form, accent);
        return;
    }
    let outer = panel(
        " Create API key — Tab/↑/↓ fields, Enter next/submit, Esc cancel ",
        accent,
    );
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
            Style::new()
                .fg(accent)
                .bg(theme::SURFACE)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::border()
        };
        frame.render_widget(
            Paragraph::new(value).style(theme::panel()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(style)
                    .title_style(style)
                    .title(format!(" {label} "))
                    .style(theme::panel()),
            ),
            rows[index],
        );
    }
    frame.render_widget(
        Paragraph::new(
            "The UUID cannot authenticate. The secret API key appears once after creation; press y there.",
        )
        .style(theme::muted().bg(theme::SURFACE)),
        rows[7],
    );
}

fn draw_action_picker(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &crate::app::KeyFormState,
    accent: ratatui::style::Color,
) {
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
                theme::selected(accent)
            } else if mark == "[x]" {
                Style::new().fg(theme::SUCCESS).bg(theme::SURFACE)
            } else {
                theme::panel()
            };
            ListItem::new(format!("{mark} {action}")).style(style)
        });
    frame.render_widget(
        List::new(items).style(theme::panel()).block(panel(
            " Select actions — ↑/↓ move, Space toggle, Enter confirm, Esc back ",
            accent,
        )),
        areas[0],
    );
    let current = crate::app::API_KEY_ACTIONS[form.action_cursor];
    frame.render_widget(
        Paragraph::new(format!("{current}: {}", action_help(current)))
            .wrap(Wrap { trim: true })
            .style(theme::muted()),
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
fn active_accent(app: &App) -> ratatui::style::Color {
    app.active
        .and_then(|active_id| {
            app.config
                .applications
                .iter()
                .find(|connection| connection.id == active_id)
        })
        .map_or(theme::ACCENT, |connection| {
            theme::connection_color(connection.color)
        })
}

fn connection_indicator(app: &App) -> Line<'static> {
    let Some(active_id) = app.active else {
        return Line::from(Span::styled(
            " ● NO CONNECTION ",
            Style::new()
                .fg(theme::BG)
                .bg(theme::DANGER)
                .add_modifier(Modifier::BOLD),
        ))
        .right_aligned();
    };
    let Some(connection) = app
        .config
        .applications
        .iter()
        .find(|connection| connection.id == active_id)
    else {
        return Line::from(Span::styled(
            " ● CONNECTION MISSING ",
            Style::new()
                .fg(theme::BG)
                .bg(theme::DANGER)
                .add_modifier(Modifier::BOLD),
        ))
        .right_aligned();
    };
    let (label, color) = if app.service.is_some() {
        ("CONNECTED", theme::SUCCESS)
    } else if app.loading {
        ("CONNECTING", theme::WARNING)
    } else {
        ("DISCONNECTED", theme::DANGER)
    };
    Line::from(vec![
        Span::styled(
            "  ",
            Style::new().bg(theme::connection_color(connection.color)),
        ),
        Span::styled(
            format!(" {} · {} ", connection.name, connection.url),
            Style::new().fg(theme::TEXT).bg(theme::SURFACE),
        ),
        Span::styled(
            format!(" ● {label} "),
            Style::new()
                .fg(theme::BG)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .right_aligned()
}

fn hint_line(hints: &[(&str, &str)], accent: ratatui::style::Color) -> Line<'static> {
    let mut spans = Vec::with_capacity(hints.len() * 3);
    for (key, label) in hints {
        spans.push(Span::styled(format!(" {key} "), theme::key(accent)));
        spans.push(Span::styled(format!(" {label} "), theme::muted()));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn panel<'a>(title: impl Into<Line<'a>>, accent: ratatui::style::Color) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(accent).bg(theme::SURFACE))
        .title_style(theme::title(accent))
        .title(title)
        .style(theme::panel())
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
