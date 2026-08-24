//! Real-world silent install test.
//!
//! Downloads a real app installer and silently installs it through the full
//! pipeline: download → detect → generate args → execute → verify files.
//!
//! Run with: cargo test -p velocity-core --test real_install_test -- --ignored --nocapture
//!
//! NOTE: Some tests require admin elevation. Run from an admin terminal if needed.

use std::path::PathBuf;
use velocity_core::fetch::{
    DownloadManager, detect_installer_type, execute_silent_installer, InstallerType,
};

fn test_install_dir() -> PathBuf {
    std::env::temp_dir().join("velocity_real_install_test")
}

/// Test 1: Download 7-Zip and verify binary detection works on real files.
/// This does NOT require admin — it only tests download + detection.
#[test]
#[ignore] // Requires network access
fn test_real_7zip_download_and_detect() {
    let dm = DownloadManager::new().expect("Failed to create download manager");
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    println!("=== Real Installer Download + Detection Test ===\n");

    // Download 7-Zip installer
    println!("--- Downloading 7-Zip (expected: 7-Zip custom framework) ---");
    let path = dm.download("https://www.7-zip.org/a/7z2409-x64.exe", temp_dir.path(), Some("7zip.exe"), None, None)
        .expect("Failed to download 7-Zip installer");

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("Downloaded: {} ({} bytes, {:.1} MB)", path.display(), size, size as f64 / 1024.0 / 1024.0);
    assert!(size > 100_000, "Downloaded file should be > 100KB");

    let detected = detect_installer_type(&path);
    println!("Detected as: {}", detected);
    assert_eq!(detected, InstallerType::SevenZip, "7-Zip should be detected as SevenZip");
    println!("✓ 7-Zip correctly detected as custom 7-Zip framework\n");

    // Verify we can generate silent args
    let args = velocity_core::fetch::get_silent_args(InstallerType::SevenZip, Some(temp_dir.path()));
    println!("Auto-generated silent args: {:?}", args);
    assert!(args.iter().any(|a| a.contains("/S")), "Should include /S flag");
    println!("✓ Silent args correctly generated: {:?}\n", args);

    println!("=== Download + Detection Test PASSED ===");
}

/// Test 2: Actually install 7-Zip silently.
/// REQUIRES ADMIN ELEVATION. Will fail with OS error 740 if not admin.
#[test]
#[ignore] // Requires network + admin elevation
fn test_real_7zip_silent_install() {
    let install_dir = test_install_dir();
    let _ = std::fs::create_dir_all(&install_dir);

    println!("=== Real Install Test: 7-Zip ===");
    println!("Install dir: {}", install_dir.display());
    println!("NOTE: This test requires admin elevation.\n");

    // Step 1: Download 7-Zip installer
    println!("--- Step 1: Downloading 7-Zip installer ---");
    let dm = DownloadManager::new().expect("Failed to create download manager");

    let downloaded_path = dm.download("https://www.7-zip.org/a/7z2409-x64.exe", &install_dir, None, None, None)
        .expect("Failed to download 7-Zip installer");

    println!("Downloaded to: {}", downloaded_path.display());
    let file_size = std::fs::metadata(&downloaded_path).map(|m| m.len()).unwrap_or(0);
    println!("File size: {} bytes ({:.1} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
    assert!(file_size > 100_000, "Downloaded file should be > 100KB");

    // Step 2: Detect installer type
    println!("\n--- Step 2: Detecting installer type ---");
    let detected = detect_installer_type(&downloaded_path);
    println!("Detected: {}", detected);
    assert_eq!(detected, InstallerType::SevenZip);

    // Step 3: Execute silent install
    println!("\n--- Step 3: Executing silent install ---");
    println!("Running: {} /S /D={}", downloaded_path.display(), install_dir.display());

    let result = execute_silent_installer(
        &downloaded_path,
        Some("/S"),
        None,
        Some(&install_dir),
        120,
    );

    match result {
        Ok(install_result) => {
            println!("Exit code: {}", install_result.exit_code);
            println!("Success: {}", install_result.success);
            assert!(install_result.success, "7-Zip install should succeed");

            // Step 4: Verify installation
            println!("\n--- Step 4: Verifying installation ---");
            let seven_zip_exe = install_dir.join("7z.exe");
            let seven_zip_fm = install_dir.join("7zFM.exe");

            println!("Files in install dir:");
            if let Ok(entries) = std::fs::read_dir(&install_dir) {
                for entry in entries.flatten() {
                    let meta = entry.metadata().ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    println!("  {} ({} bytes)", entry.file_name().to_string_lossy(), size);
                }
            }

            let installed = seven_zip_exe.exists() || seven_zip_fm.exists();
            assert!(installed, "7-Zip should have installed 7z.exe or 7zFM.exe");

            if seven_zip_exe.exists() {
                println!("\n✓ 7z.exe found ({} bytes)", std::fs::metadata(&seven_zip_exe).unwrap().len());
            }
            if seven_zip_fm.exists() {
                println!("✓ 7zFM.exe found ({} bytes)", std::fs::metadata(&seven_zip_fm).unwrap().len());
            }

            // Step 5: Uninstall
            println!("\n--- Step 5: Uninstalling 7-Zip ---");
            let uninstaller = install_dir.join("Uninstall.exe");
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

            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = std::fs::remove_dir_all(&install_dir);

            println!("\n✓✓✓ REAL INSTALL TEST PASSED ✓✓✓");
            println!("7-Zip was downloaded, detected, silently installed, verified, and uninstalled.");
        }
        Err(e) => {
            // Check the full error chain for elevation-related errors
            let err_chain = format!("{:#}", e);
            if err_chain.contains("elevation") || err_chain.contains("740") || err_chain.contains("access") {
                println!("\n⚠ SKIPPED: This test requires admin elevation.");
                println!("Error: {}", err_chain);
                println!("Run from an admin terminal to execute this test.");
                // Don't panic — this is expected when not running as admin
                return;
            }
            panic!("Unexpected error: {}", err_chain);
        }
    }
}

/// Test 3: Verify that our batch-file fake installers still work (no admin needed).
/// This proves the execution pipeline without requiring elevation.
#[test]
#[ignore] // Integration test
fn test_fake_installer_still_works() {
    use std::path::Path;

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap();
    let bat_path = project_root.join("test_silent_install").join("fake_nsis_installer.bat");

    if !bat_path.exists() {
        println!("Skipping: batch file not found at {}", bat_path.display());
        return;
    }

    println!("=== Fake Installer Execution Test ===");
    println!("Running: {}", bat_path.display());

    let result = execute_silent_installer(
        &bat_path,
        Some("/S"),
        Some("exe"),
        None,
        30,
    ).expect("Failed to execute fake installer");

    println!("Exit code: {}", result.exit_code);
    println!("Success: {}", result.success);
    assert!(result.success);
    assert_eq!(result.exit_code, 0);

    // Verify the batch file received the /S flag
    let log_path = bat_path.parent().unwrap().join("install_log.txt");
    if log_path.exists() {
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        println!("Installer log: {}", log.trim());
        assert!(log.contains("/S"), "Log should contain /S flag");
        println!("✓ Fake installer received /S flag correctly");
    }

    println!("=== Fake Installer Test PASSED ===");
}
