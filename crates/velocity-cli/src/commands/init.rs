//! `velocity init` — Initialize a new Velocity installer project.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::info;

/// Run the init command.
pub fn run(name: Option<String>, minimal: bool) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-app".to_string())
    });

    let project_dir = if Path::new(&project_name).is_absolute() {
        PathBuf::from(&project_name)
    } else {
        std::env::current_dir()?.join(&project_name)
    };

    info!("Initializing Velocity project: {}", project_dir.display());

    // Create project directory
    std::fs::create_dir_all(&project_dir)
        .context("Failed to create project directory")?;

    if minimal {
        create_minimal_project(&project_dir)?;
    } else {
        create_project(&project_dir, &project_name)?;
    }

    println!();
    println!("  Velocity project initialized!");
    println!();
    println!("  Project: {}", project_dir.display());
    println!();
    println!("  Next steps:");
    println!("    cd {}", project_name);
    println!("    # Edit velocity.toml to configure your installer");
    println!("    velocity build");
    println!();

    Ok(())
}

/// Create a full project with auto-detection.
fn create_project(project_dir: &Path, name: &str) -> Result<()> {
    // Try to auto-generate from existing files
    let manifest = match velocity_config::auto_generate(project_dir) {
        Ok(mut m) => {
            // Override name if provided
            if name != "." {
                m.app.name = velocity_config::to_title_case(name);
            }
            println!("  Auto-detected project settings from existing files.");
            m
        }
        Err(_) => {
            // Create default manifest
            create_default_manifest(name)
        }
    };

    // Write velocity.toml
    let toml_path = project_dir.join("velocity.toml");
    velocity_config::write_manifest(&manifest, &toml_path)
        .context("Failed to write velocity.toml")?;
    println!("  Created velocity.toml");

    // Create assets directory
    let assets_dir = project_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    // Create a placeholder icon note
    let readme_path = assets_dir.join("README.txt");
    std::fs::write(
        &readme_path,
        "Place your application icon here as icon.ico (256x256 recommended).\n",
    )?;

    // Create output directory
    let output_dir = project_dir.join("output");
    std::fs::create_dir_all(&output_dir)?;

    Ok(())
}

/// Create a minimal project (no auto-detection).
fn create_minimal_project(project_dir: &Path) -> Result<()> {
    let name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-app".to_string());

    let manifest = create_default_manifest(&name);

    let toml_path = project_dir.join("velocity.toml");
    velocity_config::write_manifest(&manifest, &toml_path)?;
    println!("  Created velocity.toml (minimal)");

    let output_dir = project_dir.join("output");
    std::fs::create_dir_all(&output_dir)?;

    Ok(())
}

/// Create a default manifest for a new project.
fn create_default_manifest(name: &str) -> velocity_config::VelocityManifest {
    let title_name = to_title_case(name);

    velocity_config::VelocityManifest {
        app: velocity_config::AppConfig {
            name: title_name.clone(),
            version: "1.0.0".to_string(),
            publisher: String::new(),
            icon: None,
            url: None,
            id: None,
            license: None,
            description: None,
        },
        install: velocity_config::InstallConfig {
            default_dir: format!("{{autopf}}/{}", title_name),
            start_menu: Some(title_name),
            ..Default::default()
        },
        files: velocity_config::FilesConfig {
            source: vec!["./**".to_string()],
            base_dir: None,
            mappings: Vec::new(),
            exclude: vec![
                "*.pdb".to_string(),
                "*.tmp".to_string(),
                "velocity.toml".to_string(),
                "output/**".to_string(),
            ],
        },
        shortcuts: velocity_config::ShortcutConfig {
            desktop: true,
            start_menu: true,
            ..Default::default()
        },
        registry: Vec::new(),
        uninstall: velocity_config::UninstallConfig::default(),
        ui: velocity_config::UiConfig::default(),
        pages: Vec::new(),
        scripts: velocity_config::ScriptsConfig::default(),
        env_vars: Vec::new(),
        services: Vec::new(),
        file_associations: Vec::new(),
    }
}

/// Convert a string to Title Case.
fn to_title_case(input: &str) -> String {
    input
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{}{}", upper, chars.collect::<String>())
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
