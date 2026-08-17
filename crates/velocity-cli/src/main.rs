//! Velocity CLI — Command-line tool for creating and building installers.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "velocity",
    about = "Velocity Installer — Free, open-source Windows installer framework",
    version,
    author = "UnitBuilds CC"
)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Velocity installer project
    Init {
        /// Project directory name
        name: Option<String>,

        /// Skip auto-detection and create a minimal project
        #[arg(long)]
        minimal: bool,
    },

    /// Build an installer from the current project
    Build {
        /// Output path for the installer .exe
        #[arg(short, long)]
        output: Option<String>,

        /// Compression level (0-22)
        #[arg(short, long, default_value = "3")]
        compression: i32,

        /// Path to the runtime binary
        #[arg(long)]
        runtime: Option<String>,

        /// Quiet mode (minimal output)
        #[arg(short, long)]
        quiet: bool,
    },

    /// Auto-detect project settings and generate velocity.toml
    Detect {
        /// Project directory to scan
        #[arg(default_value = ".")]
        dir: String,
    },

    /// Validate a velocity.toml configuration
    Check {
        /// Path to velocity.toml
        #[arg(default_value = "velocity.toml")]
        config: String,
    },

    /// Show information about a built installer
    Info {
        /// Path to the installer .exe
        path: String,
    },

    /// Show version information
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(filter.parse().unwrap()),
        )
        .init();

    match cli.command {
        Commands::Init { name, minimal } => {
            commands::init::run(name, minimal)?;
        }
        Commands::Build {
            output,
            compression,
            runtime,
            quiet,
        } => {
            commands::build::run(output, compression, runtime, quiet)?;
        }
        Commands::Detect { dir } => {
            commands::detect::run(&dir)?;
        }
        Commands::Check { config } => {
            commands::check::run(&config)?;
        }
        Commands::Info { path } => {
            commands::info::run(&path)?;
        }
        Commands::Version => {
            println!("Velocity Installer v{}", env!("CARGO_PKG_VERSION"));
            println!("Built with Rust {}", rustc_version());
        }
    }

    Ok(())
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string()
}
