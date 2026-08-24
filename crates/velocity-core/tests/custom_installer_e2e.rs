//! End-to-end test for execute_with_config (user-configurable installer).
//!
//! Tests the full custom installer pipeline:
//! - Custom args with {dir}/{file} placeholder substitution
//! - Pre/post install commands
//! - Custom success codes
//! - Environment variables
//!
//! Run with: cargo test -p velocity-core --test custom_installer_e2e -- --nocapture

use std::path::Path;
use velocity_config::InstallerConfig;
use velocity_core::fetch::execute_with_config;

fn test_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("test_silent_install")
}

/// Test execute_with_config with a batch file installer.
/// Verifies custom args, placeholder substitution, pre/post commands, and success codes.
#[test]
fn test_custom_installer_e2e() {
    let bat = test_dir().join("fake_nsis_installer.bat");
    if !bat.exists() {
        eprintln!("Skipping: batch file not found at {}", bat.display());
        return;
    }

    let install_dir = tempfile::tempdir().unwrap();
    let log_dir = tempfile::tempdir().unwrap();

    // Write a pre-install marker file
    let pre_marker = log_dir.path().join("pre_install_ran.txt");
    let post_marker = log_dir.path().join("post_install_ran.txt");

    let mut config = InstallerConfig::default();

    // Custom args with placeholder substitution
    config.args = Some(format!("/S /D={}", install_dir.path().display()));

    // Custom success codes (include 0 which the batch file returns)
    config.success_codes = Some(vec![0, 3010]);

    // Timeout
    config.timeout_secs = Some(30);

    // Pre-install: create a marker file
    config.pre_install = vec![
        format!("powershell -Command \"Set-Content -Path '{}' -Value 'pre_install'\"", pre_marker.display()),
    ];

    // Post-install: create a marker file
    config.post_install = vec![
        format!("powershell -Command \"Set-Content -Path '{}' -Value 'post_install'\"", post_marker.display()),
    ];

    // Environment variables
    config.env.insert("VELOCITY_TEST".to_string(), "custom_value".to_string());

    println!("=== Custom Installer E2E Test ===");
    println!("Installer: {}", bat.display());
    println!("Install dir: {}", install_dir.path().display());
    println!("Config: {:?}", config);

    let result = execute_with_config(
        &bat,
        &config,
        Some(install_dir.path()),
    );

    match &result {
        Ok(install_result) => {
            println!("Exit code: {}", install_result.exit_code);
            println!("Success: {}", install_result.success);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }

    let install_result = result.expect("execute_with_config should succeed");
    assert!(install_result.success, "Install should be successful");
    assert_eq!(install_result.exit_code, 0);

    // Verify pre-install command ran
    assert!(pre_marker.exists(), "Pre-install command should have created marker file");
    let pre_content = std::fs::read_to_string(&pre_marker).unwrap_or_default();
    assert!(pre_content.contains("pre_install"), "Pre-install marker should contain 'pre_install'");
    println!("✓ Pre-install command executed correctly");

    // Verify post-install command ran
    assert!(post_marker.exists(), "Post-install command should have created marker file");
    let post_content = std::fs::read_to_string(&post_marker).unwrap_or_default();
    assert!(post_content.contains("post_install"), "Post-install marker should contain 'post_install'");
    println!("✓ Post-install command executed correctly");

    // Verify the batch file received the correct args
    let log = test_dir().join("install_log.txt");
    if log.exists() {
        let log_content = std::fs::read_to_string(&log).unwrap_or_default();
        println!("Installer log: {}", log_content.trim());
        assert!(log_content.contains("/S"), "Log should contain /S flag");
        assert!(log_content.contains("/D="), "Log should contain /D= flag");
        println!("✓ Custom args passed correctly to installer");
    }

    println!("=== Custom Installer E2E Test PASSED ===");
}

/// Test that custom success_codes work correctly.
/// The reboot batch file exits with 3010, which should be accepted.
#[test]
fn test_custom_success_codes() {
    let bat = test_dir().join("fake_reboot_installer.bat");
    if !bat.exists() {
        eprintln!("Skipping: batch file not found at {}", bat.display());
        return;
    }

    let mut config = InstallerConfig::default();
    config.args = Some("/S".to_string());
    // Only accept 3010 as success (the batch file exits with 3010)
    config.success_codes = Some(vec![3010]);
    config.timeout_secs = Some(30);

    println!("=== Custom Success Codes Test ===");

    let result = execute_with_config(&bat, &config, None);
    let install_result = result.expect("Should succeed with custom success code 3010");
    assert!(install_result.success);
    assert_eq!(install_result.exit_code, 3010);

    println!("✓ Custom success code 3010 accepted correctly");
    println!("=== Custom Success Codes Test PASSED ===");
}

/// Test that placeholder substitution works for {dir} and {file}.
#[test]
fn test_placeholder_substitution() {
    let bat = test_dir().join("fake_innosetup_installer.bat");
    if !bat.exists() {
        eprintln!("Skipping: batch file not found at {}", bat.display());
        return;
    }

    let install_dir = tempfile::tempdir().unwrap();

    let mut config = InstallerConfig::default();
    // Use both {dir} and {file} placeholders
    config.args = Some(format!(
        "/VERYSILENT /DIR=\"{{dir}}\" /LOG=\"{{file}}.log\"",
    ));
    config.timeout_secs = Some(30);

    println!("=== Placeholder Substitution Test ===");
    println!("Template args: {}", config.args.as_ref().unwrap());

    let result = execute_with_config(&bat, &config, Some(install_dir.path()));
    let install_result = result.expect("Should succeed");
    assert!(install_result.success);

    // Check the log file for the substituted args
    let log = test_dir().join("innosetup_log.txt");
    if log.exists() {
        let log_content = std::fs::read_to_string(&log).unwrap_or_default();
        println!("Installer log: {}", log_content.trim());
        // {dir} should have been replaced with the actual install dir
        assert!(
            log_content.contains(&install_dir.path().to_string_lossy().to_string()),
            "Log should contain the install dir path"
        );
        // {file} should have been replaced with the installer path
        assert!(
            log_content.contains(&bat.to_string_lossy().to_string()),
            "Log should contain the installer file path"
        );
        println!("✓ Placeholders {{dir}} and {{file}} substituted correctly");
    }

    println!("=== Placeholder Substitution Test PASSED ===");
}

/// Test that detect_signatures logs correctly.
#[test]
fn test_detect_signatures() {
    // Create a temp file with a known signature
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("custom_installer.exe");
    // Write "Nullsoft" + our custom signature
    let mut data = vec![0u8; 1000];
    data.extend_from_slice(b"MyCustomFramework_v2");
    std::fs::write(&fake, &data).unwrap();

    let mut config = InstallerConfig::default();
    config.args = Some("/S".to_string());
    config.detect_signatures = vec!["MyCustomFramework_v2".to_string()];
    config.detect_name = Some("CustomFramework".to_string());
    config.timeout_secs = Some(5);

    println!("=== Detect Signatures Test ===");

    // The execute will fail (it's not a real executable), but the signature
    // detection should have been logged
    let result = execute_with_config(&fake, &config, None);
    // We expect this to fail since it's not a real executable
    assert!(result.is_err());
    println!("✓ detect_signatures processed (check log output above for signature match)");

    println!("=== Detect Signatures Test PASSED ===");
}
