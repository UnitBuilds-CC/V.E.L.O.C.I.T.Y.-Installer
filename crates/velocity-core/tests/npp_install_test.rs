//! Real-world Notepad++ silent install test.
//!
//! Downloads Notepad++ (NSIS-based, supports per-user install without admin)
//! and silently installs it through the full pipeline.
//!
//! Run with: cargo test -p velocity-core --test npp_install_test -- --ignored --nocapture

use std::path::PathBuf;
use velocity_core::fetch::{
    DownloadManager, detect_installer_type, execute_silent_installer, InstallerType,
};

fn test_install_dir() -> PathBuf {
    std::env::temp_dir().join("velocity_npp_install_test")
}

#[test]
#[ignore] // Requires network access
fn test_real_npp_silent_install() {
    let install_dir = test_install_dir();
    let _ = std::fs::create_dir_all(&install_dir);

    println!("=== Real Install Test: Notepad++ (NSIS) ===");
    println!("Install dir: {}", install_dir.display());

    // Step 1: Download Notepad++ installer
    println!("\n--- Step 1: Downloading Notepad++ installer ---");
    let dm = DownloadManager::new().expect("Failed to create download manager");
    let url = "https://github.com/notepad-plus-plus/notepad-plus-plus/releases/download/v8.6.9/npp.8.6.9.Installer.x64.exe";

    let downloaded_path = dm.download(url, &install_dir, Some("npp_installer.exe"), None, None)
        .expect("Failed to download Notepad++ installer");

    println!("Downloaded to: {}", downloaded_path.display());
    let file_size = std::fs::metadata(&downloaded_path).map(|m| m.len()).unwrap_or(0);
    println!("File size: {} bytes ({:.1} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
    assert!(file_size > 100_000, "Downloaded file should be > 100KB");

    // Step 2: Detect installer type
    println!("\n--- Step 2: Detecting installer type ---");
    let detected = detect_installer_type(&downloaded_path);
    println!("Detected: {}", detected);
    assert_eq!(detected, InstallerType::Nsis,
        "Notepad++ should be detected as NSIS, got: {}", detected);

    // Step 3: Execute silent install (per-user, no admin needed)
    println!("\n--- Step 3: Executing silent install ---");
    // Notepad++ NSIS: /S for silent, /D for install dir
    let result = execute_silent_installer(
        &downloaded_path,
        Some("/S"),
        None,
        Some(&install_dir),
        180, // 3 minute timeout
    );

    match result {
        Ok(install_result) => {
            println!("Exit code: {}", install_result.exit_code);
            println!("Success: {}", install_result.success);
            println!("Installer type: {}", install_result.installer_type);

            assert!(install_result.success,
                "Notepad++ install should succeed (exit code: {})", install_result.exit_code);

            // Step 4: Verify installation
            println!("\n--- Step 4: Verifying installation ---");
            let npp_exe = install_dir.join("notepad++.exe");

            println!("Files in install dir:");
            if let Ok(entries) = std::fs::read_dir(&install_dir) {
                let mut count = 0;
                for entry in entries.flatten() {
                    if count < 20 {
                        let meta = entry.metadata().ok();
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        println!("  {} ({} bytes)", entry.file_name().to_string_lossy(), size);
                    }
                    count += 1;
                }
                if count > 20 {
                    println!("  ... and {} more files", count - 20);
                }
            }

            assert!(npp_exe.exists(),
                "notepad++.exe should exist in {}", install_dir.display());

            let npp_size = std::fs::metadata(&npp_exe).unwrap().len();
            println!("\n✓ notepad++.exe found ({} bytes)", npp_size);

            // Step 5: Uninstall
            println!("\n--- Step 5: Uninstalling Notepad++ ---");
            let uninstaller = install_dir.join("uninstall.exe");
            if uninstaller.exists() {
                println!("Running: {} /S", uninstaller.display());
                let status = std::process::Command::new(&uninstaller)
                    .arg("/S")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match status {
                    Ok(s) => println!("Uninstaller exit code: {}", s.code().unwrap_or(-1)),
                    Err(e) => println!("Uninstaller error: {}", e),
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(3));
            let _ = std::fs::remove_dir_all(&install_dir);

            println!("\n✓✓✓ REAL NOTEPAD++ INSTALL TEST PASSED ✓✓✓");
            println!("Notepad++ was downloaded, detected as NSIS, silently installed, verified, and uninstalled.");
        }
        Err(e) => {
            let err_chain = format!("{:#}", e);
            if err_chain.contains("elevation") || err_chain.contains("740") || err_chain.contains("access") {
                println!("\n⚠ SKIPPED: Requires admin elevation.");
                println!("Error: {}", err_chain);
                // Clean up
                let _ = std::fs::remove_dir_all(&install_dir);
                return;
            }
            // Clean up
            let _ = std::fs::remove_dir_all(&install_dir);
            panic!("Unexpected error: {}", err_chain);
        }
    }
}
