use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const SBPM_DIR: &str = "/var/lib/sbpm";

fn manifest_dir() -> PathBuf {
    PathBuf::from(SBPM_DIR).join("installed")
}

fn freeze_path() -> PathBuf {
    PathBuf::from(SBPM_DIR).join("freeze")
}

pub fn installed_packages() -> Vec<String> {
    let dir = manifest_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut pkgs = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    pkgs.push(name.to_string());
                }
            }
        }
    }
    pkgs
}

pub fn is_installed(name: &str) -> bool {
    manifest_dir().join(name).exists()
}

pub fn get_installed_files(name: &str) -> Vec<String> {
    let manifest_file = manifest_dir().join(name).join("manifest");
    if !manifest_file.exists() {
        return Vec::new();
    }
    fs::read_to_string(&manifest_file)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn record_install(name: &str, version: &str, files: &[String]) {
    let pkg_dir = manifest_dir().join(name);
    fs::create_dir_all(&pkg_dir).expect("failed to create manifest directory");

    let manifest = files.join("\n");
    fs::write(pkg_dir.join("manifest"), &manifest).expect("failed to write manifest");
    fs::write(pkg_dir.join("version"), format!("{}\n", version)).expect("failed to write version");
}

pub fn remove_package(name: &str) {
    let pkg_dir = manifest_dir().join(name);
    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir).expect("failed to remove package manifest");
    }
}

pub fn record_installed_file(name: &str, file_path: &str) {
    let manifest_file = manifest_dir().join(name).join("manifest");
    if let Ok(content) = fs::read_to_string(&manifest_file) {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        if !lines.contains(&file_path.to_string()) {
            lines.push(file_path.to_string());
            fs::write(&manifest_file, format!("{}\n", lines.join("\n")))
                .expect("failed to update manifest");
        }
    }
}

pub fn get_installed_version(name: &str) -> Option<String> {
    let version_file = manifest_dir().join(name).join("version");
    if !version_file.exists() {
        return None;
    }
    fs::read_to_string(&version_file).ok().map(|s| s.trim().to_string())
}

pub fn is_frozen(name: &str) -> bool {
    if !freeze_path().exists() {
        return false;
    }
    let content = fs::read_to_string(freeze_path()).unwrap_or_default();
    content.lines().any(|l| l.trim() == name)
}

pub fn freeze_package(name: &str) {
    let mut frozen: HashSet<String> = HashSet::new();
    if freeze_path().exists() {
        if let Ok(content) = fs::read_to_string(freeze_path()) {
            for line in content.lines() {
                frozen.insert(line.trim().to_string());
            }
        }
    }
    frozen.insert(name.to_string());
    let mut list: Vec<String> = frozen.into_iter().collect();
    list.sort();
    fs::write(freeze_path(), format!("{}\n", list.join("\n")))
        .expect("failed to write freeze file");
}

pub fn unfreeze_package(name: &str) {
    if !freeze_path().exists() {
        return;
    }
    let content = fs::read_to_string(freeze_path()).unwrap_or_default();
    let mut frozen: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l != name)
        .collect();
    frozen.sort();
    if frozen.is_empty() {
        let _ = fs::remove_file(freeze_path());
    } else {
        fs::write(freeze_path(), format!("{}\n", frozen.join("\n")))
            .expect("failed to write freeze file");
    }
}

pub fn all_frozen() -> Vec<String> {
    if !freeze_path().exists() {
        return Vec::new();
    }
    fs::read_to_string(freeze_path())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn list_old_manifests() -> Vec<String> {
    let mut old = Vec::new();
    for pkg in installed_packages() {
        let version_file = manifest_dir().join(&pkg).join("old_versions");
        if version_file.exists() {
            old.push(pkg);
        }
    }
    old
}
