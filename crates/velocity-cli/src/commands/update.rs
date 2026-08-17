//! Self-update command — checks GitHub releases for newer versions and downloads updates.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// Default GitHub repository for update checks.
const GITHUB_REPO: &str = "UnitBuilds/velocity-installer";

/// Check for updates and optionally install them.
pub fn run_check() -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("Velocity CLI v{}", current);
    println!("Checking for updates...");

    match get_latest_release(GITHUB_REPO) {
        Ok(release) => {
            println!("Latest version: {}", release.version);

            if velocity_core::updater::is_newer_version(&release.version, current) {
                println!("A new version is available!");
                if let Some(notes) = &release.release_notes {
                    println!("\nRelease notes:\n{}", notes);
                }
                println!("\nDownload: {}", release.download_url);
                println!("\nRun `velocity update` to install the update.");
            } else {
                println!("You are up to date.");
            }
        }
        Err(e) => {
            println!("Failed to check for updates: {}", e);
            println!(
                "You can manually check: https://github.com/{}/releases",
                GITHUB_REPO
            );
        }
    }
    Ok(())
}

/// Install the latest update.
pub fn run_update() -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("Velocity CLI v{}", current);
    println!("Checking for updates...");

    let release = get_latest_release(GITHUB_REPO).context("Failed to check for updates")?;

    println!("Latest version: {}", release.version);

    if !velocity_core::updater::is_newer_version(&release.version, current) {
        println!("Already up to date (v{}).", current);
        return Ok(());
    }

    println!("Downloading v{}...", release.version);

    // Download the new binary
    let download_url = if release.download_url.is_empty() {
        format!(
            "https://github.com/{}/releases/latest/download/velocity.exe",
            GITHUB_REPO
        )
    } else {
        release.download_url.clone()
    };

    let temp_dir = std::env::temp_dir().join("velocity-update");
    std::fs::create_dir_all(&temp_dir)?;

    println!("Downloading from: {}", download_url);
    let downloaded = velocity_core::downloader::download_file(
        &download_url,
        &temp_dir,
        Some("velocity.exe"),
        None,
        None,
    )
    .context("Failed to download update")?;

    // Verify the download
    if !downloaded.exists() {
        anyhow::bail!(
            "Download failed: file not found at {}",
            downloaded.display()
        );
    }

    let file_size = std::fs::metadata(&downloaded)?.len();
    if file_size < 1024 {
        anyhow::bail!(
            "Downloaded file is suspiciously small ({} bytes), aborting",
            file_size
        );
    }

    println!("Downloaded {} bytes.", file_size);

    // Replace current binary
    replace_current_binary(&downloaded)?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("Updated to v{} successfully!", release.version);
    if let Some(notes) = &release.release_notes {
        println!("\nRelease notes:\n{}", notes);
    }

    Ok(())
}

/// Replace the currently running binary with the downloaded one.
///
/// On Windows, a running executable cannot be overwritten directly, but it CAN
/// be renamed. The standard approach is:
/// 1. Rename the current .exe to .exe.old
/// 2. Copy the new .exe to the original location
/// 3. The .old file will be cleaned up on next update or manually
fn replace_current_binary(new_binary: &PathBuf) -> Result<()> {
    let current_exe = std::env::current_exe().context("Failed to determine current exe path")?;

    info!(
        "Replacing binary: {} -> {}",
        current_exe.display(),
        new_binary.display()
    );

    let backup_path = current_exe.with_extension("exe.old");

    // Remove any previous backup
    if backup_path.exists() {
        std::fs::remove_file(&backup_path).context("Failed to remove previous backup")?;
    }

    // Rename current -> backup
    std::fs::rename(&current_exe, &backup_path)
        .context("Failed to rename current binary for backup")?;

    // Copy new -> current
    match std::fs::copy(new_binary, &current_exe) {
        Ok(_) => {
            println!("Binary replaced successfully.");
            println!("Previous version saved as: {}", backup_path.display());
            Ok(())
        }
        Err(e) => {
            // Rollback: rename backup back
            let _ = std::fs::rename(&backup_path, &current_exe);
            Err(anyhow::anyhow!(
                "Failed to install new binary: {}. Previous version restored.",
                e
            ))
        }
    }
}

/// Information about a GitHub release.
struct GitHubRelease {
    version: String,
    download_url: String,
    release_notes: Option<String>,
}

/// Query the GitHub API for the latest release.
fn get_latest_release(repo: &str) -> Result<GitHubRelease> {
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let response = velocity_core::downloader::download_to_memory(&api_url)
        .context("Failed to reach GitHub API")?;

    let body = String::from_utf8_lossy(&response);
    let json: serde_json::Value =
        serde_json::from_str(&body).context("Failed to parse GitHub API response")?;

    let version = json["tag_name"]
        .as_str()
        .unwrap_or("0.0.0")
        .trim_start_matches('v')
        .to_string();

    // Find the velocity.exe asset, or fall back to the zipball
    let download_url = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find_map(|asset| {
                if asset["name"].as_str() == Some("velocity.exe") {
                    asset["browser_download_url"].as_str().map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| {
            format!(
                "https://github.com/{}/releases/latest/download/velocity.exe",
                repo
            )
        });

    let release_notes = json["body"].as_str().map(|s| {
        // Truncate long release notes
        if s.len() > 500 {
            format!("{}...", &s[..500])
        } else {
            s.to_string()
        }
    });

    Ok(GitHubRelease {
        version,
        download_url,
        release_notes,
    })
}
