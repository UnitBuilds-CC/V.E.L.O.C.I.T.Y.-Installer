//! `velocity build` — Build an installer from the current project.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Run the build command.
pub fn run(
    output: Option<String>,
    compression: i32,
    compression_format: Option<String>,
    runtime: Option<String>,
    quiet: bool,
) -> Result<()> {
    let project_dir = std::env::current_dir().context("Failed to get current directory")?;

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.join("output").join("installer.exe"));

    // Load config to get compression settings
    let config_path = project_dir.join("velocity.toml");
    let (format, level) = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).context("Failed to read velocity.toml")?;
        let manifest: velocity_config::VelocityManifest = toml::from_str(&content).context("Failed to parse velocity.toml")?;
        
        // CLI flags override config
        let format = compression_format.unwrap_or(manifest.files.compression.format);
        let level = if compression != 3 { compression } else { manifest.files.compression.level };
        (format, level)
    } else {
        // No config, use CLI flags or defaults
        let format = compression_format.unwrap_or_else(|| "zstd".to_string());
        (format, compression)
    };

    if !quiet {
        println!();
        println!("  Building Velocity installer...");
        println!(
            "  Compression: {} (level {})",
            format,
            level.clamp(0, 22)
        );
        println!();
    }

    let options = velocity_compiler::BuildOptions {
        project_dir,
        output_path: output_path.clone(),
        compression_level: level.clamp(0, 22),
        compression_format: format,
        runtime_path: runtime.map(PathBuf::from),
        quiet,
    };

    let result = velocity_compiler::build_installer(&options).context("Build failed")?;

    if !quiet {
        println!();
        println!("  Build successful!");
        println!();
        println!("  Output:      {}", result.installer_path.display());
        println!(
            "  Size:        {} bytes",
            format_size(result.installer_size)
        );
        println!("  Files:       {}", result.file_count);
        println!(
            "  Payload:     {} (from {})",
            format_size(result.payload_size),
            format_size(result.original_size)
        );
        let ratio = if result.original_size > 0 {
            (1.0 - result.payload_size as f64 / result.original_size as f64) * 100.0
        } else {
            0.0
        };
        println!("  Compression: {:.1}%", ratio);
        println!();
    }

    Ok(())
}

/// Format a byte size to a human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
