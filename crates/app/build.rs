use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/otto-os.ico");
    println!("cargo:rerun-if-changed=otto-os.manifest");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Some(rc_exe) = find_rc_exe() else {
        panic!("Windows SDK rc.exe was not found in PATH or Windows Kits 10 bin directories");
    };

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let icon_path = manifest_dir.join("assets").join("otto-os.ico");
    let app_manifest_path = manifest_dir.join("otto-os.manifest");

    let major: u16 = env::var("CARGO_PKG_VERSION_MAJOR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let minor: u16 = env::var("CARGO_PKG_VERSION_MINOR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let patch: u16 = env::var("CARGO_PKG_VERSION_PATCH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| String::from("0.1.0"));

    let rc_content = format!(
        "1 ICON \"{icon}\"\n\
1 24 \"{manifest}\"\n\
1 VERSIONINFO\n\
FILEVERSION {major},{minor},{patch},0\n\
PRODUCTVERSION {major},{minor},{patch},0\n\
FILEFLAGSMASK 0x3fL\n\
FILEFLAGS 0x0L\n\
FILEOS 0x40004L\n\
FILETYPE 0x1L\n\
FILESUBTYPE 0x0L\n\
BEGIN\n\
    BLOCK \"StringFileInfo\"\n\
    BEGIN\n\
        BLOCK \"040904b0\"\n\
        BEGIN\n\
            VALUE \"CompanyName\", \"Otto OS Contributors\"\n\
            VALUE \"FileDescription\", \"Otto OS\"\n\
            VALUE \"FileVersion\", \"{major}.{minor}.{patch}.0\"\n\
            VALUE \"InternalName\", \"otto-os.exe\"\n\
            VALUE \"LegalCopyright\", \"Copyright (c) 2026 Otto OS Contributors (MIT)\"\n\
            VALUE \"OriginalFilename\", \"otto-os.exe\"\n\
            VALUE \"ProductName\", \"Otto OS\"\n\
            VALUE \"ProductVersion\", \"{pkg_version}\"\n\
        END\n\
    END\n\
    BLOCK \"VarFileInfo\"\n\
    BEGIN\n\
        VALUE \"Translation\", 0x0409, 1200\n\
    END\n\
END\n",
        icon = rc_path_literal(&icon_path),
        manifest = rc_path_literal(&app_manifest_path),
    );

    let rc_path = out_dir.join("otto-os.rc");
    let res_path = out_dir.join("otto-os.res");
    fs::write(&rc_path, rc_content).expect("write generated otto-os.rc");

    let output = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .output()
        .expect("invoke rc.exe");
    assert!(
        output.status.success(),
        "rc.exe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    println!("cargo:rustc-link-arg-bin=otto-os={}", res_path.display());
}

fn rc_path_literal(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
}

fn find_rc_exe() -> Option<PathBuf> {
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let arch_dir = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => "x64",
    };

    for root_var in ["ProgramFiles(x86)", "ProgramFiles"] {
        let Some(root) = env::var_os(root_var) else {
            continue;
        };
        let kits_bin = PathBuf::from(root)
            .join("Windows Kits")
            .join("10")
            .join("bin");
        let Ok(entries) = fs::read_dir(&kits_bin) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        versions.sort();
        versions.reverse();
        for version_dir in versions {
            let candidate = version_dir.join(arch_dir).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
