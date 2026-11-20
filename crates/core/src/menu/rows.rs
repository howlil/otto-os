use crate::app_model::Capability;

use super::commands::CMD_NONE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuRow {
    Item {
        id: usize,
        label: String,
        shortcut: String,
        checked: bool,
        enabled: bool,
    },
    Slider {
        id: usize,
        label: String,
        value: u8,
    },
    Calendar {
        year: u16,
        month: u8,
        today: Option<u8>,
    },
    Notification {
        id: usize,
        app_name: String,
        title: String,
        body: String,
        timestamp_label: String,
    },
    Separator,
}

impl MenuRow {
    #[must_use]
    pub fn item(id: usize, label: impl Into<String>, shortcut: impl Into<String>) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: shortcut.into(),
            checked: false,
            enabled: true,
        }
    }

    #[must_use]
    pub fn item_with_enabled(
        id: usize,
        label: impl Into<String>,
        shortcut: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: shortcut.into(),
            checked: false,
            enabled,
        }
    }

    #[must_use]
    pub fn check_item(id: usize, label: impl Into<String>, checked: bool, enabled: bool) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: String::new(),
            checked,
            enabled,
        }
    }

    #[must_use]
    pub fn heading(label: impl Into<String>) -> Self {
        Self::item_with_enabled(CMD_NONE, label, "", false)
    }
}

pub(super) fn push_capable(
    rows: &mut Vec<MenuRow>,
    capability: Capability,
    id: usize,
    label: impl Into<String>,
    shortcut: &str,
) {
    match capability {
        Capability::Available => rows.push(MenuRow::item(id, label, shortcut)),
        Capability::Unsupported => {}
        Capability::TemporarilyUnavailable | Capability::Denied => {
            rows.push(MenuRow::item_with_enabled(id, label, shortcut, false));
        }
    }
}

pub(super) fn trim_separators(mut rows: Vec<MenuRow>) -> Vec<MenuRow> {
    rows.dedup_by(|next, previous| {
        matches!(next, MenuRow::Separator) && matches!(previous, MenuRow::Separator)
    });
    while matches!(rows.first(), Some(MenuRow::Separator)) {
        rows.remove(0);
    }
    while matches!(rows.last(), Some(MenuRow::Separator)) {
        rows.pop();
    }
    rows
}
