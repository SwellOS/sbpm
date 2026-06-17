use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct RepoPackage {
    pub version: String,
    pub release: u32,
    pub depends: Vec<String>,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RepoDb {
    #[serde(flatten)]
    pub packages: HashMap<String, RepoPackage>,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub release: u32,
    pub depends: Vec<String>,
    pub sha256: String,
    pub url: String,
    pub size: u64,
}

pub fn load_repo_db(path: &Path) -> Result<Vec<Package>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read repo.db: {}", e))?;
    parse_repo(&content)
}

pub fn load_repo_db_from_str(content: &str) -> Result<Vec<Package>, String> {
    parse_repo(content)
}

fn parse_repo(content: &str) -> Result<Vec<Package>, String> {
    let repo: RepoDb = serde_json::from_str(content)
        .map_err(|e| format!("failed to parse repo.db: {}", e))?;

    let mut packages = Vec::new();
    for (name, rpkg) in repo.packages {
        packages.push(Package {
            name,
            version: rpkg.version,
            release: rpkg.release,
            depends: rpkg.depends,
            sha256: rpkg.sha256,
            url: rpkg.url,
            size: rpkg.size,
        });
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packages)
}

pub fn find_package<'a>(packages: &'a [Package], name: &str) -> Option<&'a Package> {
    packages.iter().find(|p| p.name == name)
}

pub fn search_packages<'a>(packages: &'a [Package], query: &str) -> Vec<&'a Package> {
    let q = query.to_lowercase();
    packages.iter()
        .filter(|p| p.name.to_lowercase().contains(&q))
        .collect()
}
