//! `velocity info` — Show information about a built installer.

use anyhow::Result;

/// Run the info command.
pub fn run(path: &str) -> Result<()> {
    let exe_path = std::path::PathBuf::from(path);
    let exe_path = if exe_path.is_absolute() {
        exe_path
    } else {
        std::env::current_dir()?.join(exe_path)
    };

    if !exe_path.exists() {
        anyhow::bail!("Installer not found: {}", exe_path.display());
    }

    println!();
    println!("  Installer: {}", exe_path.display());
    println!();

    // Read the payload
    match velocity_core::payload::read_payload(&exe_path) {
        Ok((manifest_data, payload_data)) => {
            // Parse manifest
            match serde_json::from_slice::<velocity_config::VelocityManifest>(&manifest_data) {
                Ok(manifest) => {
                    println!("  Application:");
                    println!("    Name:      {}", manifest.app.name);
                    println!("    Version:   {}", manifest.app.version);
                    println!("    Publisher: {}", manifest.app.publisher);
                    println!();
                    println!("  Installer:");
                    println!("    Theme:     {}", manifest.ui.theme);
                    println!("    Arch:      {}", manifest.install.arch);
                    println!("    Admin:     {}", manifest.install.require_admin);
                    println!();
                    println!("  Payload:");
                    println!("    Size:      {} bytes", format_size(payload_data.len() as u64));
                    println!();
                }
                Err(e) => {
                    println!("  Warning: Could not parse manifest: {}", e);
                    println!("  Manifest size: {} bytes", manifest_data.len());
                }
            }

            // Show exe info
            let exe_size = std::fs::metadata(&exe_path)?.len();
            let base_size = velocity_core::payload::get_base_exe_size(&exe_path)
                .unwrap_or(0);

            println!("  File info:");
            println!("    Total size:    {} bytes ({})",
                exe_size, format_size(exe_size));
            println!("    Runtime size:  {} bytes ({})",
                base_size, format_size(base_size));
            println!("    Payload size:  {} bytes ({})",
                payload_data.len(), format_size(payload_data.len() as u64));
            println!();
        }
        Err(e) => {
            println!("  Not a Velocity installer or payload is corrupt.");
            println!("  Error: {}", e);
            println!();
        }
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
