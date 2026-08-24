//! Diagnose: compare compiler MSI vs test MSI using msi crate and cfb
use std::process::Command;

fn main() {
    println!("=== Diagnose compiler MSI vs test MSI ===\n");

    // Check if both files exist
    let compiler_msi = "test_compiler.msi";
    let test_msi = "replicate_compiler.msi";
    
    if !std::path::Path::new(compiler_msi).exists() {
        println!("ERROR: {} not found. Run velocity build first.", compiler_msi);
        return;
    }
    if !std::path::Path::new(test_msi).exists() {
        println!("ERROR: {} not found. Run test_replicate_exact first.", test_msi);
        return;
    }

    let comp_data = std::fs::read(compiler_msi).unwrap();
    let test_data = std::fs::read(test_msi).unwrap();
    println!("Compiler MSI: {} bytes", comp_data.len());
    println!("Test MSI:     {} bytes", test_data.len());

    // Try opening with msi crate
    println!("\n--- Opening with msi crate ---");
    match msi::Package::open(std::io::Cursor::new(comp_data.clone())) {
        Ok(pkg) => {
            println!("Compiler MSI: OPENED OK");
            // Try to read properties
            let tables: Vec<_> = pkg.tables().map(|t| t.name().to_string()).collect();
            println!("  Tables: {:?}", tables);
        }
        Err(e) => {
            println!("Compiler MSI: OPEN FAILED: {}", e);
        }
    }

    match msi::Package::open(std::io::Cursor::new(test_data.clone())) {
        Ok(pkg) => {
            println!("Test MSI: OPENED OK");
            let tables: Vec<_> = pkg.tables().map(|t| t.name().to_string()).collect();
            println!("  Tables: {:?}", tables);
        }
        Err(e) => {
            println!("Test MSI: OPEN FAILED: {}", e);
        }
    }

    // Compare OLE structure using cfb
    println!("\n--- OLE structure comparison ---");
    compare_ole_structure(&comp_data, "Compiler");
    compare_ole_structure(&test_data, "Test");

    // Test both with msiexec
    println!("\n--- msiexec test ---");
    for (path, label) in &[(compiler_msi, "Compiler"), (test_msi, "Test")] {
        let log = format!("{}_diag.log", label);
        let status = Command::new("msiexec.exe")
            .args(&["/i", path, "/qn", "/l*v", &log])
            .status()
            .unwrap();
        let code = status.code().unwrap_or(-1);
        println!("{}: exit code {}", label, code);
        
        // Check log for key info
        if let Ok(content) = std::fs::read_to_string(&log) {
            for line in content.lines() {
                if line.contains("Product Name") && line.contains(":") {
                    println!("  {}", line.trim());
                }
            }
        }
    }
}

fn compare_ole_structure(data: &[u8], label: &str) {
    use std::io::Cursor;
    
    let cursor = Cursor::new(data);
    match cfb::CompoundFile::open(cursor) {
        Ok(comp) => {
            println!("{}: Valid OLE, version={:?}", label, comp.version());
            
            // Walk all entries
            let mut stream_count = 0;
            let mut total_stream_size = 0u64;
            for entry in comp.walk() {
                if entry.is_stream() {
                    stream_count += 1;
                    let size = entry.len();
                    total_stream_size += size;
                    // Show stream name (may be encoded)
                    let name = entry.name();
                    if name.len() < 50 {
                        println!("  Stream: {:?} ({} bytes)", name, size);
                    } else {
                        println!("  Stream: {:?}... ({} bytes)", &name[..50], size);
                    }
                }
            }
            println!("  Total: {} streams, {} bytes", stream_count, total_stream_size);
        }
        Err(e) => {
            println!("{}: INVALID OLE: {}", label, e);
        }
    }
}
