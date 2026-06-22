#![allow(dead_code)]

mod db;
mod install;
mod repo;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO_URL: &str = "https://raw.githubusercontent.com/SwellOS/packages/gh-pages/repo.db";

#[derive(Parser)]
#[command(name = "sbpm", about = "Swell Blazing Package Manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package (download binary archive, extract, install)
    #[command(alias = "-S")]
    Install {
        packages: Vec<String>,
    },
    /// Remove a package
    #[command(alias = "-R")]
    Remove {
        packages: Vec<String>,
    },
    /// Upgrade all packages
    #[command(alias = "-U")]
    Upgrade,
    /// Search for a package
    #[command(alias = "-Ss")]
    Search {
        query: String,
    },
    /// Sync repo metadata and upgrade all
    #[command(alias = "-Syu")]
    SyncUpgrade,
    /// Freeze packages (prevent upgrades)
    #[command(alias = "-F")]
    Freeze {
        /// Package names to freeze (empty = freeze all)
        packages: Vec<String>,
    },
    /// Unfreeze packages (resume upgrades)
    #[command(alias = "-u")]
    Unfreeze {
        /// Package names to unfreeze (empty = unfreeze all)
        packages: Vec<String>,
    },
    /// List installed packages
    #[command(alias = "-L", alias = "-Q")]
    List,
    /// Show info about a package
    #[command(alias = "-I")]
    Info {
        package: String,
    },
    /// Build a package from source and install it
    Build {
        /// Package name to build
        package: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install { packages }) => {
            let repo = load_repo();
            let packages_list = match repo {
                Ok(ref p) => p,
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            };

            for pkg_name in &packages {
                if db::is_frozen(pkg_name) {
                    eprintln!("error: {} is frozen", pkg_name);
                    std::process::exit(1);
                }
                if let Err(e) = install_package(packages_list, pkg_name) {
                    eprintln!("error: {}: {}", pkg_name, e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Remove { packages }) => {
            for pkg_name in &packages {
                remove_package(pkg_name);
            }
        }
        Some(Commands::Upgrade) => {
            let repo = load_repo();
            let packages_list = match repo {
                Ok(ref p) => p,
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            };
            upgrade_all(packages_list);
        }
        Some(Commands::Search { query }) => {
            let repo = load_repo();
            match repo {
                Ok(packages_list) => search_packages(&packages_list, &query),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Some(Commands::SyncUpgrade) => {
            println!("Syncing repo metadata...");
            let repo = load_repo();
            let packages_list = match repo {
                Ok(ref p) => p,
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            };
            upgrade_all(packages_list);
        }
        Some(Commands::Freeze { packages }) => {
            if packages.is_empty() {
                for pkg in db::installed_packages() {
                    db::freeze_package(&pkg);
                }
                println!("All installed packages frozen.");
            } else {
                for pkg in &packages {
                    if !db::is_installed(pkg) {
                        eprintln!("error: {} is not installed", pkg);
                        std::process::exit(1);
                    }
                    db::freeze_package(pkg);
                    println!("Frozen: {}", pkg);
                }
            }
        }
        Some(Commands::Unfreeze { packages }) => {
            if packages.is_empty() {
                for pkg in db::all_frozen() {
                    db::unfreeze_package(&pkg);
                }
                println!("All packages unfrozen.");
            } else {
                for pkg in &packages {
                    if !db::is_frozen(pkg) {
                        eprintln!("warning: {} is not frozen", pkg);
                        continue;
                    }
                    db::unfreeze_package(pkg);
                    println!("Unfrozen: {}", pkg);
                }
            }
        }
        Some(Commands::List) => {
            let installed = db::installed_packages();
            if installed.is_empty() {
                println!("No packages installed.");
                return;
            }
            println!("Installed packages:");
            for pkg in &installed {
                let version = db::get_installed_version(pkg).unwrap_or_default();
                let frozen = if db::is_frozen(pkg) { " [frozen]" } else { "" };
                println!("  {}-{}{}", pkg, version, frozen);
            }
        }
        Some(Commands::Info { package }) => {
            let repo = load_repo();
            match repo {
                Ok(packages_list) => {
                    if let Some(pkg) = repo::find_package(&packages_list, &package) {
                        let installed = db::is_installed(&pkg.name);
                        let version = db::get_installed_version(&pkg.name);
                        println!("Name:       {}", pkg.name);
                        println!("Version:    {} (available: {}-{})",
                            version.as_deref().unwrap_or("not installed"),
                            pkg.version, pkg.release);
                        println!("Depends:    {}", pkg.depends.join(", "));
                        println!("Size:       {} KB", pkg.size / 1024);
                        println!("Installed:  {}", if installed { "yes" } else { "no" });
                        println!("Frozen:     {}", if db::is_frozen(&pkg.name) { "yes" } else { "no" });
                    } else {
                        eprintln!("error: package '{}' not found in repo", package);
                    }
                }
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Some(Commands::Build { package }) => {
            if db::is_frozen(&package) {
                eprintln!("error: {} is frozen", package);
                std::process::exit(1);
            }
            if let Err(e) = build_and_install(&package) {
                eprintln!("error: {}: {}", package, e);
                std::process::exit(1);
            }
        }
        None => {
            println!("sbpm -Syu     Sync and upgrade");
            println!("sbpm -S pkg   Install package");
            println!("sbpm -R pkg   Remove package");
            println!("sbpm -U       Upgrade all");
            println!("sbpm -Ss q    Search packages");
            println!("sbpm -L       List installed");
            println!("sbpm -Q       List installed");
            println!("sbpm -I pkg   Show package info");
            println!("sbpm -F pkg   Freeze package (omit pkg for all)");
            println!("sbpm -u pkg   Unfreeze package (omit pkg for all)");
            println!("sbpm build pkg Build from source and install");
        }
    }
}

fn load_repo() -> Result<Vec<repo::Package>, String> {
    if let Some(cached) = db::load_cached_repo() {
        let packages = repo::load_repo_db_from_str(&cached)?;
        if !packages.is_empty() {
            return Ok(packages);
        }
    }

    println!("Fetching package index...");
    let resp = reqwest::blocking::get(REPO_URL)
        .map_err(|e| format!("failed to fetch repo.db: {}", e))?;

    let text = resp.text()
        .map_err(|e| format!("failed to read repo.db: {}", e))?;

    db::save_repo_db(&text)?;
    repo::load_repo_db_from_str(&text)
}

fn install_package(packages: &[repo::Package], name: &str) -> Result<(), String> {
    if db::is_installed(name) {
        println!("Already installed: {}", name);
        return Ok(());
    }

    let pkg = repo::find_package(packages, name)
        .ok_or_else(|| format!("package not found: {}", name))?;

    for dep in &pkg.depends {
        if !db::is_installed(dep) {
            println!("Installing dependency: {}", dep);
            install_package(packages, dep)?;
        }
    }

    println!("Installing {} {}-{}...", pkg.name, pkg.version, pkg.release);
    let archive = install::download_package(&pkg.name, &pkg.url, &pkg.sha256)?;
    let files = install::extract_and_install(&archive, &pkg.name)?;
    db::record_install(&pkg.name, &pkg.version, pkg.release, &files);
    println!("  Done.");
    Ok(())
}

fn remove_package(name: &str) {
    if !db::is_installed(name) {
        println!("Not installed: {}", name);
        return;
    }

    let files = db::get_installed_files(name);
    for file in &files {
        let path = Path::new(file);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("warning: could not remove {}: {}", file, e);
            }
        }
    }

    db::remove_package(name);
    println!("Removed: {}", name);
}

fn upgrade_all(packages: &[repo::Package]) {
    let installed = db::installed_packages();
    if installed.is_empty() {
        println!("No packages installed.");
        return;
    }

    for pkg_name in &installed {
        if db::is_frozen(pkg_name) {
            println!("Skipping frozen: {}", pkg_name);
            continue;
        }

        match repo::find_package(packages, pkg_name) {
            Some(pkg) => {
                let current = db::get_installed_version(pkg_name).unwrap_or_default();
                let available = format!("{}-{}", pkg.version, pkg.release);
                if current == available {
                    println!("Up to date: {}-{}", pkg_name, available);
                    continue;
                }

                println!("Upgrading: {} {} -> {}", pkg_name, current, available);

                let old_files = db::get_installed_files(pkg_name);
                for file in &old_files {
                    let path = Path::new(file);
                    if path.exists() {
                        let _ = std::fs::remove_file(path);
                    }
                }

                match install::download_package(&pkg.name, &pkg.url, &pkg.sha256) {
                    Ok(archive) => {
                        match install::extract_and_install(&archive, &pkg.name) {
                            Ok(files) => {
                                db::record_install(pkg_name, &pkg.version, pkg.release, &files);
                                println!("  Upgraded: {}", pkg_name);
                            }
                            Err(e) => eprintln!("error upgrading {}: {}", pkg_name, e),
                        }
                    }
                    Err(e) => eprintln!("error downloading {}: {}", pkg_name, e),
                }
            }
            None => {
                println!("Package not in repo: {} (skipping)", pkg_name);
            }
        }
    }
}

fn search_packages(packages: &[repo::Package], query: &str) {
    let results = repo::search_packages(packages, query);
    if results.is_empty() {
        println!("No packages found matching: {}", query);
        return;
    }
    for pkg in results {
        let installed = if db::is_installed(&pkg.name) { " [installed]" } else { "" };
        println!("  {}-{}{}{}", pkg.name, pkg.version, pkg.release, installed);
    }
}

fn find_swell_build() -> Result<String, String> {
    // Check common locations
    let candidates = vec![
        "swell-build".to_string(),
        "/usr/local/bin/swell-build".to_string(),
        "/usr/bin/swell-build".to_string(),
        "/usr/src/swell/swell-build/target/release/swell-build".to_string(),
    ];

    for candidate in &candidates {
        if candidate.contains('/') {
            if Path::new(candidate).exists() {
                return Ok(candidate.clone());
            }
        } else {
            // Check PATH
            if let Ok(output) = Command::new("which").arg(candidate).output() {
                if output.status.success() {
                    return Ok(candidate.clone());
                }
            }
        }
    }

    Err("swell-build not found. Install it from https://github.com/SwellOS/swell-build or set up the build workspace at /usr/src/swell/".to_string())
}

fn find_built_package(repo_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let pkgs_dir = repo_dir.join("packages");
    let search_dir = if pkgs_dir.exists() { &pkgs_dir } else { repo_dir };

    if !search_dir.exists() {
        return Err(format!("repo directory not found: {}", search_dir.display()));
    }

    for entry in std::fs::read_dir(search_dir).map_err(|e| format!("failed to read repo dir: {}", e))? {
        let entry = entry.map_err(|e| format!("failed to read entry: {}", e))?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "swell") {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            // Format: {name}-{version}-{release}-{arch}.swell
            if filename.starts_with(&format!("{}-", name)) {
                return Ok(path);
            }
        }
    }

    Err(format!("built package not found for {} in {}", name, search_dir.display()))
}

fn build_and_install(name: &str) -> Result<(), String> {
    let swell_build = find_swell_build()?;

    // Determine the repo directory from environment or default
    let repo_dir = PathBuf::from(std::env::var("SWELL_REPO").unwrap_or_else(|_| "/usr/src/swell/repo".to_string()));

    println!("Building {} from source...", name);
    let status = Command::new(&swell_build)
        .args(["pkg", name])
        .status()
        .map_err(|e| format!("failed to run swell-build: {}", e))?;

    if !status.success() {
        return Err(format!("swell-build failed for package '{}'", name));
    }

    // Find the built .swell archive
    let built = find_built_package(&repo_dir, name)?;
    println!("  Built: {}", built.display());

    // Read metadata from the .swell archive to get version info
    let metadata_str = read_metadata_from_swell(&built)?;
    let (version, release) = parse_metadata_version(&metadata_str)?;

    // Check deps from metadata
    let depends = parse_metadata_depends(&metadata_str);

    // Install dependencies first (from repo if available)
    if let Ok(repo_packages) = load_repo() {
        for dep in &depends {
            if !db::is_installed(dep) {
                println!("Installing dependency: {}", dep);
                if let Err(e) = install_package(&repo_packages, dep) {
                    eprintln!("warning: could not install dependency {}: {}", dep, e);
                }
            }
        }
    }

    // Install files from the built archive
    let files = install::extract_and_install(&built, name)?;
    db::record_install(name, &version, release, &files);
    println!("  Installed: {}-{}", name, version);
    Ok(())
}

fn read_metadata_from_swell(archive: &Path) -> Result<String, String> {
    let output = Command::new("tar")
        .args(["--zstd", "-xf", &archive.to_string_lossy(), "--to-stdout", "metadata.toml"])
        .output()
        .map_err(|e| format!("failed to read metadata from .swell: {}", e))?;

    if !output.status.success() {
        return Err("failed to extract metadata.toml from .swell archive".to_string());
    }

    String::from_utf8(output.stdout).map_err(|e| format!("invalid metadata encoding: {}", e))
}

fn parse_metadata_version(metadata: &str) -> Result<(String, u32), String> {
    // Parse TOML-like metadata: name = "...", version = "...", release = N
    let mut version = String::new();
    let mut release = 0u32;

    for line in metadata.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("version = ") {
            version = val.trim_matches('"').to_string();
        }
        if let Some(val) = line.strip_prefix("release = ") {
            release = val.parse::<u32>().unwrap_or(0);
        }
    }

    if version.is_empty() {
        return Err("version not found in metadata.toml".to_string());
    }

    Ok((version, release))
}

fn parse_metadata_depends(metadata: &str) -> Vec<String> {
    for line in metadata.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("depends = ") {
            let val = val.trim();
            if val == "[]" {
                return Vec::new();
            }
            // Parse ["dep1", "dep2"]
            let inner = val.trim_start_matches('[').trim_end_matches(']');
            return inner.split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}
