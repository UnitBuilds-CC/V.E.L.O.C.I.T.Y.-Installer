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

        /// Compression format: zstd (fast) or lzma2 (smaller)
        #[arg(short = 'f', long, default_value = "zstd")]
        format: Option<String>,

        /// Package format: exe (default) or msi (Windows Installer)
        #[arg(long, default_value = "exe")]
        package_format: Option<String>,

        /// Path to the runtime binary
        #[arg(long)]
        runtime: Option<String>,

        /// Generate delta update package (requires previous version in output directory)
        #[arg(long)]
        delta: bool,

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

    /// Sign an installer executable with a code signing certificate
    Sign {
        /// Path to the installer .exe to sign
        path: String,

        /// Path to the certificate file (.pfx)
        #[arg(short = 'c', long)]
        cert: Option<String>,

        /// Certificate SHA1 fingerprint (thumbprint)
        #[arg(short = 'f', long)]
        fingerprint: Option<String>,

        /// Certificate subject name
        #[arg(short = 'n', long)]
        subject: Option<String>,

        /// Timestamp server URL (RFC 3161)
        #[arg(short = 't', long)]
        timestamp: Option<String>,

        /// Description for the signed file
        #[arg(short = 'd', long)]
        description: Option<String>,

        /// Verify signature instead of signing
        #[arg(short = 'v', long)]
        verify: bool,
    },

    /// Manage remote dependencies and bundled applications
    Dep {
        /// Subcommand: list, add, resolve, remove
        subcommand: String,

        /// Path to velocity.toml
        #[arg(short = 'c', long, default_value = "velocity.toml")]
        config: String,

        /// Additional arguments for the subcommand
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Check for updates and install the latest version
    Update {
        /// Only check for updates without installing
        #[arg(long)]
        check: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(
                filter
                    .parse()
                    .unwrap_or_else(|_| tracing_subscriber::filter::LevelFilter::INFO.into()),
            ),
        )
        .init();

    match cli.command {
        Commands::Init { name, minimal } => {
            commands::init::run(name, minimal)?;
        }
        Commands::Build {
            output,
            compression,
            format,
            package_format,
            runtime,
            delta,
            quiet,
        } => {
            commands::build::run(output, compression, format, package_format, runtime, delta, quiet)?;
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
        Commands::Sign {
            path,
            cert,
            fingerprint,
            subject,
            timestamp,
            description,
            verify,
        } => {
            if verify {
                commands::sign::verify(&path)?;
            } else {
                commands::sign::run(
                    &path,
                    cert.as_deref(),
                    fingerprint.as_deref(),
                    subject.as_deref(),
                    timestamp.as_deref(),
                    description.as_deref(),
                )?;
            }
        }
        Commands::Dep {
            subcommand,
            config,
            args,
        } => {
            #[cfg(target_os = "windows")]
            {
                commands::dep::run(&subcommand, &config, &args)?;
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (subcommand, config, args);
                eprintln!("The 'dep' command is currently only supported on Windows.");
            }
        }
        Commands::Update { check } => {
            #[cfg(target_os = "windows")]
            {
                if check {
                    commands::update::run_check()?;
                } else {
                    commands::update::run_update()?;
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = check;
                eprintln!("The 'update' command is currently only supported on Windows.");
            }
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
