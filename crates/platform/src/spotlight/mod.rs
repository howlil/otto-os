use std::collections::{HashMap, HashSet};
use std::slice;

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::SystemServices::{SFGAO_FLAGS, SFGAO_FOLDER, SFGAO_HIDDEN};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, IShellItem2, SHCreateItemFromParsingName,
    SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::{GUID, Interface, PWSTR, w};

use crate::com::ComGuard;
use otto_core::{
    AppIdentity, AppleIconKind, DockEntry, DockItemKind, FocusCatalogApp, compact_app_id,
    is_non_trackable_focus_app, normalize_app_key,
};

const MAX_SPOTLIGHT_APPS: usize = 2_048;
const MAX_SHELL_NAME_UNITS: usize = 1_024;
const PKEY_LINK_TARGET_PARSING_PATH: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xb9b4b3fc_2b51_4a42_b5d8_324146afcf25),
    pid: 2,
};

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

fn shell_target(item: &IShellItem) -> Option<String> {
    let item: IShellItem2 = item.cast().ok()?;
    let raw = unsafe { item.GetString(&PKEY_LINK_TARGET_PARSING_PATH) }.ok()?;
    CoTaskString::read(raw).map(|value| value.trim().to_owned())
}

fn resolved_executable<'a>(identity: &'a str, target: Option<&'a str>) -> Option<&'a str> {
    target
        .filter(|value| value.to_ascii_lowercase().ends_with(".exe"))
        .or_else(|| {
            identity
                .to_ascii_lowercase()
                .ends_with(".exe")
                .then_some(identity)
        })
}

fn catalog_key(identity: &str, title: &str, target: Option<&str>) -> String {
    if let Some(path) = resolved_executable(identity, target) {
        return AppIdentity::executable(path).key();
    }
    if identity.contains('!') {
        return AppIdentity::user_model_id(identity).key();
    }
    normalize_app_key(title)
}

fn focus_block_key(identity: &str, title: &str, target: Option<&str>) -> String {
    if let Some(path) = resolved_executable(identity, target) {
        return normalize_app_key(path);
    }
    normalize_app_key(title)
}

#[cfg_attr(test, derive(Clone))]
struct ShellAppRecord {
    title: String,
    identity: String,
    target: Option<String>,
}

fn enumerate_shell_apps() -> Vec<ShellAppRecord> {
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
    let mut records = Vec::new();
    let mut identities = HashSet::new();
    let mut names = HashSet::new();
    while records.len() < MAX_SPOTLIGHT_APPS {
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
        let target = shell_target(&item);
        records.push(ShellAppRecord {
            title,
            identity,
            target,
        });
    }
    records
}

fn is_explorer_executable(path: &str) -> bool {
    let trimmed = path.trim();
    let file_name = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
    file_name.eq_ignore_ascii_case("explorer.exe")
}

fn is_file_explorer_record(record: &ShellAppRecord) -> bool {
    let lower_id = record.identity.trim().to_ascii_lowercase();
    if lower_id.starts_with("microsoft.windows.explorer")
        || is_explorer_executable(&record.identity)
        || record.target.as_deref().is_some_and(is_explorer_executable)
    {
        return true;
    }
    let title = record.title.trim();
    record.target.is_none()
        && (title.eq_ignore_ascii_case("file explorer")
            || title.eq_ignore_ascii_case("windows explorer"))
}

fn finder_spotlight_entry() -> DockEntry {
    DockEntry {
        key: String::from("finder"),
        title: String::from("Finder"),
        kind: DockItemKind::Finder,
        apple_icon: Some(AppleIconKind::Finder),
        app: None,
        hwnds: Vec::new(),
        window_titles: Vec::new(),
        running: false,
        active: false,
        pinned: false,
        hidden: false,
        minimized: false,
    }
}

fn build_spotlight_apps(records: Vec<ShellAppRecord>, finder_enabled: bool) -> Vec<DockEntry> {
    let mut entries: Vec<DockEntry> = records
        .into_iter()
        .filter(|record| !finder_enabled || !is_file_explorer_record(record))
        .filter_map(|record| {
            let key = catalog_key(&record.identity, &record.title, record.target.as_deref());
            if key.is_empty() || (finder_enabled && key == "finder") {
                return None;
            }
            let launch_path = format!("shell:AppsFolder\\{}", record.identity);
            Some(DockEntry {
                key,
                title: record.title,
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
            })
        })
        .collect();
    if finder_enabled {
        entries.push(finder_spotlight_entry());
    }
    entries.sort_by_cached_key(|entry| entry.title.to_lowercase());
    entries
}

