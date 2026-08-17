//! Cross-platform terminal wizard for Linux and macOS.
//!
//! Provides an interactive terminal-based installer wizard that works
//! on all Unix platforms. On Windows, the GUI wizards are used instead.

use crate::error::{Result, UiError};
use crate::wizard::InstallWizardResult;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use tracing::info;
use velocity_config::VelocityManifest;

/// Run the terminal-based wizard on non-Windows platforms.
pub fn run_terminal_wizard(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║  {} {} Setup", manifest.app.name, manifest.app.version);
    if !manifest.app.publisher.is_empty() {
        println!("║  Publisher: {}", manifest.app.publisher);
    }
    println!("╚══════════════════════════════════════════╝");
    println!();

    // Page 1: License agreement
    if let Some(ref license_path) = manifest.app.license {
        if let Ok(license_text) = std::fs::read_to_string(license_path) {
            println!("─── License Agreement ───");
            // Show first 30 lines of the license
            for (i, line) in license_text.lines().enumerate() {
                if i >= 30 {
                    println!("  ... (truncated, see {} for full text)", license_path);
                    break;
                }
                println!("  {}", line);
            }
            println!();
            if !prompt_yes_no("Do you accept the license agreement?", true)? {
                println!("License not accepted. Installation cancelled.");
                return Err(UiError::Cancelled);
            }
            println!();
        }
    }

    // Page 2: Install directory
    let default_dir = velocity_core::platform::default_install_dir(&manifest.app.name);
    let default_dir_str = default_dir.to_string_lossy().to_string();
    println!("─── Installation Directory ───");
    println!("  Default: {}", default_dir_str);
    print!("  Install directory [{}]: ", default_dir_str);
    io::stdout().flush()?;

    let install_dir = read_line()?;
    let install_dir = if install_dir.trim().is_empty() {
        PathBuf::from(&default_dir_str)
    } else {
        PathBuf::from(install_dir.trim())
    };

    // Create the directory if it doesn't exist
    if !install_dir.exists() {
        match std::fs::create_dir_all(&install_dir) {
            Ok(_) => info!("Created install directory: {}", install_dir.display()),
            Err(e) => {
                return Err(UiError::Other(format!(
                    "Failed to create install directory {}: {}",
                    install_dir.display(),
                    e
                )));
            }
        }
    }
    println!("  Using: {}", install_dir.display());
    println!();

    // Page 3: Component selection
    let mut selected_components = Vec::new();
    if !manifest.components.is_empty() {
        println!("─── Component Selection ───");
        for (i, comp) in manifest.components.iter().enumerate() {
            let size_mb = comp.size as f64 / (1024.0 * 1024.0);
            let default_marker = if comp.selected_by_default { "*" } else { " " };
            let mandatory_marker = if comp.mandatory { "!" } else { default_marker };
            println!(
                "  [{}] {}. {} ({:.1} MB){}",
                mandatory_marker,
                i + 1,
                comp.name,
                size_mb,
                if comp.mandatory { " [required]" } else { "" }
            );
            if let Some(ref desc) = comp.description {
                println!("       {}", desc);
            }
        }
        println!();
        println!("  * = selected by default, ! = required");

        if manifest.components.iter().any(|c| !c.mandatory) {
            print!("  Select components [1,2,3 or 'all'/'default']: ");
            io::stdout().flush()?;
            let selection = read_line()?;

            // Always include mandatory components
            for comp in &manifest.components {
                if comp.mandatory {
                    selected_components.push(comp.id.clone());
                }
            }

            let selection = selection.trim().to_lowercase();
            if selection == "all" {
                for comp in &manifest.components {
                    if !selected_components.contains(&comp.id) {
                        selected_components.push(comp.id.clone());
                    }
                }
            } else if selection == "default" || selection.is_empty() {
                for comp in &manifest.components {
                    if comp.selected_by_default && !selected_components.contains(&comp.id) {
                        selected_components.push(comp.id.clone());
                    }
                }
            } else {
                // Parse comma-separated or space-separated numbers
                for part in selection.split(|c: char| c == ',' || c == ' ') {
                    if let Ok(n) = part.parse::<usize>() {
                        if n >= 1 && n <= manifest.components.len() {
                            let comp = &manifest.components[n - 1];
                            if !selected_components.contains(&comp.id) {
                                selected_components.push(comp.id.clone());
                            }
                        }
                    }
                }
            }
        } else {
            // All components are mandatory
            for comp in &manifest.components {
                selected_components.push(comp.id.clone());
            }
        }
        println!("  Selected: {} component(s)", selected_components.len());
        println!();
    }

    // Page 4: Confirm and install
    println!("─── Ready to Install ───");
    println!(
        "  Application: {} {}",
        manifest.app.name, manifest.app.version
    );
    println!("  Directory:   {}", install_dir.display());
    println!("  Components:  {}", selected_components.len());
    println!();

    if !prompt_yes_no("Proceed with installation?", true)? {
        println!("Installation cancelled.");
        return Err(UiError::Cancelled);
    }

    println!();
    println!("─── Installing ───");

    // Page 5: Launch after install
    let launch_after = prompt_yes_no("Launch application after installation?", false)?;

    Ok(InstallWizardResult {
        install_dir,
        cancelled: false,
        launch_after,
        selected_components,
        install_completed: false,
    })
}

/// Prompt the user for a yes/no answer.
fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("  {} {}: ", prompt, hint);
    io::stdout().flush()?;

    let input = read_line()?;
    let trimmed = input.trim().to_lowercase();

    if trimmed.is_empty() {
        Ok(default)
    } else {
        Ok(trimmed == "y" || trimmed == "yes")
    }
}

/// Read a line from stdin.
fn read_line() -> Result<String> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| UiError::Other(format!("Failed to read input: {}", e)))?;
    Ok(line)
}

/// Show a message dialog (terminal version).
pub fn show_message(title: &str, message: &str) {
    println!();
    println!("─── {} ───", title);
    println!("{}", message);
    println!();
}

/// Show an error dialog (terminal version).
pub fn show_error(title: &str, message: &str) {
    eprintln!();
    eprintln!("ERROR: {}", title);
    eprintln!("{}", message);
    eprintln!();
}

/// Show installation complete message.
pub fn show_complete(app_name: &str, install_dir: &std::path::Path) {
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║  {} has been installed!", app_name);
    println!("║  Location: {}", install_dir.display());
    println!("╚══════════════════════════════════════════╝");
    println!();
}

/// Show finish dialog with option to launch.
pub fn show_finish(app_name: &str, install_dir: &std::path::Path, run_after: Option<&str>) -> bool {
    show_complete(app_name, install_dir);
    if let Some(exe) = run_after {
        prompt_yes_no(&format!("Launch {} now?", app_name), true).unwrap_or(false)
    } else {
        true
    }
}
