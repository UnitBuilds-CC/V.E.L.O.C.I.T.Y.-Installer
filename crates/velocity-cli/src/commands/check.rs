//! `velocity check` — Validate a velocity.toml configuration.
//!
//! Performs deep validation including:
//! - Manifest parsing
//! - Source file existence
//! - Path variable validity
//! - Registry root validity
//! - Service binary path checks
//! - File association extension format

use anyhow::Result;

/// Run the check command.
pub fn run(config_path: &str) -> Result<()> {
    let path = std::path::PathBuf::from(config_path);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };

    println!();
    println!("  Checking: {}", path.display());
    println!();

    if !path.exists() {
        anyhow::bail!("Configuration file not found: {}", path.display());
    }

    // Step 1: Parse the manifest
    let manifest = match velocity_config::parse_manifest(&path) {
        Ok(m) => m,
        Err(e) => {
            println!("  Configuration errors found:");
            println!();
            println!("  {}", e);
            println!();
            anyhow::bail!("Configuration validation failed");
        }
    };

    println!("  Configuration is valid!");
    println!();
    println!("  App:      {} v{}", manifest.app.name, manifest.app.version);
    println!("  Publisher: {}", if manifest.app.publisher.is_empty() { "(not set)" } else { &manifest.app.publisher });
    println!("  Theme:    {}", manifest.ui.theme);
    println!("  Arch:     {}", manifest.install.arch);
    println!("  Admin:    {}", manifest.install.require_admin);
    println!();

    // Step 2: Validate path variables in default_dir
    let project_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // Check that variables in default_dir are known
    let vars = velocity_config::extract_variables(&manifest.install.default_dir);
    let known_vars = [
        "app", "autopf", "autopf64", "autopf32", "commonstartup",
        "autodesktop", "autostartmenu", "autoprograms", "win", "sys",
        "tmp", "src", "home", "group", "version",
    ];
    for var in &vars {
        if !known_vars.contains(&var.as_str()) {
            warnings.push(format!("Unknown variable '{{{}}}' in default_dir", var));
        }
    }

    // Step 3: Check source files exist
    match velocity_config::collect_files(&manifest, project_dir) {
        Ok(files) => {
            if files.is_empty() {
                errors.push("No files found matching source patterns".to_string());
            } else {
                let total_size: u64 = files.iter()
                    .filter_map(|(p, _)| std::fs::metadata(p).ok().map(|m| m.len()))
                    .sum();
                println!("  Files:    {} file(s), {} total",
                    files.len(),
                    format_size(total_size));
            }
        }
        Err(e) => {
            errors.push(format!("File collection error: {}", e));
        }
    }

    // Step 4: Validate registry entries
    for (i, entry) in manifest.registry.iter().enumerate() {
        let valid_roots = ["HKLM", "HKCU", "HKCR", "HKU"];
        if !valid_roots.contains(&entry.root.as_str()) {
            errors.push(format!(
                "Registry entry #{}: invalid root '{}' (must be one of: HKLM, HKCU, HKCR, HKU)",
                i + 1, entry.root
            ));
        }
        if entry.key.is_empty() {
            errors.push(format!("Registry entry #{}: key path is empty", i + 1));
        }
    }
    println!("  Registry: {} entries", manifest.registry.len());

    // Step 5: Validate services
    for (i, svc) in manifest.services.iter().enumerate() {
        if svc.name.is_empty() {
            errors.push(format!("Service #{}: name is empty", i + 1));
        }
        if svc.binary_path.is_empty() {
            errors.push(format!("Service #{}: binary_path is empty", i + 1));
        }
        let valid_start_types = ["auto", "manual", "disabled", "delayed_auto"];
        if !valid_start_types.contains(&svc.start_type.as_str()) {
            warnings.push(format!(
                "Service '{}': unknown start_type '{}' (valid: auto, manual, disabled, delayed_auto)",
                svc.name, svc.start_type
            ));
        }
    }
    println!("  Services: {} entries", manifest.services.len());

    // Step 6: Validate env vars
    for (i, var) in manifest.env_vars.iter().enumerate() {
        if var.name.is_empty() {
            errors.push(format!("Env var #{}: name is empty", i + 1));
        }
        let valid_scopes = ["system", "user"];
        if !valid_scopes.contains(&var.scope.as_str()) {
            warnings.push(format!(
                "Env var '{}': unknown scope '{}' (valid: system, user)",
                var.name, var.scope
            ));
        }
    }
    println!("  Env vars: {} entries", manifest.env_vars.len());

    // Step 7: Validate file associations
    for (i, assoc) in manifest.file_associations.iter().enumerate() {
        if !assoc.extension.starts_with('.') && !assoc.extension.chars().all(|c| c.is_alphanumeric()) {
            warnings.push(format!(
                "File association #{}: extension '{}' may need a leading dot",
                i + 1, assoc.extension
            ));
        }
        if assoc.handler.is_empty() {
            errors.push(format!("File association #{}: handler is empty", i + 1));
        }
    }
    println!("  Assoc:    {} entries", manifest.file_associations.len());

    // Step 8: Check icon file if specified
    if let Some(ref icon_path) = manifest.app.icon {
        let full_icon = project_dir.join(icon_path);
        if !full_icon.exists() {
            warnings.push(format!("Icon file not found: {}", full_icon.display()));
        }
    }

    // Step 9: Check license file if specified
    if let Some(ref license_path) = manifest.app.license {
        let full_license = project_dir.join(license_path);
        if !full_license.exists() {
            warnings.push(format!("License file not found: {}", full_license.display()));
        }
    }

    println!();

    // Report warnings
    if !warnings.is_empty() {
        println!("  Warnings ({}):", warnings.len());
        for w in &warnings {
            println!("    ⚠ {}", w);
        }
        println!();
    }

    // Report errors
    if !errors.is_empty() {
        println!("  Errors ({}):", errors.len());
        for e in &errors {
            println!("    ✗ {}", e);
        }
        println!();
        anyhow::bail!("Configuration validation failed with {} error(s)", errors.len());
    }

    if warnings.is_empty() {
        println!("  All checks passed. Ready to build!");
    } else {
        println!("  Configuration is valid (with {} warning(s)).", warnings.len());
    }
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
