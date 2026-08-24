//! End-to-end silent install test.
//!
//! Uses fake batch-file installers to prove the full pipeline:
//! 1. execute_silent_installer() actually launches the process
//! 2. Silent flags are correctly passed to the installer
//! 3. Exit codes (0, 3010) are handled properly
//!
//! Run with: cargo test -p velocity-core --test silent_install_e2e -- --nocapture

use std::path::Path;
use velocity_core::fetch::{execute_silent_installer, detect_installer_type, get_silent_args, InstallerType};

fn test_dir() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = crates/velocity-core, need to go up 2 levels to project root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("test_silent_install")
}

/// Helper: check if any element in Vec<String> contains the given substring
fn vec_contains(args: &[String], needle: &str) -> bool {
    args.iter().any(|s| s.contains(needle))
}

/// Test 1: Execute a fake NSIS installer with explicit /S flag.
/// Proves that the process actually runs and receives the correct arguments.
#[test]
fn test_e2e_nsis_silent_install() {
    let dir = test_dir();
    let installer = dir.join("fake_nsis_installer.bat");
    assert!(installer.exists(), "Fake NSIS installer not found at: {}", installer.display());
    
    // Clean up any previous log
    let log_file = dir.join("install_log.txt");
    let _ = std::fs::remove_file(&log_file);

    println!("=== E2E Test: NSIS Silent Install ===");
    println!("Installer: {}", installer.display());
    
    // Detect installer type (will be Unknown since .bat has no NSIS signature)
    let detected = detect_installer_type(&installer);
    println!("Detected type: {}", detected);
    
    // Execute with explicit NSIS-style silent args
    let result = execute_silent_installer(
        &installer,
        Some("/S"),
        Some("exe"),
        Some(&dir),
        30,
    ).expect("Failed to execute fake NSIS installer");

    println!("Exit code: {}", result.exit_code);
    println!("Success: {}", result.success);
    println!("Installer type: {}", result.installer_type);

    assert!(result.success, "Installer should have exited successfully");
    assert_eq!(result.exit_code, 0);

    // Verify the log file was created (proves the process actually ran)
    assert!(log_file.exists(), "Install log should exist - proves the process ran!");
    
    let log_content = std::fs::read_to_string(&log_file).unwrap();
    println!("\n--- Install Log ---\n{}\n--- End Log ---", log_content);
    
    assert!(log_content.contains("FAKE_NSIS_INSTALLER"), "Log should contain installer marker");
    assert!(log_content.contains("/S"), "Log should show /S flag was passed!");
    
    println!("\n[PASS] NSIS silent install E2E: Process ran, /S flag received, exit code 0");
}

/// Test 2: Execute a fake installer that returns exit code 3010 (reboot required).
/// Proves that acceptable exit codes are handled correctly.
#[test]
fn test_e2e_acceptable_exit_code_3010() {
    let dir = test_dir();
    let installer = dir.join("fake_reboot_installer.bat");
    assert!(installer.exists(), "Fake reboot installer not found");
    
    let log_file = dir.join("reboot_log.txt");
    let _ = std::fs::remove_file(&log_file);

    println!("=== E2E Test: Acceptable Exit Code 3010 ===");

    let result = execute_silent_installer(
        &installer,
        Some("/S /norestart"),
        Some("exe"),
        Some(&dir),
        30,
    ).expect("Failed to execute fake reboot installer");

    println!("Exit code: {}", result.exit_code);
    println!("Success: {}", result.success);

    // Exit code 3010 (reboot required) should be treated as success
    assert!(result.success, "Exit code 3010 should be treated as acceptable");
    assert_eq!(result.exit_code, 3010);

    assert!(log_file.exists(), "Reboot install log should exist");
    let log_content = std::fs::read_to_string(&log_file).unwrap();
    println!("\n--- Reboot Log ---\n{}\n--- End Log ---", log_content);
    assert!(log_content.contains("FAKE_REBOOT_REQUIRED_INSTALLER"));
    
    println!("\n[PASS] Acceptable exit code E2E: 3010 treated as success");
}

