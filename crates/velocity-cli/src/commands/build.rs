//! `velocity build` — Build an installer from the current project.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Run the build command.
pub fn run(
    output: Option<String>,
    compression: i32,
    compression_format: Option<String>,
    runtime: Option<String>,
    delta: bool,
    quiet: bool,
) -> Result<()> {
    let project_dir = std::env::current_dir().context("Failed to get current directory")?;

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.join("output").join("installer.exe"));

    // Load config to get compression settings and version info
    let config_path = project_dir.join("velocity.toml");
    let (format, level, current_version) = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).context("Failed to read velocity.toml")?;
        let manifest: velocity_config::VelocityManifest = toml::from_str(&content).context("Failed to parse velocity.toml")?;
        
        // CLI flags override config
        let format = compression_format.unwrap_or(manifest.files.compression.format);
        let level = if compression != 3 { compression } else { manifest.files.compression.level };
        let version = manifest.app.version.clone();
        (format, level, version)
    } else {
        // No config, use CLI flags or defaults
        let format = compression_format.unwrap_or_else(|| "zstd".to_string());
        (format, compression, "0.0.0".to_string())
    };

    if !quiet {
        println!();
        println!("  Building Velocity installer...");
        println!(
            "  Compression: {} (level {})",
            format,
            level.clamp(0, 22)
        );
        if delta {
            println!("  Delta updates: enabled");
        }
        println!();
    }

    let options = velocity_compiler::BuildOptions {
        project_dir: project_dir.clone(),
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
    }

    // Generate delta if requested
    if delta {
        generate_delta_update(&project_dir, &output_path, &current_version, quiet)?;
    }

    if !quiet {
        println!();
    }

    Ok(())
}

/// Generate a delta update package by comparing with the previous version.
fn generate_delta_update(
    project_dir: &std::path::Path,
    output_path: &std::path::Path,
    current_version: &str,
    quiet: bool,
) -> Result<()> {
    use velocity_core::delta::{generate_delta, save_delta_package, DeltaOptions};

    // Look for previous version in output directory
    let output_dir = output_path.parent().unwrap_or(project_dir);
    
    // Find the most recent previous installer
    let mut previous_installers: Vec<_> = std::fs::read_dir(output_dir)
        .context("Failed to read output directory")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.extension().map_or(false, |ext| ext == "exe")
                && path.file_stem().map_or(false, |stem| {
                    stem.to_string_lossy().contains("installer")
                        || stem.to_string_lossy().contains("sample-app")
                })
                && path != *output_path
        })
        .collect();

    if previous_installers.is_empty() {
        if !quiet {
            println!();
            println!("  Delta: No previous version found in output directory");
            println!("         Delta generation requires a previous installer in the same directory");
        }
        return Ok(());
    }

    // Sort by modification time (newest first)
    previous_installers.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    let previous_installer = previous_installers[0].path();
    
    if !quiet {
        println!();
        println!("  Generating delta update...");
        println!("    Previous: {}", previous_installer.display());
        println!("    Current:  v{}", current_version);
    }

    // Extract both installers to temporary directories
    let temp_dir = project_dir.join("target").join("delta-temp");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    let old_extract_dir = temp_dir.join("old");
    let new_extract_dir = temp_dir.join("new");

    // Extract previous installer
    extract_installer_payload(&previous_installer, &old_extract_dir)?;

    // Extract current installer
    extract_installer_payload(output_path, &new_extract_dir)?;

    // Determine previous version from filename or use "unknown"
    let previous_version = previous_installer
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| {
            // Try to extract version from filename like "installer-1.0.0"
            s.rsplit('-').next()
        })
        .unwrap_or("unknown");

    // Generate delta
    let delta = generate_delta(
        &old_extract_dir,
        &new_extract_dir,
        previous_version,
        current_version,
        &DeltaOptions::default(),
    )?;

    // Save delta package
    let delta_path = output_dir.join(format!(
        "{}-delta.zip",
        output_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    save_delta_package(&delta, &delta_path)?;

    // Cleanup temp directories
    std::fs::remove_dir_all(&temp_dir)?;

    if !quiet {
        println!("    Delta:     {}", delta_path.display());
        println!(
            "    Delta size: {} ({} patches)",
            format_size(delta.total_patch_size),
            delta.patches.len()
        );
        
        // Calculate size reduction
        let current_size = std::fs::metadata(output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if current_size > 0 {
            let reduction = (1.0 - delta.total_patch_size as f64 / current_size as f64) * 100.0;
            println!("    Reduction:  {:.1}% smaller than full package", reduction);
        }
    }

    Ok(())
}

/// Extract the payload from an installer to a directory.
fn extract_installer_payload(installer_path: &std::path::Path, extract_dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(extract_dir)?;

    // For delta generation, we need to extract the files
    // This is a simplified placeholder - in production, we'd parse the installer format
    // and extract the actual payload files
    
    // Create a marker file to indicate extraction
    std::fs::write(
        extract_dir.join(".extracted"),
        format!("Extracted from: {}", installer_path.display()),
    )?;

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
