#![allow(dead_code)]

mod db;
mod install;
mod repo;

use clap::{Parser, Subcommand};
use std::path::Path;

const REPO_URL: &str = "https://packages.swellos.org/repo.db";

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
    /// Pin all installed packages
    Freeze,
    /// Unpin and resume rolling
    Unfreeze,
    /// List installed packages
    #[command(alias = "-L")]
    List,
    /// Show info about a package
    #[command(alias = "-I")]
    Info {
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
        Some(Commands::Freeze) => {
            for pkg in db::installed_packages() {
                db::freeze_package(&pkg);
            }
            println!("All installed packages frozen.");
        }
        Some(Commands::Unfreeze) => {
            for pkg in db::all_frozen() {
                db::unfreeze_package(&pkg);
            }
            println!("All packages unfrozen.");
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
        None => {
            println!("sbpm -Syu    Sync and upgrade");
            println!("sbpm -S pkg  Install package");
            println!("sbpm -R pkg  Remove package");
            println!("sbpm -U      Upgrade all");
            println!("sbpm -Ss q   Search packages");
            println!("sbpm -L      List installed");
            println!("sbpm -I pkg  Show package info");
            println!("sbpm freeze  Freeze all installed");
            println!("sbpm unfreeze");
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

    // Fetch fresh repo.db
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

    // Install dependencies first
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

                // Remove old files
                let old_files = db::get_installed_files(pkg_name);
                for file in &old_files {
                    let path = Path::new(file);
                    if path.exists() {
                        let _ = std::fs::remove_file(path);
                    }
                }

                // Download and install new version
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
