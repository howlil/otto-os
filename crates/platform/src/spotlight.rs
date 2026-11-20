use std::collections::HashSet;
use std::slice;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::SystemServices::{SFGAO_FLAGS, SFGAO_FOLDER, SFGAO_HIDDEN};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::{PWSTR, w};

use crate::com::ComGuard;
use crate::dock::{DockEntry, DockItemKind};

const MAX_SPOTLIGHT_APPS: usize = 2_048;
const MAX_SHELL_NAME_UNITS: usize = 1_024;

struct CoTaskString(PWSTR);

impl CoTaskString {
    fn read(raw: PWSTR) -> Option<String> {
        let owned = Self(raw);
        if owned.0.is_null() {
            return None;
        }
        let mut length = 0;
        while length < MAX_SHELL_NAME_UNITS {
            if unsafe { *owned.0.0.add(length) } == 0 {
                break;
            }
            length += 1;
        }
        if length == 0 || length == MAX_SHELL_NAME_UNITS {
            return None;
        }
        let value = unsafe { slice::from_raw_parts(owned.0.0, length) };
        Some(String::from_utf16_lossy(value))
    }
}

impl Drop for CoTaskString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.0.0.cast_const().cast()));
            }
        }
    }
}

#[must_use]
pub fn collect_spotlight_apps() -> Vec<DockEntry> {
    let _com = ComGuard::single_threaded();
    let Ok(folder) =
        (unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:AppsFolder"), None) })
    else {
        return Vec::new();
    };
    let Ok(items) = (unsafe { folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) })
    else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut identities = HashSet::new();
    let mut names = HashSet::new();
    while entries.len() < MAX_SPOTLIGHT_APPS {
        let mut next = [None];
        if unsafe { items.Next(&mut next, None) }.is_err() {
            break;
        }
        let Some(item) = next[0].take() else {
            break;
        };
        let attributes =
            unsafe { item.GetAttributes(SFGAO_FLAGS(SFGAO_FOLDER.0 | SFGAO_HIDDEN.0)) }
                .unwrap_or_default();
        if attributes.contains(SFGAO_FOLDER) || attributes.contains(SFGAO_HIDDEN) {
            continue;
        }
        let Ok(raw_title) = (unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }) else {
            continue;
        };
        let Some(title) = CoTaskString::read(raw_title).map(|name| name.trim().to_owned()) else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let Ok(raw_identity) = (unsafe { item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING) }) else {
            continue;
        };
        let Some(identity) = CoTaskString::read(raw_identity).map(|value| value.trim().to_owned())
        else {
            continue;
        };
        if identity.is_empty() || !identities.insert(identity.to_lowercase()) {
            continue;
        }
        if !names.insert(title.to_lowercase()) {
            continue;
        }
        let launch_path = format!("shell:AppsFolder\\{identity}");
        entries.push(DockEntry {
            key: format!("aumid:{identity}"),
            title,
            kind: DockItemKind::App {
                exe_path: launch_path,
            },
            apple_icon: None,
            app: None,
            hwnds: Vec::new(),
            window_titles: Vec::new(),
            running: false,
            active: false,
            pinned: false,
            hidden: false,
            minimized: false,
        });
    }
    entries.sort_by_cached_key(|entry| entry.title.to_lowercase());
    entries
}
