use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read source dir")
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn crate_rs_files(crate_dir: &str) -> Vec<(String, String)> {
    let base = workspace_root().join("crates").join(crate_dir).join("src");
    let mut paths = Vec::new();
    collect_rs_files(&base, &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(&base)
                .expect("relative path")
                .to_string_lossy()
                .replace('\\', "/");
            let content = fs::read_to_string(&path).expect("read rust source");
            (rel, content)
        })
        .collect()
}

fn has_comment_outside_strings(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut line = 1;
    let mut in_string = false;
    let mut raw_hashes: Option<usize> = None;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if let Some(hashes) = raw_hashes {
            if b == b'"' {
                let mut matched = 0;
                while i + 1 + matched < bytes.len()
                    && matched < hashes
                    && bytes[i + 1 + matched] == b'#'
                {
                    matched += 1;
                }
                if matched == hashes {
                    raw_hashes = None;
                    i += 1 + hashes;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'r' || (b == b'b' && i + 1 < bytes.len() && bytes[i + 1] == b'r') {
            let start = if b == b'b' { i + 2 } else { i + 1 };
            let mut hashes = 0;
            while start + hashes < bytes.len() && bytes[start + hashes] == b'#' {
                hashes += 1;
            }
            if start + hashes < bytes.len() && bytes[start + hashes] == b'"' {
                raw_hashes = Some(hashes);
                i = start + hashes + 1;
                continue;
            }
        }
        if b == b'\'' {
            if i + 2 < bytes.len() && bytes[i + 1] != b'\\' && bytes[i + 2] == b'\'' {
                i += 3;
                continue;
            }
            if i + 3 < bytes.len() && bytes[i + 1] == b'\\' && bytes[i + 3] == b'\'' {
                i += 4;
                continue;
            }
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            return Some(line);
        }
        i += 1;
    }
    None
}

#[test]
fn f1_core_has_no_outward_or_win32_dependencies() {
    for (file, content) in crate_rs_files("core") {
        for forbidden in ["otto_platform", "otto_render", "otto_os", "windows::"] {
            assert!(
                !content.contains(forbidden),
                "core/{file} must not reference {forbidden}"
            );
        }
    }
}

#[test]
fn f2_notch_controller_is_isolated_from_siblings_and_adapters() {
    for (file, content) in crate_rs_files("app") {
        if !file.starts_with("notch/") {
            continue;
        }
        for forbidden in [
            "otto_platform",
            "otto_render",
            "windows::",
            "crate::dock",
            "crate::window_controls",
            "crate::control_center",
            "crate::spotlight",
            "crate::menu",
            "crate::genie",
            "crate::settings",
            "crate::settings_window",
        ] {
            assert!(
                !content.contains(forbidden),
                "app/{file} must not reference {forbidden}"
            );
        }
    }
}

#[test]
fn f3_app_has_only_one_host_owned_thread_local() {
    let mut holders = Vec::new();
    for (file, content) in crate_rs_files("app") {
        if file == "host/fitness_tests.rs" {
            continue;
        }
        if content.contains("thread_local!") {
            holders.push(file);
        }
    }
    assert_eq!(holders, vec![String::from("host/mod.rs")]);
}

#[test]
fn f4_render_never_imports_platform_or_app() {
    for (file, content) in crate_rs_files("render") {
        for forbidden in ["otto_platform", "otto_os"] {
            assert!(
                !content.contains(forbidden),
                "render/{file} must not reference {forbidden}"
            );
        }
    }
}

#[test]
fn app_window_message_constants_live_only_in_win_msg() {
    let mut holders = Vec::new();
    for (file, content) in crate_rs_files("app") {
        if file == "host/fitness_tests.rs" {
            continue;
        }
        if content.contains("const WM_") {
            holders.push(file);
        }
    }
    assert_eq!(holders, vec![String::from("host/win_msg.rs")]);
}

#[test]
fn crate_roots_contain_only_entry_file_and_module_folders() {
    for (crate_dir, entry) in [
        ("core", "lib.rs"),
        ("platform", "lib.rs"),
        ("render", "lib.rs"),
        ("app", "main.rs"),
    ] {
        for (file, _) in crate_rs_files(crate_dir) {
            if !file.contains('/') {
                assert_eq!(
                    file, entry,
                    "crates/{crate_dir}/src/{file} should be inside a module folder"
                );
            }
        }
    }
}

#[test]
fn workspace_rust_sources_remain_comment_free() {
    for crate_dir in ["core", "platform", "render", "app"] {
        for (file, content) in crate_rs_files(crate_dir) {
            if let Some(line) = has_comment_outside_strings(&content) {
                panic!("crates/{crate_dir}/src/{file}:{line} contains a comment");
            }
        }
    }
}

fn is_test_source(rel: &str) -> bool {
    rel == "tests.rs"
        || rel.ends_with("/tests.rs")
        || rel.ends_with("_tests.rs")
        || rel.starts_with("resource_tests/")
        || rel.contains("/resource_tests/")
}

fn prod_source(content: &str) -> String {
    let normalized = content.replace("\r\n", "\n");
    match normalized.find("#[cfg(test)]\nmod tests") {
        Some(index) => normalized[..index].to_owned(),
        None => normalized,
    }
}

fn count_identifier(source: &str, ident: &str) -> usize {
    let bytes = source.as_bytes();
    let needle = ident.as_bytes();
    let mut count = 0usize;
    let mut offset = 0usize;
    while let Some(pos) = source[offset..].find(ident) {
        let start = offset + pos;
        let end = start + needle.len();
        let left_ok =
            start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let right_ok =
            end == bytes.len() || !(bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
        if left_ok && right_ok {
            count += 1;
        }
        offset = end;
    }
    count
}

#[test]
fn f5_no_wildcard_reexports_in_crate_roots_or_mod_files() {
    for crate_dir in ["core", "platform", "render", "app"] {
        for (file, content) in crate_rs_files(crate_dir) {
            if !(file == "lib.rs" || file == "mod.rs" || file.ends_with("/mod.rs")) {
                continue;
            }
            let prod = prod_source(&content);
            for line in prod.lines() {
                let trimmed = line.trim();
                assert!(
                    !(trimmed.starts_with("pub use ") && trimmed.contains("::*")),
                    "crates/{crate_dir}/src/{file} contains wildcard re-export: {trimmed}"
                );
            }
        }
    }
}

#[test]
fn f6_render_visual_primitive_boundaries() {
    let mut text_format_callers = Vec::new();
    for (file, content) in crate_rs_files("render") {
        let prod = prod_source(&content);
        if prod.contains("CreateTextFormat") {
            text_format_callers.push(file.clone());
        }
        if !file.starts_with("notch/") && file != "lib.rs" {
            assert!(
                !prod.contains("crate::notch::"),
                "render/{file} must not couple to crate::notch::"
            );
        }
        if file.starts_with("focus_shield/") {
            assert!(
                !prod.contains(".CreateSolidColorBrush("),
                "render/{file} must use BrushPalette instead of CreateSolidColorBrush"
            );
        }
    }
    assert_eq!(text_format_callers, vec![String::from("lib.rs")]);
}

#[test]
fn f7_workspace_thread_locals_stay_bounded() {
    let mut holders = Vec::new();
    for crate_dir in ["core", "platform", "render", "app"] {
        for (file, content) in crate_rs_files(crate_dir) {
            if crate_dir == "app" && file == "host/fitness_tests.rs" {
                continue;
            }
            if prod_source(&content).contains("thread_local!") {
                holders.push(format!("{crate_dir}/{file}"));
            }
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec![
            String::from("app/host/mod.rs"),
            String::from("platform/appearance/backdrop.rs"),
            String::from("render/appearance/glass_gpu.rs"),
            String::from("render/lib.rs"),
        ]
    );
}

#[test]
fn f8_prod_allow_suppression_budget_does_not_grow() {
    const MAX_PROD_ALLOWS: usize = 85;
    const MAX_TOO_MANY_ARGUMENTS: usize = 34;
    const MAX_TOO_MANY_LINES: usize = 25;
    const MAX_STRUCT_EXCESSIVE_BOOLS: usize = 22;
    let mut total = 0usize;
    let mut too_many_arguments = 0usize;
    let mut too_many_lines = 0usize;
    let mut struct_excessive_bools = 0usize;
    for crate_dir in ["core", "platform", "render", "app"] {
        for (file, content) in crate_rs_files(crate_dir) {
            if is_test_source(&file) {
                continue;
            }
            let prod = prod_source(&content);
            total += prod.matches("#[allow(").count();
            too_many_arguments += prod.matches("clippy::too_many_arguments").count();
            too_many_lines += prod.matches("clippy::too_many_lines").count();
            struct_excessive_bools += prod.matches("clippy::struct_excessive_bools").count();
        }
    }
    assert!(
        total <= MAX_PROD_ALLOWS,
        "workspace prod #[allow( count {total} exceeds budget {MAX_PROD_ALLOWS}"
    );
    assert!(
        too_many_arguments <= MAX_TOO_MANY_ARGUMENTS,
        "prod clippy::too_many_arguments count {too_many_arguments} exceeds budget {MAX_TOO_MANY_ARGUMENTS}"
    );
    assert!(
        too_many_lines <= MAX_TOO_MANY_LINES,
        "prod clippy::too_many_lines count {too_many_lines} exceeds budget {MAX_TOO_MANY_LINES}"
    );
    assert!(
        struct_excessive_bools <= MAX_STRUCT_EXCESSIVE_BOOLS,
        "prod clippy::struct_excessive_bools count {struct_excessive_bools} exceeds budget {MAX_STRUCT_EXCESSIVE_BOOLS}"
    );
}

#[test]
fn f9_workspace_prod_unsafe_budgets_do_not_grow() {
    const MAX_CORE_UNSAFE: usize = 0;
    const MAX_APP_UNSAFE: usize = 28;
    const MAX_RENDER_UNSAFE: usize = 355;
    const MAX_PLATFORM_UNSAFE: usize = 661;
    const ALLOWED_APP_UNSAFE_FILES: &[&str] = &[
        "bar/mod.rs",
        "dock/mod.rs",
        "host/lifecycle.rs",
        "host/messages.rs",
        "host/mod.rs",
    ];
    for (crate_dir, max_budget) in [
        ("core", MAX_CORE_UNSAFE),
        ("app", MAX_APP_UNSAFE),
        ("render", MAX_RENDER_UNSAFE),
        ("platform", MAX_PLATFORM_UNSAFE),
    ] {
        let mut total = 0usize;
        for (file, content) in crate_rs_files(crate_dir) {
            if is_test_source(&file) {
                continue;
            }
            let count = count_identifier(&prod_source(&content), "unsafe");
            if crate_dir == "app" && count > 0 {
                assert!(
                    ALLOWED_APP_UNSAFE_FILES.contains(&file.as_str()),
                    "app/{file} must not introduce unsafe blocks (wrap Win32 calls in otto-platform)"
                );
            }
            total += count;
        }
        assert!(
            total <= max_budget,
            "{crate_dir} prod unsafe keyword count {total} exceeds budget {max_budget}"
        );
    }
}

#[test]
fn f10_no_new_prod_files_exceed_800_lines() {
    const MAX_PROD_LINES: usize = 800;
    const LEGACY_LARGE_FILES: &[(&str, usize)] = &[
        ("app/bar/mod.rs", 839),
        ("app/menu/mod.rs", 1296),
        ("core/notch/focus_activity.rs", 1593),
        ("core/settings/focus_settings.rs", 1369),
        ("core/settings/settings_content.rs", 1202),
        ("platform/notch/shelf.rs", 986),
        ("platform/window/ops.rs", 879),
        ("render/control_center/main_page.rs", 910),
        ("render/dock/renderer.rs", 1220),
        ("render/lib.rs", 1413),
        ("render/notch/focus_stats.rs", 865),
        ("render/notch/notification.rs", 816),
        ("render/notch/pomodoro.rs", 1111),
        ("render/notch/shelf.rs", 989),
        ("render/popup/menu.rs", 962),
        ("render/popup/notifications.rs", 848),
        ("render/settings/focus.rs", 903),
        ("render/window/genie.rs", 846),
    ];
    for crate_dir in ["core", "platform", "render", "app"] {
        for (file, content) in crate_rs_files(crate_dir) {
            if is_test_source(&file) {
                continue;
            }
            let key = format!("{crate_dir}/{file}");
            let lines = prod_source(&content).lines().count();
            if let Some(&(_, ceiling)) = LEGACY_LARGE_FILES
                .iter()
                .find(|&&(legacy, _)| legacy == key)
            {
                assert!(
                    lines <= ceiling,
                    "{key} grew to {lines} prod lines (ratchet ceiling {ceiling}); split it instead of growing a legacy god file"
                );
            } else {
                assert!(
                    lines <= MAX_PROD_LINES,
                    "{key} has {lines} prod lines (limit {MAX_PROD_LINES}); split it instead of growing a god file"
                );
            }
        }
    }
}

fn canonical_app_feature(segment: &str) -> Option<&'static str> {
    match segment {
        "appearance" | "backdrop_feed" | "theme_state" => Some("appearance"),
        "bar" | "bar_controls" | "notch_io" => Some("bar"),
        "control_center" => Some("control_center"),
        "dock" => Some("dock"),
        "focus_shield" => Some("focus_shield"),
        "genie" => Some("genie"),
        "menu" => Some("menu"),
        "notch" => Some("notch"),
        "settings" | "settings_window" => Some("settings"),
        "spotlight" => Some("spotlight"),
        "window_controls" => Some("window_controls"),
        _ => None,
    }
}

#[test]
fn f11_app_feature_sibling_coupling_does_not_grow() {
    const ALLOWED_EDGES: &[(&str, &[&str])] = &[
        (
            "appearance",
            &[
                "bar",
                "control_center",
                "dock",
                "menu",
                "settings",
                "spotlight",
            ],
        ),
        (
            "bar",
            &[
                "appearance",
                "control_center",
                "dock",
                "focus_shield",
                "menu",
                "notch",
                "settings",
                "spotlight",
                "window_controls",
            ],
        ),
        (
            "dock",
            &[
                "appearance",
                "bar",
                "control_center",
                "focus_shield",
                "genie",
                "menu",
                "spotlight",
            ],
        ),
        ("focus_shield", &["appearance"]),
        ("genie", &["dock"]),
        (
            "menu",
            &[
                "appearance",
                "bar",
                "control_center",
                "dock",
                "genie",
                "settings",
                "spotlight",
            ],
        ),
        ("notch", &[]),
        ("spotlight", &["appearance", "control_center", "menu"]),
        ("window_controls", &["genie", "menu"]),
        (
            "control_center",
            &["appearance", "bar", "menu", "settings", "spotlight"],
        ),
        (
            "settings",
            &[
                "appearance",
                "bar",
                "control_center",
                "genie",
                "menu",
                "window_controls",
            ],
        ),
    ];
    for &(feature, allowed) in ALLOWED_EDGES {
        let prefix = format!("{feature}/");
        for (file, content) in crate_rs_files("app") {
            if !file.starts_with(&prefix) || is_test_source(&file) {
                continue;
            }
            let prod = prod_source(&content);
            let mut rest = prod.as_str();
            while let Some(pos) = rest.find("crate::") {
                rest = &rest[pos + 7..];
                let ident_len = rest
                    .bytes()
                    .take_while(|b| b.is_ascii_lowercase() || *b == b'_')
                    .count();
                let segment = &rest[..ident_len];
                if let Some(target) = canonical_app_feature(segment)
                    && target != feature
                {
                    assert!(
                        allowed.contains(&target),
                        "app/{file} must not add new sibling dependency on crate::{target}"
                    );
                }
            }
        }
    }
}

fn collect_decision_ids(markdown: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in markdown.lines() {
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        let rest = rest.strip_prefix("**").unwrap_or(rest);
        let token = rest
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
            .next()
            .unwrap_or("");
        if let Some(num) = token.strip_prefix("DEC-")
            && num.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        {
            ids.push(token.to_owned());
        }
    }
    ids
}

#[test]
fn f12_agent_docs_stay_concise_and_consistent() {
    const MAX_AGENT_DOC_BYTES: usize = 12_288;
    const MAX_ROOT_AGENTS_BYTES: usize = 8_192;

    let root = workspace_root();
    let agents_md = fs::read_to_string(root.join("AGENTS.md")).expect("read AGENTS.md");
    assert!(
        agents_md.len() <= MAX_ROOT_AGENTS_BYTES,
        "AGENTS.md is {} bytes (limit {MAX_ROOT_AGENTS_BYTES}); keep root instructions concise",
        agents_md.len()
    );
    assert!(
        (agents_md.contains("scripts/gate.ps1") || agents_md.contains(r"scripts\gate.ps1"))
            && agents_md.contains(".agents/product.md"),
        "AGENTS.md must stay aligned with scripts/gate.ps1 and .agents/product.md"
    );

    for rel in [
        ".agents/engineering.md",
        ".agents/product.md",
        ".agents/DESIGN.md",
    ] {
        let content = fs::read_to_string(root.join(rel)).expect("read active agent doc");
        assert!(
            content.len() <= MAX_AGENT_DOC_BYTES,
            "{rel} is {} bytes (limit {MAX_AGENT_DOC_BYTES}); archive historical detail under .agents/archive/",
            content.len()
        );
    }

    for rel in [".agents/engineering.md", ".agents/archive/decisions-v1.md"] {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read decision doc");
        let mut seen = std::collections::HashSet::new();
        for id in collect_decision_ids(&content) {
            assert!(
                seen.insert(id.clone()),
                "{rel} defines duplicate decision ID {id}"
            );
        }
    }
}
