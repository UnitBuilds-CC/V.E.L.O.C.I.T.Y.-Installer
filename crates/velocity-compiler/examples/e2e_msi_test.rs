/// End-to-end test: build MSI through velocity-compiler pipeline and test with msiexec.
/// This exercises the FULL pipeline: manifest → tables → cabinet → OLE → msiexec.
use std::io::Cursor;
use std::path::PathBuf;
use velocity_compiler::msi_builder::{build_msi, MsiOptions};

fn main() {
    std::fs::create_dir_all("C:\\temp").ok();

    // Point to the sample-app example
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_dir = manifest_dir
        .parent().unwrap()  // crates/
        .parent().unwrap()  // workspace root
        .join("examples")
        .join("sample-app");

    println!("Manifest dir: {}", manifest_dir.display());
    println!("Project dir: {}", project_dir.display());
    assert!(project_dir.exists(), "Sample app dir must exist: {}", project_dir.display());

    // Parse the sample manifest
    let toml_path = project_dir.join("velocity.toml");
    let toml_str = std::fs::read_to_string(&toml_path).expect("read velocity.toml");
    let manifest: velocity_config::VelocityManifest =
        velocity_config::parse_manifest_str(&toml_str).expect("parse manifest");

    println!("App: {} v{}", manifest.app.name, manifest.app.version);

    // Build MSI through the full compiler pipeline
    let output_path = PathBuf::from("C:\\temp\\e2e_sample_app.msi");
    let options = MsiOptions {
        output_path: output_path.clone(),
        project_dir: project_dir.clone(),
        architecture: "x64".to_string(),
        language: 1033,
        per_machine: true,
        upgrade_code: None,
    };

    println!("\n=== Building MSI through velocity-compiler pipeline ===");
    match build_msi(&manifest, &options) {
        Ok(result) => {
            println!("MSI built successfully!");
            println!("  Path: {}", result.msi_path.display());
            println!("  Size: {} bytes", result.msi_size);
            println!("  Files: {}", result.file_count);
            println!("  Components: {}", result.component_count);
            println!("  ProductCode: {}", result.product_code);
            println!("  UpgradeCode: {}", result.upgrade_code);
        }
        Err(e) => {
            println!("BUILD FAILED: {}", e);
            std::process::exit(1);
        }
    }

    // Test with msiexec
    println!("\n=== msiexec test ===");
    let output = std::process::Command::new("msiexec.exe")
        .args(&["/i", output_path.to_str().unwrap(), "/quiet", "/norestart"])
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    let status = match code {
        0 => "SUCCESS (installed!)",
        1603 => "FATAL ERROR (during install)",
        1613 => "OK (opens, can't repair)",
        1618 => "Another install in progress",
        1620 => "FAIL (can't open - THIS IS THE BUG)",
        1625 => "OK (opens, blocked by policy)",
        _ => "OTHER",
    };
    println!("msiexec exit code: {} ({})", code, status);

    if code == 1620 {
        println!("\n*** ERROR 1620 STILL PRESENT - BUG NOT FIXED ***");
        std::process::exit(2);
    }

    // Verify with msi crate
    println!("\n=== msi crate verification ===");
    let msi_data = std::fs::read(&output_path).unwrap();
    match msi::Package::open(Cursor::new(&msi_data)) {
        Ok(pkg) => {
            println!("msi crate can read our MSI!");
            let table_names: Vec<String> = pkg.tables().map(|t| t.name().to_string()).collect();
            println!("  Tables ({}):", table_names.len());
            for name in &table_names {
                println!("    - {}", name);
            }

            // Read Property table
            println!("\n  Property table contents:");
            let mut pkg2 = msi::Package::open(Cursor::new(&msi_data)).unwrap();
            let query = msi::Select::table("Property");
            if let Ok(rows) = pkg2.select_rows(query) {
                for row in rows {
                    let prop_name = format!("{}", &row["Property"]);
                    let prop_val = format!("{}", &row["Value"]);
                    println!("    {} = {}", prop_name, prop_val);
                }
            }
        }
        Err(e) => {
            println!("msi crate FAILED: {}", e);
        }
    }

    // Verify with cfb
    println!("\n=== OLE structure verification ===");
    match cfb::CompoundFile::open(Cursor::new(&msi_data)) {
        Ok(comp) => {
            println!("cfb can read our OLE structure!");
            let entries: Vec<String> = comp.walk().map(|e| e.name().to_string()).collect();
            println!("  Entries: {}", entries.len());
            for name in &entries {
                println!("    - {}", name);
            }
        }
        Err(e) => println!("cfb FAILED: {}", e),
    }

    println!("\n=== RESULT ===");
    if code != 1620 {
        println!("SUCCESS: msiexec can open our MSI (exit code {})", code);
    } else {
        println!("FAILURE: msiexec cannot open our MSI (error 1620)");
    }
}
