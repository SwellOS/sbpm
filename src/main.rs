use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sbpm", about = "Swell Blazing Package Manager")]
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install { packages, from_cache, source_only }) => {
            println!("Installing: {:?}", packages);
            if from_cache { println!("  Cache mode enabled"); }
            if source_only { println!("  Source-only mode"); }
        }
        Some(Commands::Remove { packages }) => {
            println!("Removing: {:?}", packages);
        }
        Some(Commands::Upgrade) => {
            println!("Upgrading all packages from source");
        }
        Some(Commands::Search { query }) => {
            println!("Searching for: {}", query);
        }
        Some(Commands::SyncUpgrade) => {
            println!("Syncing repo metadata and upgrading all");
        }
        Some(Commands::Freeze) => {
            println!("Freezing all packages");
        }
        Some(Commands::Unfreeze) => {
            println!("Unfreezing all packages");
        }
        None => {
            println!("sbpm -Syu    Sync and upgrade");
            println!("sbpm -S pkg  Install package");
            println!("sbpm -R pkg  Remove package");
            println!("sbpm -Ss q   Search packages");
        }
    }
}
