use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::package::Package;

const SOURCE_DIR: &str = "/usr/src/swell/sources";
const BUILD_DIR: &str = "/usr/src/swell/build";
const DESTDIR_BASE: &str = "/usr/src/swell/dest";

pub fn download_source(pkg: &Package) -> Result<(), String> {
    let src_dir = PathBuf::from(SOURCE_DIR).join(&pkg.name);
    fs::create_dir_all(&src_dir).map_err(|e| format!("failed to create source dir: {}", e))?;

    let checksums = pkg.checksums();

    for url in &pkg.sources {
        let filename = url.split('/').last().unwrap_or("unknown");
        let dest = src_dir.join(filename);

        if dest.exists() {
            if let Some(expected_hash) = checksums.get(filename) {
                if verify_sha256(&dest, expected_hash) {
                    println!("  Source already downloaded and verified: {}", filename);
                    continue;
                } else {
                    println!("  Checksum mismatch, re-downloading: {}", filename);
                }
            } else {
                println!("  Already downloaded: {}", filename);
                continue;
            }
        }

        println!("  Downloading: {}", url);
        let resp = reqwest::blocking::get(url)
            .map_err(|e| format!("failed to download {}: {}", url, e))?;

        let bytes = resp.bytes()
            .map_err(|e| format!("failed to read response: {}", e))?;

        fs::write(&dest, &bytes)
            .map_err(|e| format!("failed to write {}: {}", filename, e))?;

        println!("  Saved: {}", filename);

        if let Some(expected_hash) = checksums.get(filename) {
            if !verify_sha256(&dest, expected_hash) {
                return Err(format!("sha256 mismatch for {}", filename));
            }
            println!("  Checksum verified: {}", filename);
        }
    }
    Ok(())
}

pub fn verify_sha256(path: &Path, expected: &str) -> bool {
    use sha2::{Digest, Sha256};
    if let Ok(data) = fs::read(path) {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = format!("{:x}", hasher.finalize());
        result == expected.to_lowercase()
    } else {
        false
    }
}

pub fn run_build(pkg: &Package) -> Result<(), String> {
    let build_dir = PathBuf::from(BUILD_DIR).join(&pkg.name);
    let dest_dir = PathBuf::from(DESTDIR_BASE).join(&pkg.name);

    if build_dir.exists() {
        fs::remove_dir_all(&build_dir)
            .map_err(|e| format!("failed to clean build dir: {}", e))?;
    }
    if dest_dir.exists() {
        fs::remove_dir_all(&dest_dir)
            .map_err(|e| format!("failed to clean dest dir: {}", e))?;
    }

    fs::create_dir_all(&build_dir)
        .map_err(|e| format!("failed to create build dir: {}", e))?;
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create dest dir: {}", e))?;

    if !pkg.has_build_script() {
        return Err(format!("no swellbuild script for {}", pkg.name));
    }

    let source_dir = PathBuf::from(SOURCE_DIR).join(&pkg.name);
    let script_path = pkg.build_script_path();

    let sd = source_dir.display();
    let dd = dest_dir.display();
    let sbpm_lib = format!(
        r#"
DESTDIR="{dd}"
SRCDIR="{sd}"

fetch_url() {{
    echo "fetch_url: $1 (handled by sbpm)"
}}

unpack() {{
    local archive="$1"
    case "$archive" in
        *.tar.gz|*.tgz) tar xzf "{sd}/$archive" ;;
        *.tar.xz)       tar xJf "{sd}/$archive" ;;
        *.tar.bz2)      tar xjf "{sd}/$archive" ;;
        *.tar)          tar xf "{sd}/$archive" ;;
        *.zip)          unzip "{sd}/$archive" ;;
        *)              echo "Unknown archive: $archive"; return 1 ;;
    esac
}}
"#
    );

    let script = format!(
        r#"set -e
{}
source "{}"

if declare -f pkg_fetch > /dev/null; then
    cd "{}"
    pkg_fetch
fi

if declare -f pkg_unpack > /dev/null; then
    pkg_unpack
fi

# cd into the first extracted directory
cd_dir() {{
    for d in */; do
        if [ -d "$d" ]; then
            cd "$d"
            return 0
        fi
    done
    return 1
}}

if declare -f pkg_build > /dev/null; then
    cd_dir
    pkg_build
fi

if declare -f pkg_install > /dev/null; then
    pkg_install
fi
"#,
        sbpm_lib,
        script_path.display(),
        source_dir.display(),
    );

    println!("  Building {}...", pkg.name);

    let output = Command::new("bash")
        .arg("-c")
        .arg(&script)
        .current_dir(&build_dir)
        .output()
        .map_err(|e| format!("failed to run build script: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Build failed for {}:\nSTDOUT:\n{}\nSTDERR:\n{}",
            pkg.name, stdout, stderr
        ));
    }

    println!("  Build complete for {}", pkg.name);
    Ok(())
}

pub fn collect_installed_files(pkg: &Package) -> Result<Vec<String>, String> {
    let dest_dir = PathBuf::from(DESTDIR_BASE).join(&pkg.name);
    if !dest_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_files(&dest_dir, &dest_dir, &mut files);
    Ok(files)
}

fn collect_files(base: &Path, dir: &Path, files: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(base, &path, files);
            } else {
                if let Ok(relative) = path.strip_prefix(base) {
                    files.push(format!("/{}", relative.display()));
                }
            }
        }
    }
}

pub fn install_to_root(pkg: &Package) -> Result<Vec<String>, String> {
    let dest_dir = PathBuf::from(DESTDIR_BASE).join(&pkg.name);
    if !dest_dir.exists() {
        return Err(format!("no build output for {}", pkg.name));
    }

    let mut installed = Vec::new();
    copy_to_root(&dest_dir, &dest_dir, &mut installed)?;
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

pub fn resolve_dependencies<'a>(
    packages: &'a [crate::package::Package],
    name: &str,
) -> Result<Vec<&'a crate::package::Package>, String> {
    let mut resolved = Vec::new();
    let mut seen = std::collections::HashSet::new();
    resolve_deps_recursive(packages, name, &mut resolved, &mut seen)?;
    Ok(resolved)
}

fn resolve_deps_recursive<'a>(
    packages: &'a [crate::package::Package],
    name: &str,
    resolved: &mut Vec<&'a crate::package::Package>,
    seen: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    if seen.contains(name) {
        return Ok(());
    }
    seen.insert(name.to_string());

    let pkg = crate::package::find_package(packages, name)
        .ok_or_else(|| format!("package not found: {}", name))?;

    for dep in &pkg.depends {
        resolve_deps_recursive(packages, dep, resolved, seen)?;
    }

    resolved.push(pkg);
    Ok(())
}
