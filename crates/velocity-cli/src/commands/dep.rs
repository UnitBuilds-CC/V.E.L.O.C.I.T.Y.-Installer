//! `velocity dep` — dependency management commands.
//!
//! Manage remote dependencies and bundled apps in the installer manifest.

use anyhow::{Context, Result};
use std::path::Path;
use velocity_config::VelocityManifest;

/// Run the `dep` command with the given subcommand.
pub fn run(subcommand: &str, config_path: &str, args: &[String]) -> Result<()> {
    match subcommand {
        "list" => run_list(config_path),
        "add" => run_add(config_path, args),
        "resolve" => run_resolve(config_path),
        "remove" => run_remove(config_path, args),
        _ => {
            println!("Unknown dep subcommand: {}", subcommand);
            println!("Available: list, add, resolve, remove");
            Ok(())
        }
    }
}

/// List all configured dependencies and bundled apps.
fn run_list(config_path: &str) -> Result<()> {
    let manifest = load_manifest(config_path)?;

    if manifest.dependencies.is_empty() && manifest.bundled_apps.is_empty() {
        println!("No dependencies or bundled apps configured.");
        return Ok(());
    }

    if !manifest.dependencies.is_empty() {
        println!("=== Remote Dependencies ===");
        println!();
        for (i, dep) in manifest.dependencies.iter().enumerate() {
            println!("  [{}] {}", i + 1, dep.name);
            println!("      URL:      {}", dep.url);
            println!("      Type:     {}", dep.file_type);
            println!("      Priority: {}", dep.priority);
            println!("      Required: {}", dep.required);
            println!("      Condition: {}", dep.condition);
            if !dep.install_args.is_empty() {
                println!("      Args:     {}", dep.install_args);
            }
            if let Some(ref hash) = dep.sha256 {
                println!("      SHA256:   {}...", &hash[..16.min(hash.len())]);
            }
            println!();
        }
    }

    if !manifest.bundled_apps.is_empty() {
        println!("=== Bundled Applications ===");
        println!();
        for (i, app) in manifest.bundled_apps.iter().enumerate() {
            println!("  [{}] {}", i + 1, app.name);
            println!("      Installer: {}", app.installer);
            println!("      Priority:  {}", app.priority);
            println!("      Required:  {}", app.required);
            println!("      Condition: {}", app.condition);
            if !app.install_args.is_empty() {
                println!("      Args:      {}", app.install_args);
            }
            println!();
        }
    }

    Ok(())
}

/// Add a new remote dependency to the manifest.
fn run_add(config_path: &str, args: &[String]) -> Result<()> {
    if args.is_empty() {
        println!("Usage: velocity dep add <name> <url> [options]");
        println!();
        println!("Options:");
        println!("  --type <exe|msi>       Installer type (default: exe)");
        println!("  --args <args>          Silent install arguments");
        println!("  --condition <cond>     Install condition (default: always)");
        println!("  --priority <n>         Install priority (default: 100)");
        println!("  --optional             Mark as optional (not required)");
        println!("  --bundled <path>       Add as bundled app instead of remote dep");
        println!();
        println!("Examples:");
        println!("  velocity dep add \"VC++ 2015-2022\" https://aka.ms/vs/17/release/vc_redist.x64.exe --args \"/install /quiet /norestart\"");
        println!("  velocity dep add \"DirectX\" https://download.microsoft.com/directx.exe --type exe --args \"/Q\" --condition \"file_missing:C:\\\\Windows\\\\System32\\\\d3d11.dll\"");
        println!("  velocity dep add \"7-Zip\" \"\" --bundled ./third-party/7z.exe --args \"/S\"");
        return Ok(());
    }

    let name = args[0].clone();
    let url = if args.len() > 1 { args[1].clone() } else { String::new() };

    // Parse options
    let mut file_type = "exe".to_string();
    let mut install_args = String::new();
    let mut condition = "always".to_string();
    let mut priority = 100u32;
    let mut required = true;
    let mut bundled_path: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--type" => {
                i += 1;
                if i < args.len() { file_type = args[i].clone(); }
            }
            "--args" => {
                i += 1;
                if i < args.len() { install_args = args[i].clone(); }
            }
            "--condition" => {
                i += 1;
                if i < args.len() { condition = args[i].clone(); }
            }
            "--priority" => {
                i += 1;
                if i < args.len() { priority = args[i].parse().unwrap_or(100); }
            }
            "--optional" => {
                required = false;
            }
            "--bundled" => {
                i += 1;
                if i < args.len() { bundled_path = Some(args[i].clone()); }
            }
            _ => {}
        }
        i += 1;
    }

    let mut manifest = load_manifest(config_path)?;

    if let Some(bpath) = bundled_path {
        // Add as bundled app
        let entry = velocity_config::BundledAppEntry {
            name: name.clone(),
            installer: bpath,
            install_args,
            condition,
            priority,
            required,
            working_dir: None,
        };
        manifest.bundled_apps.push(entry);
        println!("Added bundled app: {}", name);
    } else {
        // Add as remote dependency
        if url.is_empty() {
            anyhow::bail!("URL is required for remote dependencies. Use --bundled <path> for bundled apps.");
        }
        let entry = velocity_config::DependencyEntry {
            name: name.clone(),
            url,
            sha256: None,
            install_args,
            condition,
            priority,
            required,
            file_type,
        };
        manifest.dependencies.push(entry);
        println!("Added dependency: {}", name);
    }

    // Write back to TOML
    let toml_str = toml::to_string_pretty(&manifest)
        .context("Failed to serialize manifest")?;
    std::fs::write(config_path, toml_str)?;
    println!("Updated {}", config_path);

    Ok(())
}

