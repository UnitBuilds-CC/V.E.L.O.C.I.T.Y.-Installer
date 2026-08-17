//! `velocity check` — Validate a velocity.toml configuration.

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

    match velocity_config::parse_manifest(&path) {
        Ok(manifest) => {
            println!("  Configuration is valid!");
            println!();
            println!("  App:     {} v{}", manifest.app.name, manifest.app.version);
            println!("  Theme:   {}", manifest.ui.theme);
            println!("  Arch:    {}", manifest.install.arch);
            println!("  Admin:   {}", manifest.install.require_admin);
            println!("  Files:   {} source pattern(s)", manifest.files.source.len());
            println!("  Registry: {} entries", manifest.registry.len());
            println!("  Services: {}", manifest.services.len());
            println!("  Env vars: {}", manifest.env_vars.len());
            println!();
            Ok(())
        }
        Err(e) => {
            println!("  Configuration errors found:");
            println!();
            println!("  {}", e);
            println!();
            anyhow::bail!("Configuration validation failed");
        }
    }
}
