use ratatui::style::{Color, Modifier, Style};

use crate::config::ConnectionColor;

// Meilisearch-inspired dark palette. RGB colors keep the look consistent in
// modern terminals while preserving enough contrast for dense data screens.
pub const BG: Color = Color::Rgb(17, 18, 30);
pub const SURFACE: Color = Color::Rgb(25, 27, 44);
pub const SURFACE_ALT: Color = Color::Rgb(31, 34, 54);
pub const BORDER: Color = Color::Rgb(72, 76, 110);
pub const TEXT: Color = Color::Rgb(229, 231, 242);
pub const MUTED: Color = Color::Rgb(139, 144, 174);
pub const ACCENT: Color = Color::Rgb(161, 128, 255);
pub const SUCCESS: Color = Color::Rgb(79, 218, 143);
pub const WARNING: Color = Color::Rgb(247, 190, 82);
pub const DANGER: Color = Color::Rgb(255, 107, 120);

pub const fn connection_color(color: ConnectionColor) -> Color {
    match color {
        ConnectionColor::Violet => Color::Rgb(161, 128, 255),
        ConnectionColor::Blue => Color::Rgb(91, 156, 255),
        ConnectionColor::Cyan => Color::Rgb(65, 210, 225),
        ConnectionColor::Lime => Color::Rgb(145, 220, 90),
        ConnectionColor::Yellow => Color::Rgb(247, 210, 82),
        ConnectionColor::Orange => Color::Rgb(255, 153, 72),
        ConnectionColor::Red => Color::Rgb(255, 91, 105),
        ConnectionColor::Pink => Color::Rgb(255, 103, 174),
        ConnectionColor::Gray => Color::Rgb(139, 144, 174),
    }
}

pub const fn base() -> Style {
    Style::new().fg(TEXT).bg(BG)
}

pub const fn panel() -> Style {
    Style::new().fg(TEXT).bg(SURFACE)
}

pub const fn muted() -> Style {
    Style::new().fg(MUTED).bg(BG)
}

pub const fn border() -> Style {
    Style::new().fg(BORDER).bg(SURFACE)
}

pub const fn title(accent: Color) -> Style {
    Style::new()
        .fg(accent)
        .bg(SURFACE)
        .add_modifier(Modifier::BOLD)
}

pub const fn selected(accent: Color) -> Style {
    Style::new().fg(BG).bg(accent).add_modifier(Modifier::BOLD)
}

pub const fn key(accent: Color) -> Style {
    Style::new().fg(BG).bg(accent).add_modifier(Modifier::BOLD)
}

pub fn semantic(value: &str) -> Style {
    match value.to_ascii_lowercase().as_str() {
        "ready" | "succeeded" | "available" | "connected" | "yes" => Style::new().fg(SUCCESS),
        "connecting" | "indexing" | "processing" | "enqueued" | "running" | "unknown" => {
            Style::new().fg(WARNING)
        }
        "disconnected" | "failed" | "canceled" | "unavailable" | "none" | "no" => {
            Style::new().fg(DANGER)
        }
        "violet" => Style::new().fg(connection_color(ConnectionColor::Violet)),
        "blue" => Style::new().fg(connection_color(ConnectionColor::Blue)),
        "cyan" => Style::new().fg(connection_color(ConnectionColor::Cyan)),
        "lime" => Style::new().fg(connection_color(ConnectionColor::Lime)),
        "yellow" => Style::new().fg(connection_color(ConnectionColor::Yellow)),
        "orange" => Style::new().fg(connection_color(ConnectionColor::Orange)),
        "red" => Style::new().fg(connection_color(ConnectionColor::Red)),
        "pink" => Style::new().fg(connection_color(ConnectionColor::Pink)),
        "gray" => Style::new().fg(connection_color(ConnectionColor::Gray)),
        _ => Style::new().fg(TEXT),
    }
}
