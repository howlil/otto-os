use crate::menu::{CMD_OPEN_LOGS, CMD_OTTO_SETTINGS, CMD_WIN_MINIMIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMenuEffect {
    GenieMinimize,
    OpenOttoSettings,
    OpenLogs,
    Native(usize),
}

#[must_use]
pub fn window_menu_effect(cmd: usize, minimize_available: bool) -> WindowMenuEffect {
    match cmd {
        CMD_OTTO_SETTINGS => WindowMenuEffect::OpenOttoSettings,
        CMD_OPEN_LOGS => WindowMenuEffect::OpenLogs,
        CMD_WIN_MINIMIZE if minimize_available => WindowMenuEffect::GenieMinimize,
        _ => WindowMenuEffect::Native(cmd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::CMD_WIN_CLOSE;

    #[test]
    fn minimize_uses_genie_only_when_the_window_allows_it() {
        assert_eq!(
            window_menu_effect(CMD_WIN_MINIMIZE, true),
            WindowMenuEffect::GenieMinimize
        );
        assert_eq!(
            window_menu_effect(CMD_WIN_MINIMIZE, false),
            WindowMenuEffect::Native(CMD_WIN_MINIMIZE)
        );
        assert_eq!(
            window_menu_effect(CMD_WIN_CLOSE, true),
            WindowMenuEffect::Native(CMD_WIN_CLOSE)
        );
    }
}
