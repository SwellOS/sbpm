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

fn repo_cache_path() -> PathBuf {
    PathBuf::from(SBPM_DIR).join("repo.db")
}

pub fn save_repo_db(data: &str) -> Result<(), String> {
    let dir = PathBuf::from(SBPM_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create sbpm dir: {}", e))?;
    fs::write(repo_cache_path(), data).map_err(|e| format!("failed to write repo.db: {}", e))
}

pub fn load_cached_repo() -> Option<String> {
    let path = repo_cache_path();
    if path.exists() {
        fs::read_to_string(&path).ok()
    } else {
        None
    }
}

pub fn repo_age_seconds() -> u64 {
    let path = repo_cache_path();
    if !path.exists() {
        return u64::MAX;
    }
    if let Ok(meta) = fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs();
            }
        }
    }
    u64::MAX
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

pub fn record_install(name: &str, version: &str, release: u32, files: &[String]) {
    let pkg_dir = manifest_dir().join(name);
    fs::create_dir_all(&pkg_dir).expect("failed to create manifest directory");

    let manifest = files.join("\n");
    fs::write(pkg_dir.join("manifest"), &manifest).expect("failed to write manifest");
    fs::write(pkg_dir.join("version"), format!("{}-{}\n", version, release)).expect("failed to write version");
}

pub fn remove_package(name: &str) {
    let pkg_dir = manifest_dir().join(name);
    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir).expect("failed to remove package manifest");
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
