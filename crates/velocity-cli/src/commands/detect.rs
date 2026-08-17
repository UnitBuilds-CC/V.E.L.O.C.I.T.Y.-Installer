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

    if !project_dir.exists() {
        anyhow::bail!("Project directory not found: {}", project_dir.display());
    }

    let manifest = velocity_config::auto_generate(&project_dir).context("Auto-detection failed")?;

    println!("  Detected settings:");
    println!();
    println!("  Application:");
    println!("    Name:      {}", manifest.app.name);
    println!("    Version:   {}", manifest.app.version);
    println!(
        "    Publisher: {}",
        if manifest.app.publisher.is_empty() {
            "(not detected)"
        } else {
            &manifest.app.publisher
        }
    );
    println!(
        "    Icon:      {}",
        manifest
            .app
            .icon
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not found)".to_string())
    );
    println!();

    println!("  Installation:");
    println!("    Default dir: {}", manifest.install.default_dir);
    println!("    Arch:        {}", manifest.install.arch);
    println!("    Admin:       {}", manifest.install.require_admin);
    if let Some(ref exe) = manifest.install.run_after_install {
        println!("    Run after:   {}", exe);
    }
    println!();

    println!("  Files:");
    println!("    Source patterns: {}", manifest.files.source.len());
    for pattern in &manifest.files.source {
        println!("      - {}", pattern);
    }
    if !manifest.files.exclude.is_empty() {
        println!("    Exclusions:");
        for exc in &manifest.files.exclude {
            println!("      - {}", exc);
        }
    }
    println!();

    println!("  Shortcuts:");
    println!("    Desktop:    {}", manifest.shortcuts.desktop);
    println!("    Start Menu: {}", manifest.shortcuts.start_menu);
    if !manifest.shortcuts.custom.is_empty() {
        println!(
            "    Custom:     {} shortcut(s)",
            manifest.shortcuts.custom.len()
        );
    }
    println!();

    // Scan for actual files that would be included
    match velocity_config::collect_files(&manifest, &project_dir) {
        Ok(files) => {
            println!(
                "  Detected files: {} file(s) would be packaged",
                files.len()
            );
            if !files.is_empty() {
                let total_size: u64 = files
                    .iter()
                    .filter_map(|(path, _)| std::fs::metadata(path).ok().map(|m| m.len()))
                    .sum();
                println!("  Total size:     {}", format_size(total_size));
            }
            println!();
        }
        Err(e) => {
            println!("  File detection warning: {}", e);
            println!();
        }
    }

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
