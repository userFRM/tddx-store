use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let workspace_root = find_workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("workspace root with Cargo.lock");
    let lock_path = workspace_root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());

    emit_locked_version(&lock_path, "thetadatadx", "TDDS_THETADATADX_VERSION");
    emit_locked_version(&lock_path, "tdbe", "TDDS_TDBE_VERSION");

    tauri_build::build()
}

fn emit_locked_version(lock_path: &Path, package_name: &str, env_name: &str) {
    let version = resolved_version(lock_path, package_name).unwrap_or_else(|err| {
        panic!(
            "failed to resolve {package_name} version from {}: {err}",
            lock_path.display()
        )
    });
    println!("cargo:rustc-env={env_name}={version}");
}

fn resolved_version(lock_path: &Path, package_name: &str) -> Result<String, String> {
    let raw = fs::read_to_string(lock_path).map_err(|err| format!("read lockfile: {err}"))?;
    let value: toml::Value = raw
        .parse()
        .map_err(|err| format!("parse lockfile: {err}"))?;
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock missing [[package]] entries")?;
    for package in packages {
        let table = package.as_table().ok_or("package entry was not a table")?;
        if table.get("name").and_then(toml::Value::as_str) == Some(package_name) {
            return table
                .get("version")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{package_name} package missing version"));
        }
    }
    Err(format!("{package_name} not present in Cargo.lock"))
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| path.join("Cargo.lock").exists())
        .map(Path::to_path_buf)
}
