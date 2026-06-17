#![allow(dead_code)]

mod build;
mod db;
mod package;

use clap::{Parser, Subcommand};
use std::path::Path;

const REPO_PATH: &str = "/usr/src/swell/packages";

#[derive(Parser)]
#[command(name = "sbpm", about = "Swell Blazing Package Manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package (download, build, install)
    #[command(alias = "-S")]
    Install {
        packages: Vec<String>,
        #[arg(long, help = "Try binary cache first")]
        from_cache: bool,
        #[arg(long, help = "Always build from source")]
        source_only: bool,
    },
    /// Remove a package
    #[command(alias = "-R")]
    Remove {
        packages: Vec<String>,
    },
    /// Upgrade all out-of-date packages
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
    /// Pin all packages to current versions
    Freeze,
    /// Unpin and resume rolling
    Unfreeze,
    /// List installed packages
    List,
    /// Show info about a package
    Info {
        package: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let repo_path = Path::new(REPO_PATH);
    let packages = package::discover_packages(repo_path);

    match cli.command {
        Some(Commands::Install { packages: pkgs, from_cache, source_only }) => {
            if pkgs.is_empty() {
                eprintln!("error: no packages specified");
                std::process::exit(1);
            }
            for pkg_name in &pkgs {
                if db::is_frozen(pkg_name) {
                    eprintln!("error: {} is frozen — unfreeze first to upgrade", pkg_name);
                    std::process::exit(1);
                }
                if let Err(e) = install_package(&packages, pkg_name, from_cache, source_only) {
                    eprintln!("error: {}: {}", pkg_name, e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Remove { packages: pkgs }) => {
            for pkg_name in &pkgs {
                remove_package(pkg_name);
            }
        }
        Some(Commands::Upgrade) => {
            upgrade_all(&packages);
        }
        Some(Commands::Search { query }) => {
            search_packages(&packages, &query);
        }
        Some(Commands::SyncUpgrade) => {
            println!("Syncing repo metadata...");
            upgrade_all(&packages);
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
            if let Some(pkg) = package::find_package(&packages, &package) {
                let installed = db::is_installed(&pkg.name);
                let version = db::get_installed_version(&pkg.name);
                println!("Name:       {}", pkg.name);
                println!("Version:    {} (available: {})",
                    version.as_deref().unwrap_or("not installed"),
                    pkg.version);
                println!("Depends:    {}", pkg.depends.join(", "));
                println!("Sources:    {}", pkg.sources.join(", "));
                println!("Patches:    {}", pkg.patches.len());
                println!("Installed:  {}", if installed { "yes" } else { "no" });
                println!("Frozen:     {}", if db::is_frozen(&pkg.name) { "yes" } else { "no" });
            } else {
                eprintln!("error: package '{}' not found in repo", package);
                std::process::exit(1);
            }
        }
        None => {
            println!("sbpm -Syu    Sync and upgrade");
            println!("sbpm -S pkg  Install package(s)");
            println!("sbpm -R pkg  Remove package(s)");
            println!("sbpm -U      Upgrade all");
            println!("sbpm -Ss q   Search packages");
            println!("sbpm -L      List installed");
            println!("sbpm -I pkg  Show package info");
            println!("sbpm freeze  Freeze all installed");
            println!("sbpm unfreeze");
        }
    }
}

fn install_package(
    packages: &[package::Package],
    name: &str,
    _from_cache: bool,
    _source_only: bool,
) -> Result<(), String> {
    if db::is_installed(name) {
        println!("Already installed: {}", name);
        return Ok(());
    }

    let deps = build::resolve_dependencies(packages, name)?;
    let mut newly_installed = Vec::new();

    for dep in &deps {
        if db::is_installed(&dep.name) {
            println!("Already installed (dep): {}", dep.name);
            continue;
        }

        println!("Installing {} {}...", dep.name, dep.version);

        if dep.has_build_script() {
            build::download_source(dep)?;
            build::run_build(dep)?;
            let files = build::install_to_root(dep)?;
            db::record_install(&dep.name, &dep.version, &files);
            println!("  Installed {} files for {}", files.len(), dep.name);
        } else {
            // metapackage or no build script — just record it
            db::record_install(&dep.name, &dep.version, &[]);
            println!("  Recorded metapackage: {}", dep.name);
        }

        newly_installed.push(dep.name.clone());
    }

    println!("Successfully installed: {}", name);
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

fn upgrade_all(packages: &[package::Package]) {
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

        match package::find_package(packages, pkg_name) {
            Some(pkg) => {
                let current_version = db::get_installed_version(pkg_name).unwrap_or_default();
                if current_version == pkg.version {
                    println!("Up to date: {}-{}", pkg_name, pkg.version);
                    continue;
                }

                println!("Upgrading: {} {} -> {}", pkg_name, current_version, pkg.version);

                if pkg.has_build_script() {
                    // Remove old files first
                    let old_files = db::get_installed_files(pkg_name);
                    for file in &old_files {
                        let path = Path::new(file);
                        if path.exists() {
                            let _ = std::fs::remove_file(path);
                        }
                    }

                    if let Err(e) = build::download_source(pkg) {
                        eprintln!("error upgrading {}: {}", pkg_name, e);
                        continue;
                    }
                    if let Err(e) = build::run_build(pkg) {
                        eprintln!("error upgrading {}: {}", pkg_name, e);
                        continue;
                    }
                    match build::install_to_root(pkg) {
                        Ok(files) => {
                            db::record_install(pkg_name, &pkg.version, &files);
                            println!("  Upgraded: {}-{} ({} files)", pkg_name, pkg.version, files.len());
                        }
                        Err(e) => {
                            eprintln!("error installing {}: {}", pkg_name, e);
                        }
                    }
                }
            }
            None => {
                println!("Package not in repo: {} (skipping)", pkg_name);
            }
        }
    }
}

fn search_packages(packages: &[package::Package], query: &str) {
    let q = query.to_lowercase();
    let mut found = false;
    for pkg in packages {
        if pkg.name.to_lowercase().contains(&q) {
            let installed = if db::is_installed(&pkg.name) { "[installed]" } else { "" };
            println!("  {}-{}  {}", pkg.name, pkg.version, installed);
            found = true;
        }
    }
    if !found {
        println!("No packages found matching: {}", query);
    }
}