#[must_use]
pub fn collect_spotlight_apps() -> Vec<DockEntry> {
    collect_spotlight_apps_with_options(false)
}

#[must_use]
pub fn collect_spotlight_apps_with_options(finder_enabled: bool) -> Vec<DockEntry> {
    build_spotlight_apps(enumerate_shell_apps(), finder_enabled)
}

fn is_user_packaged_aumid(identity: &str) -> bool {
    let trimmed = identity.trim();
    if trimmed.starts_with('{') {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("microsoft.autogenerated.")
        || lower.starts_with("microsoft.windows.explorer")
        || lower.starts_with("microsoft.windows.controlpanel")
        || lower.starts_with("microsoft.windows.shellexperiencehost")
        || lower.starts_with("microsoft.windows.startmenuexperiencehost")
        || lower.starts_with("microsoft.windows.search")
        || lower.starts_with("microsoft.sechealthui")
        || lower.starts_with("microsoftwindows.client.coreai")
        || lower.starts_with("windows.immersivecontrolpanel")
        || lower.starts_with("microsoft.aad.brokerplugin")
        || lower.starts_with("microsoft.accountscontrol")
        || lower.starts_with("microsoft.asynctextservice")
        || lower.starts_with("microsoft.bioenrollment")
        || lower.starts_with("microsoft.creddialoghost")
        || lower.starts_with("microsoft.ecapp")
        || lower.starts_with("microsoft.lockapp")
        || lower.starts_with("microsoft.win32webviewhost")
        || lower.starts_with("microsoft.xboxgamecallableui")
    {
        return false;
    }
    let Some((package_family, app_id)) = lower.split_once('!') else {
        return false;
    };
    if app_id.trim().is_empty() {
        return false;
    }
    let Some((package_name, publisher_hash)) = package_family.rsplit_once('_') else {
        return false;
    };
    !package_name.trim().is_empty()
        && (10..=16).contains(&publisher_hash.len())
        && publisher_hash
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn is_eligible_focus_catalog_record(record: &ShellAppRecord) -> bool {
    if is_non_trackable_focus_app(&record.identity, &record.title) {
        return false;
    }
    if let Some(target) = record.target.as_deref()
        && is_non_trackable_focus_app(target, &record.title)
    {
        return false;
    }
    if let Some(exe_path) = resolved_executable(&record.identity, record.target.as_deref()) {
        if is_non_trackable_focus_app(exe_path, &record.title) {
            return false;
        }
        if (exe_path.contains('\\') || exe_path.contains('/'))
            && !exe_path.starts_with('{')
            && !std::path::Path::new(exe_path).is_file()
        {
            return false;
        }
        return true;
    }
    record.target.is_none() && is_user_packaged_aumid(&record.identity)
}

#[must_use]
pub fn collect_focus_catalog_apps() -> Vec<FocusCatalogApp> {
    let mut by_compact: HashMap<String, FocusCatalogApp> = HashMap::new();
    for record in enumerate_shell_apps() {
        if !is_eligible_focus_catalog_record(&record) {
            continue;
        }
        let block_key = focus_block_key(&record.identity, &record.title, record.target.as_deref());
        let compact = compact_app_id(&block_key);
        if block_key.is_empty() || compact.is_empty() {
            continue;
        }
        let detail = resolved_executable(&record.identity, record.target.as_deref())
            .map(str::to_owned)
            .unwrap_or_default();
        let icon_source = format!("shell:AppsFolder\\{}", record.identity);
        let candidate = FocusCatalogApp {
            block_key,
            display_name: record.title,
            detail,
            icon_source,
        };
        by_compact
            .entry(compact)
            .and_modify(|existing| {
                if existing.detail.is_empty() && !candidate.detail.is_empty() {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    let mut entries: Vec<FocusCatalogApp> = by_compact.into_values().collect();
    entries.sort_by_cached_key(|entry| entry.display_name.to_lowercase());
    entries
}

#[cfg(test)]
mod tests {
    use otto_core::{AppleIconKind, DockItemKind};

    use super::{
        ShellAppRecord, build_spotlight_apps, catalog_key, focus_block_key,
        is_eligible_focus_catalog_record, is_file_explorer_record, is_user_packaged_aumid,
    };

    #[test]
    fn catalog_keys_match_runtime_executable_and_packaged_identities() {
        assert_eq!(
            catalog_key(
                "Visual Studio Code",
                "Visual Studio Code",
                Some("C:\\Apps\\Code.exe")
            ),
            "c:\\apps\\code.exe"
        );
        assert_eq!(
            catalog_key(
                "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
                "Calculator",
                None
            ),
            "aumid:Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
        assert_eq!(
            catalog_key("opaque", "Portable Tool", None),
            "portable tool"
        );
    }

    #[test]
    fn focus_block_keys_map_executables_and_never_store_raw_aumids() {
        assert_eq!(
            focus_block_key(
                "Visual Studio Code",
                "Visual Studio Code",
                Some("C:\\Apps\\Code.exe")
            ),
            "code"
        );
        let packaged = focus_block_key(
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            "Calculator",
            None,
        );
        assert_eq!(packaged, "calculator");
        assert!(!packaged.contains('!'));
        assert!(!packaged.starts_with("aumid:"));
    }

    #[test]
    fn focus_catalog_eligibility_keeps_user_apps_and_rejects_system_folders_and_docs() {
        assert!(is_user_packaged_aumid("Claude_pzs8sxrjxfjjc!Claude"));
        assert!(is_user_packaged_aumid(
            "AppleInc.AppleMusicWin_nzyj5cx40ttqa!App"
        ));
        assert!(!is_user_packaged_aumid(
            "Microsoft.AutoGenerated.{923DD477-5846-686B-A659-0FCCD73851A8}"
        ));
        assert!(!is_user_packaged_aumid(
            "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\appverif.exe"
        ));
        assert!(!is_user_packaged_aumid(
            "Microsoft.SecHealthUI_8wekyb3d8bbwe!SecHealthUI"
        ));

        let packaged_claude = ShellAppRecord {
            title: String::from("Claude"),
            identity: String::from("Claude_pzs8sxrjxfjjc!Claude"),
            target: None,
        };
        assert!(is_eligible_focus_catalog_record(&packaged_claude));

        let folder_shortcut = ShellAppRecord {
            title: String::from("career"),
            identity: String::from("C:\\Users\\demo\\career"),
            target: None,
        };
        assert!(!is_eligible_focus_catalog_record(&folder_shortcut));

        let chm_help = ShellAppRecord {
            title: String::from("WinRAR help"),
            identity: String::from("Microsoft.AutoGenerated.{1234}"),
            target: Some(String::from("C:\\Program Files\\WinRAR\\WinRAR.chm")),
        };
        assert!(!is_eligible_focus_catalog_record(&chm_help));

        let system_tool = ShellAppRecord {
            title: String::from("Application Verifier (X64)"),
            identity: String::from("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\appverif.exe"),
            target: Some(String::from("C:\\Windows\\System32\\appverif.exe")),
        };
        assert!(!is_eligible_focus_catalog_record(&system_tool));
    }

    #[test]
    fn spotlight_hides_file_explorer_and_exposes_finder_when_finder_is_enabled() {
        let records = vec![
            ShellAppRecord {
                title: String::from("File Explorer"),
                identity: String::from("Microsoft.Windows.Explorer"),
                target: None,
            },
            ShellAppRecord {
                title: String::from("Explorer Shortcut"),
                identity: String::from("Microsoft.AutoGenerated.{1111}"),
                target: Some(String::from("C:\\Windows\\explorer.exe")),
            },
            ShellAppRecord {
                title: String::from("Notepad"),
                identity: String::from("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"),
                target: None,
            },
        ];
        assert!(is_file_explorer_record(&records[0]));
        assert!(is_file_explorer_record(&records[1]));
        assert!(!is_file_explorer_record(&records[2]));

        let with_finder = build_spotlight_apps(records.clone(), true);
        let titles: Vec<&str> = with_finder.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["Finder", "Notepad"]);
        assert!(matches!(with_finder[0].kind, DockItemKind::Finder));
        assert_eq!(with_finder[0].apple_icon, Some(AppleIconKind::Finder));

        let without_finder = build_spotlight_apps(records, false);
        let legacy_titles: Vec<&str> = without_finder.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            legacy_titles,
            vec!["Explorer Shortcut", "File Explorer", "Notepad"]
        );
    }
}
