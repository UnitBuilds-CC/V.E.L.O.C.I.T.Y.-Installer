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
                    println!("    Name:        {}", manifest.app.name);
                    println!("    Version:     {}", manifest.app.version);
                    println!(
                        "    Publisher:   {}",
                        if manifest.app.publisher.is_empty() {
                            "(not set)"
                        } else {
                            &manifest.app.publisher
                        }
                    );
                    if let Some(ref desc) = manifest.app.description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref url) = manifest.app.url {
                        println!("    Website:     {}", url);
                    }
                    if let Some(ref id) = manifest.app.id {
                        println!("    ID:          {}", id);
                    }
                    println!();

                    println!("  Installation:");
                    println!("    Default dir: {}", manifest.install.default_dir);
                    println!("    Arch:        {}", manifest.install.arch);
                    println!("    Admin:       {}", manifest.install.require_admin);
                    println!("    Theme:       {}", manifest.ui.theme);
                    if let Some(ref exe) = manifest.install.run_after_install {
                        println!("    Run after:   {}", exe);
                    }
                    println!();

                    println!("  Contents:");
                    println!(
                        "    Files:       {} pattern(s)",
                        manifest.files.source.len()
                    );
                    if !manifest.files.exclude.is_empty() {
                        println!("    Excludes:    {}", manifest.files.exclude.join(", "));
                    }
                    println!(
                        "    Shortcuts:   desktop={}, start_menu={}",
                        manifest.shortcuts.desktop, manifest.shortcuts.start_menu
                    );
                    println!("    Registry:    {} entries", manifest.registry.len());
                    println!("    Env vars:    {} entries", manifest.env_vars.len());
                    println!("    Services:    {} entries", manifest.services.len());
                    println!(
                        "    File assoc:  {} entries",
                        manifest.file_associations.len()
                    );
                    if !manifest.scripts.pre_install.is_empty() {
                        println!(
                            "    Pre-install: {} command(s)",
                            manifest.scripts.pre_install.len()
                        );
                    }
                    if !manifest.scripts.post_install.is_empty() {
                        println!(
                            "    Post-install:{} command(s)",
                            manifest.scripts.post_install.len()
                        );
                    }
                    println!();

                    println!("  Uninstaller:");
                    println!("    Add/Remove:  {}", manifest.uninstall.add_remove);
                    if let Some(ref name) = manifest.uninstall.display_name {
                        println!("    Display:     {}", name);
                    }
                    println!();

                    println!("  Payload:");
                    println!(
                        "    Compressed:  {}",
                        format_size(payload_data.len() as u64)
                    );
                    println!();
                }
                Err(e) => {
                    println!("  Warning: Could not parse manifest: {}", e);
                    println!("  Manifest size: {} bytes", manifest_data.len());
                    println!();
                }
            }

            // Show exe info
            let exe_size = std::fs::metadata(&exe_path)?.len();
            let base_size = velocity_core::payload::get_base_exe_size(&exe_path).unwrap_or(0);

            println!("  File info:");
            println!(
                "    Total size:    {} ({})",
                format_size(exe_size),
                exe_size
            );
            println!(
                "    Runtime size:  {} ({})",
                format_size(base_size),
                base_size
            );
            println!(
                "    Payload size:  {} ({})",
                format_size(payload_data.len() as u64),
                payload_data.len()
            );
            if base_size > 0 {
                let ratio = (payload_data.len() as f64 / exe_size as f64) * 100.0;
                println!("    Payload ratio: {:.1}% of total", ratio);
            }
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
