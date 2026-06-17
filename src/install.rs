use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const CACHE_DIR: &str = "/var/cache/sbpm";

pub fn download_package(name: &str, url: &str, expected_sha256: &str) -> Result<PathBuf, String> {
    let cache_dir = PathBuf::from(CACHE_DIR);
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("failed to create cache dir: {}", e))?;

    let filename = url.split('/').last().unwrap_or(&format!("{}.swell", name)).to_string();
    let dest = cache_dir.join(&filename);

    if dest.exists() {
        if verify_sha256(&dest, expected_sha256) {
            println!("  Using cached: {}", filename);
            return Ok(dest);
        } else {
            println!("  Cache invalid, re-downloading: {}", filename);
            let _ = fs::remove_file(&dest);
        }
    }

    println!("  Downloading: {}", url);
    let resp = reqwest::blocking::get(url)
        .map_err(|e| format!("failed to download {}: {}", url, e))?;

    let bytes = resp.bytes()
        .map_err(|e| format!("failed to read response: {}", e))?;

    fs::write(&dest, &bytes)
        .map_err(|e| format!("failed to write {}: {}", filename, e))?;

    if !verify_sha256(&dest, expected_sha256) {
        let _ = fs::remove_file(&dest);
        return Err(format!("sha256 mismatch for {}", filename));
    }

    println!("  Verified: {}", filename);
    Ok(dest)
}

pub fn verify_sha256(path: &Path, expected: &str) -> bool {
    if let Ok(data) = fs::read(path) {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = format!("{:x}", hasher.finalize());
        result == expected.to_lowercase()
    } else {
        false
    }
}

pub fn extract_and_install(archive: &Path, name: &str) -> Result<Vec<String>, String> {
    let extract_dir = PathBuf::from("/tmp/sbpm-extract").join(name);

    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir)
            .map_err(|e| format!("failed to clean extract dir: {}", e))?;
    }
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("failed to create extract dir: {}", e))?;

    println!("  Extracting...");
    let status = Command::new("tar")
        .args(["--zstd", "-xf", &archive.to_string_lossy(), "-C", &extract_dir.to_string_lossy()])
        .status()
        .map_err(|e| format!("failed to extract archive: {}", e))?;

    if !status.success() {
        return Err("archive extraction failed".to_string());
    }

    let root_dir = extract_dir.join("root");
    if !root_dir.exists() {
        return Err("archive missing root/ directory".to_string());
    }

    println!("  Installing files...");
    let mut installed = Vec::new();
    copy_to_root(&root_dir, &root_dir, &mut installed)?;

    println!("  Installed {} files", installed.len());
    Ok(installed)
}

fn copy_to_root(base: &Path, dir: &Path, installed: &mut Vec<String>) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(base)
                .map_err(|_| "strip_prefix failed".to_string())?;
            let target = Path::new("/").join(relative);

            if path.is_dir() {
                fs::create_dir_all(&target)
                    .map_err(|e| format!("failed to create dir {}: {}", target.display(), e))?;
                copy_to_root(base, &path, installed)?;
            } else {
                fs::copy(&path, &target)
                    .map_err(|e| format!("failed to copy {}: {}", target.display(), e))?;
                installed.push(format!("/{}", relative.display()));
            }
        }
    }
    Ok(())
}

pub fn list_cached() -> Vec<String> {
    let cache_dir = PathBuf::from(CACHE_DIR);
    if !cache_dir.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map_or(false, |e| e == "swell") {
                if let Some(name) = entry.file_name().to_str() {
                    files.push(name.to_string());
                }
            }
        }
    }
    files
}
