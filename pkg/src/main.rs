//! NOVA Package Manager
//! 
//! Handles project initialization, dependency resolution, package publishing,
//! and registry interactions for the NOVA ecosystem.

mod commands;
mod manifest;
mod resolver;
mod registry;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "nova-pkg")]
#[command(author = "NOVA Team")]
#[command(version = "0.3.0")]
#[command(about = "NOVA Package Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new NOVA project
    Init {
        /// Project name
        name: String,
        /// Create as library instead of binary
        #[arg(long)]
        lib: bool,
    },
    
    /// Add a dependency to the project
    Add {
        /// Package name
        name: String,
        /// Version requirement (e.g., \"1.0\", \">=0.5,<2.0\")
        #[arg(short, long, default_value = "*")]
        version: String,
        /// Add as dev dependency
        #[arg(long)]
        dev: bool,
    },
    
    /// Remove a dependency from the project
    Remove {
        /// Package name
        name: String,
        /// Remove from dev dependencies
        #[arg(long)]
        dev: bool,
    },
    
    /// Update dependencies to latest compatible versions
    Update {
        /// Specific package to update
        name: Option<String>,
        /// Update to latest incompatible versions
        #[arg(long)]
        breaking: bool,
    },
    
    /// Build the project and its dependencies
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Disable incremental compilation
        #[arg(long)]
        no_incremental: bool,
    },
    
    /// Run the project
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
        /// Build in release mode
        #[arg(long)]
        release: bool,
    },
    
    /// Publish the package to the registry
    Publish {
        /// Registry URL (defaults to public registry)
        #[arg(long)]
        registry: Option<String>,
        /// Dry run - don't actually publish
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Search for packages in the registry
    Search {
        /// Search query
        query: String,
        /// Maximum results to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    
    /// Show information about a package
    Info {
        /// Package name
        name: String,
        /// Specific version
        #[arg(long)]
        version: Option<String>,
    },
    
    /// List all dependencies
    Tree {
        /// Show only direct dependencies
        #[arg(long)]
        depth: Option<usize>,
    },
    
    /// Clean build artifacts
    Clean {
        /// Clean release builds only
        #[arg(long)]
        release: bool,
    },
    
    /// Check for outdated dependencies
    Outdated {
        /// Show only major version updates available
        #[arg(long)]
        major_only: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    
    if let Err(e) = match cli.command {
        Commands::Init { name, lib } => commands::init(&name, lib),
        Commands::Add { name, version, dev } => commands::add(&name, &version, dev),
        Commands::Remove { name, dev } => commands::remove(&name, dev),
        Commands::Update { name, breaking } => commands::update(name.as_deref(), breaking),
        Commands::Build { release, no_incremental } => commands::build(release, no_incremental),
        Commands::Run { args, release } => commands::run(&args, release),
        Commands::Publish { registry, dry_run } => commands::publish(registry.as_deref(), dry_run),
        Commands::Search { query, limit } => commands::search(&query, limit),
        Commands::Info { name, version } => commands::info(&name, version.as_deref()),
        Commands::Tree { depth } => commands::tree(depth),
        Commands::Clean { release } => commands::clean(release),
        Commands::Outdated { major_only } => commands::outdated(major_only),
    } {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
