//! End-to-end integration test: builds a real installer and verifies it installs correctly.
//!
//! Run with: `cargo test --test e2e -- --ignored --nocapture`
//! Requires: `cargo build --release -p velocity-runtime` (runtime must be pre-built)

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    // crates/velocity-compiler -> workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn runtime_exe() -> PathBuf {
    workspace_root().join("target").join("release").join("velocity-runtime.exe")
}

fn velocity_cli() -> PathBuf {
    workspace_root().join("target").join("release").join("velocity.exe")
}

/// Create a minimal test project with velocity.toml and sample files.
fn create_test_project(dir: &Path) {
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::write(dir.join("bin").join("hello.txt"), "Hello from Velocity!").unwrap();
    std::fs::write(dir.join("bin").join("data.bin"), vec![0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
    std::fs::write(dir.join("bin").join("readme.txt"), "Integration test payload").unwrap();

    let toml = r#"
[app]
name = "E2E Integration Test"
version = "0.1.0"
publisher = "Velocity CI"

[install]
default_dir = "{autopf}\\E2E Integration Test"
require_admin = false

[files]
source = ["bin/**/*"]

[shortcuts]
desktop = false
start_menu = false

[scripts]
pre_install = ["echo pre-install-ok"]
post_install = ["echo post-install-ok"]

[uninstall]
add_remove = false

[ui]
theme = "classic"
"#;
    std::fs::write(dir.join("velocity.toml"), toml).unwrap();
}

#[test]
#[ignore] // Requires pre-built runtime: `cargo build --release -p velocity-runtime -p velocity-cli`
fn test_build_and_run_installer() {
    let temp = std::env::temp_dir().join("velocity_e2e_integration");
    let project_dir = temp.join("project");
    let install_dir = temp.join("install_output");

    // Clean up from any previous run
    let _ = std::fs::remove_dir_all(&temp);

    // Prerequisites
    assert!(runtime_exe().exists(), "Runtime not found. Run: cargo build --release -p velocity-runtime");
    assert!(velocity_cli().exists(), "CLI not found. Run: cargo build --release -p velocity-cli");

    // Step 1: Create test project
    create_test_project(&project_dir);

    // Copy runtime to where the compiler expects it
    let runtime_target = project_dir.join("target").join("release");
    std::fs::create_dir_all(&runtime_target).unwrap();
    std::fs::copy(runtime_exe(), runtime_target.join("velocity-runtime.exe")).unwrap();

    // Step 2: Build the installer
    let output = Command::new(velocity_cli())
        .args(["build"])
        .current_dir(&project_dir)
        .output()
        .expect("Failed to run velocity build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Build failed.\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Step 3: Verify installer exe exists and is valid PE
    let installer = project_dir.join("output").join("installer.exe");
    assert!(installer.exists(), "Installer exe not created");

    let exe_bytes = std::fs::read(&installer).unwrap();
    assert!(exe_bytes.len() > 1000, "Installer exe is too small to be valid");

    // Check PE signature
    let pe_offset = i32::from_le_bytes([exe_bytes[60], exe_bytes[61], exe_bytes[62], exe_bytes[63]]) as usize;
    assert_eq!(&exe_bytes[pe_offset..pe_offset + 2], b"PE", "Invalid PE signature");

    // Step 4: Run installer in silent mode
    let run_output = Command::new(&installer)
        .args(["/S", &format!("/D={}", install_dir.display())])
        .output()
        .expect("Failed to run installer");

    let run_stdout = String::from_utf8_lossy(&run_output.stdout);
    let run_stderr = String::from_utf8_lossy(&run_output.stderr);
    assert!(
        run_output.status.success(),
        "Installer failed.\nstdout: {}\nstderr: {}",
        run_stdout, run_stderr
    );

    // Step 5: Verify installed files
    assert!(install_dir.exists(), "Install directory not created");
    assert!(install_dir.join("bin").join("hello.txt").exists(), "hello.txt not installed");
    assert!(install_dir.join("bin").join("data.bin").exists(), "data.bin not installed");
    assert!(install_dir.join("bin").join("readme.txt").exists(), "readme.txt not installed");
    assert!(install_dir.join("uninstall.exe").exists(), "Uninstaller not generated");

    // Verify file contents
    let hello = std::fs::read_to_string(install_dir.join("bin").join("hello.txt")).unwrap();
    assert_eq!(hello, "Hello from Velocity!");

    let data = std::fs::read(install_dir.join("bin").join("data.bin")).unwrap();
    assert_eq!(data, vec![0xDE, 0xAD, 0xBE, 0xEF]);

    let readme = std::fs::read_to_string(install_dir.join("bin").join("readme.txt")).unwrap();
    assert_eq!(readme, "Integration test payload");

    // Verify install log exists and contains expected entries
    let log_files: Vec<_> = std::fs::read_dir(&install_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "log"))
        .collect();
    assert!(!log_files.is_empty(), "No install log file found");

    let log_content = std::fs::read_to_string(log_files[0].path()).unwrap();
    assert!(log_content.contains("E2E Integration Test"), "Log missing app name");
    assert!(log_content.contains("Extracted 3 files"), "Log missing extraction count");
    assert!(log_content.contains("completed successfully"), "Log missing success message");

    // Step 6: Verify install log contains script execution
    assert!(log_content.contains("pre-install-ok"), "Pre-install script not logged");
    assert!(log_content.contains("post-install-ok"), "Post-install script not logged");

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp);

    println!("E2E integration test passed!");
}

#[test]
#[ignore] // Requires pre-built runtime
fn test_encrypted_installer() {
    let temp = std::env::temp_dir().join("velocity_e2e_encrypted");
    let project_dir = temp.join("project");
    let install_dir = temp.join("install_output");

    let _ = std::fs::remove_dir_all(&temp);

    assert!(runtime_exe().exists(), "Runtime not found");
    assert!(velocity_cli().exists(), "CLI not found");

    // Create project with password
    std::fs::create_dir_all(project_dir.join("bin")).unwrap();
    std::fs::write(project_dir.join("bin").join("secret.txt"), "Encrypted content").unwrap();

    let toml = r#"
[app]
name = "Encrypted Test"
version = "1.0.0"
publisher = "Velocity CI"

[install]
default_dir = "{autopf}\\Encrypted Test"
require_admin = false
password = "test-password-123"

[files]
source = ["bin/**/*"]

[shortcuts]
desktop = false
start_menu = false

[uninstall]
add_remove = false

[ui]
theme = "classic"
"#;
    std::fs::write(project_dir.join("velocity.toml"), toml).unwrap();

    let runtime_target = project_dir.join("target").join("release");
    std::fs::create_dir_all(&runtime_target).unwrap();
    std::fs::copy(runtime_exe(), runtime_target.join("velocity-runtime.exe")).unwrap();

    // Build
    let output = Command::new(velocity_cli())
        .args(["build"])
        .current_dir(&project_dir)
        .output()
        .expect("Failed to run velocity build");
    assert!(output.status.success(), "Encrypted build failed");

    // Run with password
    let installer = project_dir.join("output").join("installer.exe");
    let run_output = Command::new(&installer)
        .args(["/S", "/P=test-password-123", &format!("/D={}", install_dir.display())])
        .output()
        .expect("Failed to run encrypted installer");

    assert!(
        run_output.status.success(),
        "Encrypted installer failed: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );

    // Verify file
    assert!(install_dir.join("bin").join("secret.txt").exists());
    let content = std::fs::read_to_string(install_dir.join("bin").join("secret.txt")).unwrap();
    assert_eq!(content, "Encrypted content");

    let _ = std::fs::remove_dir_all(&temp);
    println!("Encrypted installer E2E test passed!");
}
