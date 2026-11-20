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
