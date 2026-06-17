use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
    pub depends: Vec<String>,
    pub sources: Vec<String>,
    pub patches: Vec<PathBuf>,
}

impl Package {
    pub fn from_dir(path: &Path) -> Option<Self> {
        if !path.is_dir() {
            return None;
        }
        let name = path.file_name()?.to_str()?.to_string();

        let version = fs::read_to_string(path.join("version"))
            .ok()?
            .trim()
            .to_string();

        let depends = if path.join("depends").exists() {
            fs::read_to_string(path.join("depends"))
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let sources = if path.join("sources").exists() {
            fs::read_to_string(path.join("sources"))
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let patches = if path.join("patches").exists() {
            let mut p = Vec::new();
            if let Ok(entries) = fs::read_dir(path.join("patches")) {
                for entry in entries.flatten() {
                    p.push(entry.path());
                }
            }
            p.sort();
            p
        } else {
            Vec::new()
        };

        Some(Package { name, path: path.to_path_buf(), version, depends, sources, patches })
    }

    pub fn has_build_script(&self) -> bool {
        self.path.join("swellbuild").exists()
    }

    pub fn build_script_path(&self) -> PathBuf {
        self.path.join("swellbuild")
    }

    pub fn sha256_path(&self) -> PathBuf {
        self.path.join(".sha256")
    }

    pub fn checksums(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let spath = self.sha256_path();
        if !spath.exists() {
            return map;
        }
        if let Ok(content) = fs::read_to_string(&spath) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some((hash, filename)) = line.split_once(char::is_whitespace).map(|(a, b)| {
                    let b = b.trim();
                    (a.to_string(), b.trim_start_matches('*').to_string())
                }) {
                    map.insert(filename, hash);
                }
            }
        }
        map
    }
}

pub fn discover_packages(repo_path: &Path) -> Vec<Package> {
    let mut packages = Vec::new();
    if !repo_path.exists() {
        return packages;
    }

    let categories = ["meta", "core", "desktop", "browser", "dev", "gaming", "multimedia", "office", "network"];
    for cat in &categories {
        let cat_path = repo_path.join(cat);
        if !cat_path.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&cat_path) {
            for entry in entries.flatten() {
                let pkg_path = entry.path();
                if pkg_path.is_dir() {
                    if let Some(pkg) = Package::from_dir(&pkg_path) {
                        packages.push(pkg);
                    }
                }
            }
        }
    }
    packages
}

pub fn find_package<'a>(packages: &'a [Package], name: &str) -> Option<&'a Package> {
    packages.iter().find(|p| p.name == name)
}