/// Resolve dependencies — check which ones need to be installed.
fn run_resolve(config_path: &str) -> Result<()> {
    let manifest = load_manifest(config_path)?;

    if manifest.dependencies.is_empty() && manifest.bundled_apps.is_empty() {
        println!("No dependencies or bundled apps configured.");
        return Ok(());
    }

    println!("=== Dependency Resolution ===\n");

    let mut needs_install = 0;
    let mut already_satisfied = 0;

    if !manifest.dependencies.is_empty() {
        println!("Remote Dependencies:");
        for dep in &manifest.dependencies {
            let needed = velocity_core::dep_resolver::evaluate_condition(&dep.condition);
            let status = if needed { "NEEDED" } else { "SATISFIED" };
            let icon = if needed { "[!]" } else { "[OK]" };
            println!("  {} {} — {} (condition: {})", icon, dep.name, status, dep.condition);
            if needed {
                needs_install += 1;
            } else {
                already_satisfied += 1;
            }
        }
        println!();
    }

    if !manifest.bundled_apps.is_empty() {
        println!("Bundled Applications:");
        for app in &manifest.bundled_apps {
            let needed = velocity_core::dep_resolver::evaluate_condition(&app.condition);
            let status = if needed { "NEEDED" } else { "SATISFIED" };
            let icon = if needed { "[!]" } else { "[OK]" };
            println!("  {} {} — {} (condition: {})", icon, app.name, status, app.condition);
            if needed {
                needs_install += 1;
            } else {
                already_satisfied += 1;
            }
        }
        println!();
    }

    println!("Summary: {} need installation, {} already satisfied", needs_install, already_satisfied);

    Ok(())
}

/// Remove a dependency by name.
fn run_remove(config_path: &str, args: &[String]) -> Result<()> {
    if args.is_empty() {
        println!("Usage: velocity dep remove <name>");
        return Ok(());
    }

    let name = &args[0];
    let mut manifest = load_manifest(config_path)?;

    let orig_dep_len = manifest.dependencies.len();
    manifest.dependencies.retain(|d| d.name != *name);

    let orig_bundled_len = manifest.bundled_apps.len();
    manifest.bundled_apps.retain(|b| b.name != *name);

    if manifest.dependencies.len() < orig_dep_len {
        println!("Removed dependency: {}", name);
    } else if manifest.bundled_apps.len() < orig_bundled_len {
        println!("Removed bundled app: {}", name);
    } else {
        println!("No dependency or bundled app named '{}' found.", name);
        return Ok(());
    }

    // Write back to TOML
    let toml_str = toml::to_string_pretty(&manifest)
        .context("Failed to serialize manifest")?;
    std::fs::write(config_path, toml_str)?;
    println!("Updated {}", config_path);

    Ok(())
}

/// Load and parse the manifest from a config path.
fn load_manifest(config_path: &str) -> Result<VelocityManifest> {
    let path = Path::new(config_path);
    if !path.exists() {
        anyhow::bail!("Config file not found: {}", config_path);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", config_path))?;
    let manifest: VelocityManifest = velocity_config::parse_manifest_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path))?;
    Ok(manifest)
}
