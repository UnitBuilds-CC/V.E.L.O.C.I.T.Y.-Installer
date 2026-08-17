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

    // Generate a default 16x16 icon
    let icon_path = assets_dir.join("icon.ico");
    let icon_data = generate_default_icon();
    std::fs::write(&icon_path, &icon_data)?;

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
        dependencies: Vec::new(),
        bundled_apps: Vec::new(),
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

/// Generate a minimal valid 16x16 32-bit ICO file with a blue gradient.
fn generate_default_icon() -> Vec<u8> {
    let width: u32 = 16;
    let height: u32 = 16;
    let bpp: u16 = 32;

    // BMP info header (BITMAPINFOHEADER)
    let header_size = 40u32;
    let pixel_data_size = width * height * 4; // 1024 bytes
    let image_data_size = header_size + pixel_data_size; // 1064 bytes

    // ICO file structure:
    // ICONDIR (6 bytes) + ICONDIRENTRY (16 bytes) + image data
    let mut ico = Vec::with_capacity(6 + 16 + image_data_size as usize);

    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Type: icon
    ico.extend_from_slice(&1u16.to_le_bytes()); // Count: 1 image

    // ICONDIRENTRY
    ico.push(width as u8);   // Width (16)
    ico.push(height as u8);  // Height (16)
    ico.push(0);             // Color count (0 = >= 256)
    ico.push(0);             // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Color planes
    ico.extend_from_slice(&bpp.to_le_bytes());  // Bits per pixel
    ico.extend_from_slice(&image_data_size.to_le_bytes()); // Size of image data
    ico.extend_from_slice(&(6u32 + 16u32).to_le_bytes()); // Offset to image data

    // BITMAPINFOHEADER
    ico.extend_from_slice(&header_size.to_le_bytes()); // biSize
    ico.extend_from_slice(&width.to_le_bytes());       // biWidth
    ico.extend_from_slice(&(height * 2).to_le_bytes()); // biHeight (2x for ICO)
    ico.extend_from_slice(&1u16.to_le_bytes());        // biPlanes
    ico.extend_from_slice(&bpp.to_le_bytes());         // biBitCount
    ico.extend_from_slice(&0u32.to_le_bytes());        // biCompression (BI_RGB)
    ico.extend_from_slice(&pixel_data_size.to_le_bytes()); // biSizeImage
    ico.extend_from_slice(&0u32.to_le_bytes());        // biXPelsPerMeter
    ico.extend_from_slice(&0u32.to_le_bytes());        // biYPelsPerMeter
    ico.extend_from_slice(&0u32.to_le_bytes());        // biClrUsed
    ico.extend_from_slice(&0u32.to_le_bytes());        // biClrImportant

    // Pixel data (BGRA, bottom-up)
    // Create a blue gradient with a "V" shape for Velocity
    for y in 0..height {
        for x in 0..width {
            let fy = y as f32 / (height - 1) as f32;
            let fx = x as f32 / (width - 1) as f32;

            // Blue gradient background (#0078D4 -> #005A9E)
            let r = (0.0 + fy * 0.0) as u8;
            let g = (0x78 - (fy * 30.0) as u8).max(0x40);
            let b = (0xD4 - (fy * 20.0) as u8).max(0x80);

            // Draw a white "V" shape
            let in_v = {
                let cx = width as f32 / 2.0;
                let left_line = (fx * height as f32) < (cx - 1.0) * (1.0 - fy) + 2.0;
                let right_line = ((width as f32 - 1.0 - fx) * height as f32) < (cx - 1.0) * (1.0 - fy) + 2.0;
                let center_check = fy > 0.2 && fy < 0.9;
                center_check && (left_line || right_line)
            };

            let (br, bg, bb, ba) = if in_v {
                (255, 255, 255, 255) // White V
            } else {
                (r, g, b, 255) // Blue background
            };

            ico.push(bb); // B
            ico.push(bg); // G
            ico.push(br); // R
            ico.push(ba); // A
        }
    }

    ico
}
