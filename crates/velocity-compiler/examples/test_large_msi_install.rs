// Test large MSI installation (50MB+)
use velocity_compiler::msi_builder::{build_msi, MsiOptions};
use velocity_config::parse_manifest;
use std::process::Command;

fn main() {
    let project_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test_msi_install");
    
    println!("Loading manifest from: {}", project_dir.display());
    let manifest = parse_manifest(&project_dir.join("velocity.toml"))
        .expect("Failed to parse manifest");
    
    println!("App: {} v{}", manifest.app.name, manifest.app.version);
    println!("Building large MSI (50MB+)...");
    
    let output_path = project_dir.join("output/large_test.msi");
    std::fs::create_dir_all(output_path.parent().unwrap()).ok();
    
    let options = MsiOptions {
        output_path: output_path,
        project_dir: project_dir.clone(),
        architecture: "x64".to_string(),
        language: 1033,
        per_machine: false,
        upgrade_code: None,
    };
    
    let result = build_msi(&manifest, &options).expect("Failed to build MSI");
    
    println!("MSI built: {} ({} bytes)", result.msi_path.display(), result.msi_size);
    println!("Files: {}", result.file_count);
    println!("Product code: {}", result.product_code);
    
    // Test installation
    let abs_path = std::fs::canonicalize(&result.msi_path).unwrap();
    let msi_path = abs_path.to_str().unwrap();
    // Strip UNC prefix (\\?\) which msiexec doesn't like
    let msi_path = msi_path.strip_prefix(r"\\?\").unwrap_or(msi_path);
    
    println!("\nTesting installation with msiexec...");
    println!("MSI path: {}", msi_path);
    
    let install_log = project_dir.join("install.log");
    let status = Command::new("msiexec")
        .args(&[
            "/i", msi_path,
            "/qn",
            "/norestart",
            "/l*v", install_log.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run msiexec");
    
    println!("msiexec exit code: {}", status.code().unwrap_or(-1));
    
    if status.success() {
        println!("✓ Installation successful!");
        
        // Verify files were installed
        let install_dir = format!("{}\\VelocityTestApp", std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()));
        if std::path::Path::new(&install_dir).exists() {
            println!("✓ Install directory exists: {}", install_dir);
            
            // List installed files
            if let Ok(entries) = std::fs::read_dir(&install_dir) {
                println!("\nInstalled files:");
                for entry in entries.flatten() {
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    println!("  {} ({} bytes)", entry.file_name().to_string_lossy(), size);
                }
            }
        } else {
            println!("✗ Install directory not found");
        }
        
        // Test uninstallation
        println!("\nTesting uninstallation...");
        let uninstall_status = Command::new("msiexec")
            .args(&[
                "/x", msi_path,
                "/qn",
                "/norestart",
            ])
            .status()
            .expect("Failed to run msiexec");
        
        println!("msiexec uninstall exit code: {}", uninstall_status.code().unwrap_or(-1));
        
        if uninstall_status.success() {
            println!("✓ Uninstallation successful!");
                    
            // Verify files were removed
            if !std::path::Path::new(&install_dir).exists() {
                println!("✓ Install directory removed");
            } else {
                println!("✗ Install directory still exists");
            }
        } else {
            println!("✗ Uninstallation failed");
        }
    } else {
        println!("✗ Installation failed");
        println!("Check install log: {}", install_log.display());
    }
}
