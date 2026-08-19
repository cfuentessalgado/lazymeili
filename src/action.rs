use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Help,
    Escape,
    Confirm,
    Next,
    Previous,
    Left,
    Right,
    Refresh,
    Create,
    Edit,
    Delete,
    Search,
    Yank,
    Applications,
    Settings,
    Tasks,
    Keys,
    Dump,
    PageNext,
    PagePrevious,
    Input(char),
    Backspace,
    None,
}

#[must_use]
pub fn map_key(key: KeyEvent, editing: bool) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    if editing {
        return match key.code {
            KeyCode::Esc => Action::Escape,
            KeyCode::Enter => Action::Confirm,
            KeyCode::Down => Action::Next,
            KeyCode::Up => Action::Previous,
            KeyCode::Tab | KeyCode::Right => Action::Right,
            KeyCode::BackTab | KeyCode::Left => Action::Left,
            KeyCode::Backspace => Action::Backspace,
            KeyCode::Char(c) => Action::Input(c),
            _ => Action::None,
        };
    }
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::Help,
        KeyCode::Esc => Action::Escape,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Down | KeyCode::Char('j') => Action::Next,
        KeyCode::Up | KeyCode::Char('k') => Action::Previous,
        KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => Action::Left,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => Action::Right,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('n') => Action::Create,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('y') => Action::Yank,
        KeyCode::Char('a') => Action::Applications,
        KeyCode::Char('s') => Action::Settings,
        KeyCode::Char('t') => Action::Tasks,
        KeyCode::Char('K') => Action::Keys,
        KeyCode::Char('D') => Action::Dump,
        KeyCode::PageDown => Action::PageNext,
        KeyCode::PageUp => Action::PagePrevious,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vim_navigation_is_mapped() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), false),
            Action::Next
        );
    }

    #[test]
    fn text_mode_keeps_command_characters() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), true),
            Action::Input('q')
        );
    }

    #[test]
    fn y_yanks_and_a_selects_applications() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE), false),
            Action::Yank
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), false),
            Action::Applications
        );
    }
}
