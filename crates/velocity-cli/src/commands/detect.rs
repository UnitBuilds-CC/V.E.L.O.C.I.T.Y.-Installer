//! `velocity detect` — Auto-detect project settings and generate velocity.toml.

use anyhow::{Context, Result};

/// Run the detect command.
pub fn run(dir: &str) -> Result<()> {
    let project_dir = std::path::PathBuf::from(dir);
    let project_dir = if project_dir.is_absolute() {
        project_dir
    } else {
        std::env::current_dir()?.join(project_dir)
    };

    println!();
    println!("  Scanning project: {}", project_dir.display());
    println!();

    let manifest = velocity_config::auto_generate(&project_dir)
        .context("Auto-detection failed")?;

    println!("  Detected settings:");
    println!("    Name:      {}", manifest.app.name);
    println!("    Version:   {}", manifest.app.version);
    println!("    Publisher: {}", if manifest.app.publisher.is_empty() { "(not detected)" } else { &manifest.app.publisher });
    println!("    Icon:      {}", manifest.app.icon.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(not found)".to_string()));
    println!("    Files:     {} pattern(s)", manifest.files.source.len());
    println!();

    // Write the generated manifest
    let toml_path = project_dir.join("velocity.toml");
    velocity_config::write_manifest(&manifest, &toml_path)
        .context("Failed to write velocity.toml")?;

    println!("  Generated velocity.toml");
    println!();
    println!("  Review and edit velocity.toml before running `velocity build`.");
    println!();

    Ok(())
}
