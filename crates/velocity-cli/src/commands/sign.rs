//! Code signing command — signs installer executables using signtool.exe.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Sign an installer executable using a certificate.
pub fn run(
    installer_path: &str,
    cert_file: Option<&str>,
    cert_fingerprint: Option<&str>,
    cert_subject: Option<&str>,
    timestamp_url: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    let path = Path::new(installer_path);
    if !path.exists() {
        anyhow::bail!("Installer not found: {}", installer_path);
    }

    // Find signtool.exe
    let signtool = match find_signtool() {
        Some(path) => path,
        None => {
            anyhow::bail!(
                "signtool.exe not found. Install the Windows SDK or Windows Driver Kit (WDK) \
                 to get signtool.exe, or add it to your PATH."
            );
        }
    };

    println!("Signing: {}", installer_path);

    // Build signtool command
    let mut cmd = Command::new(&signtool);
    cmd.arg("sign");

    // Signing method
    if let Some(fingerprint) = cert_fingerprint {
        cmd.arg("/sha1").arg(fingerprint);
    } else if let Some(subject) = cert_subject {
        cmd.arg("/n").arg(subject);
    } else if let Some(cert) = cert_file {
        cmd.arg("/f").arg(cert);
    } else {
        // Try to use the default certificate store
        println!("No certificate specified, trying default store...");
    }

    // Description
    if let Some(desc) = description {
        cmd.arg("/d").arg(desc);
    }

    // Timestamp
    if let Some(url) = timestamp_url {
        cmd.arg("/tr").arg(url);
        cmd.arg("/td").arg("sha256");
    }

    // Algorithm
    cmd.arg("/fd").arg("sha256");

    // The file to sign
    cmd.arg(installer_path);

    // Run signtool
    let output = cmd.output().context("Failed to run signtool.exe")?;

    if output.status.success() {
        println!("Successfully signed: {}", installer_path);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            println!("{}", stdout.trim());
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("Code signing failed:\n{}\n{}", stderr, stdout);
    }
}

/// Verify a signed executable.
pub fn verify(installer_path: &str) -> Result<()> {
    let path = Path::new(installer_path);
    if !path.exists() {
        anyhow::bail!("File not found: {}", installer_path);
    }

    let signtool = match find_signtool() {
        Some(path) => path,
        None => anyhow::bail!("signtool.exe not found."),
    };

    println!("Verifying signature: {}", installer_path);

    let output = Command::new(&signtool)
        .args(["verify", "/pa", installer_path])
        .output()
        .context("Failed to run signtool.exe")?;

    if output.status.success() {
        println!("Signature verified: {}", installer_path);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("Signature verification failed:\n{}\n{}", stderr, stdout);
    }
}

/// Find signtool.exe in common locations.
fn find_signtool() -> Option<String> {
    // First check PATH
    if let Ok(output) = Command::new("where").arg("signtool.exe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            if let Some(p) = path.lines().next() {
                return Some(p.trim().to_string());
            }
        }
    }

    // Search in Windows SDK directories
    let sdk_roots = [
        "C:\\Program Files (x86)\\Windows Kits\\10\\bin",
        "C:\\Program Files\\Windows Kits\\10\\bin",
        "C:\\Program Files (x86)\\Windows Kits\\8.1\\bin",
    ];

    for root in &sdk_roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let bin_dir = entry.path();
                if bin_dir.is_dir() {
                    // Check architecture-specific subdirs
                    for arch in &["x64", "x86", "arm64"] {
                        let signtool = bin_dir.join(arch).join("signtool.exe");
                        if signtool.exists() {
                            return Some(signtool.to_string_lossy().to_string());
                        }
                    }
                    // Check directly in the bin dir
                    let signtool = bin_dir.join("signtool.exe");
                    if signtool.exists() {
                        return Some(signtool.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    None
}