/// Test 3: Verify auto-detection of silent args for known installer types.
#[test]
fn test_e2e_auto_detect_silent_args() {
    println!("=== E2E Test: Auto-detect Silent Args ===");
    
    // NSIS
    let nsis_args = get_silent_args(InstallerType::Nsis, None);
    println!("NSIS auto-detected args: {:?}", nsis_args);
    assert!(vec_contains(&nsis_args, "/S"), "NSIS should auto-detect /S");
    
    let nsis_dir_args = get_silent_args(InstallerType::Nsis, Some(Path::new("C:\\TestApp")));
    println!("NSIS with dir: {:?}", nsis_dir_args);
    assert!(vec_contains(&nsis_dir_args, "/S"), "NSIS should include /S");
    assert!(vec_contains(&nsis_dir_args, "/D="), "NSIS should include /D= dir");
    
    // InnoSetup
    let inno_args = get_silent_args(InstallerType::InnoSetup, None);
    println!("InnoSetup auto-detected args: {:?}", inno_args);
    assert!(vec_contains(&inno_args, "/VERYSILENT"), "InnoSetup should auto-detect /VERYSILENT");
    assert!(vec_contains(&inno_args, "/SUPPRESSMSGBOXES"), "InnoSetup should include /SUPPRESSMSGBOXES");
    assert!(vec_contains(&inno_args, "/NORESTART"), "InnoSetup should include /NORESTART");
    
    let inno_dir_args = get_silent_args(InstallerType::InnoSetup, Some(Path::new("C:\\TestApp")));
    println!("InnoSetup with dir: {:?}", inno_dir_args);
    assert!(vec_contains(&inno_dir_args, "/DIR="), "InnoSetup should include /DIR= when dir provided");
    
    // 7z-SFX
    let sfx_args = get_silent_args(InstallerType::SevenZipSfx, None);
    println!("7z-SFX auto-detected args: {:?}", sfx_args);
    assert!(vec_contains(&sfx_args, "-y"), "7z-SFX should auto-detect -y");
    
    // Unknown
    let unknown_args = get_silent_args(InstallerType::Unknown, None);
    println!("Unknown auto-detected args: {:?}", unknown_args);
    assert!(unknown_args.is_empty(), "Unknown should return empty args");
    
    println!("\n[PASS] Auto-detect silent args: All installer types generate correct flags");
}

/// Test 4: Execute a fake InnoSetup installer with auto-detected flags.
/// Proves the full auto-detect -> generate args -> execute pipeline.
#[test]
fn test_e2e_innosetup_auto_silent_install() {
    let dir = test_dir();
    let installer = dir.join("fake_innosetup_installer.bat");
    assert!(installer.exists(), "Fake InnoSetup installer not found");
    
    let log_file = dir.join("innosetup_log.txt");
    let _ = std::fs::remove_file(&log_file);

    println!("=== E2E Test: InnoSetup Auto-detected Silent Install ===");
    
    // Get auto-detected InnoSetup args and join them into a single string
    let auto_args = get_silent_args(InstallerType::InnoSetup, Some(&dir));
    let auto_args_str = auto_args.join(" ");
    println!("Auto-detected InnoSetup args: {}", auto_args_str);
    
    // Execute with auto-detected args (simulating what the pipeline does)
    let result = execute_silent_installer(
        &installer,
        Some(&auto_args_str),
        Some("exe"),
        Some(&dir),
        30,
    ).expect("Failed to execute fake InnoSetup installer");

    println!("Exit code: {}", result.exit_code);
    println!("Success: {}", result.success);

    assert!(result.success, "InnoSetup installer should succeed");
    assert_eq!(result.exit_code, 0);

    assert!(log_file.exists(), "InnoSetup log should exist");
    let log_content = std::fs::read_to_string(&log_file).unwrap();
    println!("\n--- InnoSetup Log ---\n{}\n--- End Log ---", log_content);
    
    assert!(log_content.contains("FAKE_INNOSETUP_INSTALLER"));
    assert!(log_content.contains("/VERYSILENT"), "Should have received /VERYSILENT flag");
    assert!(log_content.contains("/SUPPRESSMSGBOXES"), "Should have received /SUPPRESSMSGBOXES flag");
    assert!(log_content.contains("/NORESTART"), "Should have received /NORESTART flag");
    
    println!("\n[PASS] InnoSetup auto-silent install E2E: Auto-detected flags received by installer");
}
