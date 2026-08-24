/// Test: use actual compiler to build MSI, then test with msiexec
use velocity_compiler::msi_builder::{build_msi, MsiOptions};
use velocity_config::parse_manifest;
use std::process::Command;

fn main() {
    // Load the sample app manifest
    let project_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/sample-app");
    let manifest_path = project_dir.join("velocity.toml");
    
    eprintln!("Loading manifest from: {}", manifest_path.display());
    let manifest = parse_manifest(&manifest_path).expect("Failed to load manifest");
    
    eprintln!("App: {} v{}", manifest.app.name, manifest.app.version);
    eprintln!("Publisher: {}", manifest.app.publisher);
    
    // Build MSI using the compiler
    let output_path = project_dir.join("output/test_compiler.msi");
    let options = MsiOptions {
        output_path: output_path.clone(),
        project_dir: project_dir.clone(),
        architecture: manifest.install.arch.clone(),
        language: 1033,
        per_machine: false,  // Use per-user install to avoid admin privilege requirement
        upgrade_code: None,
    };
    
    std::fs::create_dir_all(output_path.parent().unwrap()).ok();
    
    eprintln!("Building MSI...");
    let result = build_msi(&manifest, &options).expect("Failed to build MSI");
    
    eprintln!("MSI built: {} ({} bytes)", result.msi_path.display(), result.msi_size);
    eprintln!("Product code: {}", result.product_code);
    
    // Strip UNC path prefix for msiexec
    let abs_path = std::fs::canonicalize(&result.msi_path).unwrap();
    let msi_str = abs_path.to_str().unwrap().trim_start_matches(r"\\?\").to_string();
    eprintln!("Testing with msiexec: {}", msi_str);
    
    // Test with msiexec
    let status = Command::new("msiexec")
        .args(&["/i", &msi_str, "/qn", "/norestart"])
        .status()
        .expect("Failed to run msiexec");
    
    let exit_code = status.code().unwrap_or(-1);
    eprintln!("msiexec exit code: {}", exit_code);
    
    if exit_code == 0 {
        eprintln!("✓ SUCCESS: MSI installed without errors!");
        
        // Try to uninstall using product code
        eprintln!("Testing uninstall...");
        let status2 = Command::new("msiexec")
            .args(&["/x", &result.product_code, "/qn", "/norestart"])
            .status();
        
        if let Ok(s) = status2 {
            eprintln!("uninstall exit code: {}", s.code().unwrap_or(-1));
        }
    } else {
        eprintln!("✗ FAILED: msiexec returned error {}", exit_code);
    }
}
