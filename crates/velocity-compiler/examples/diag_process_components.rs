/// Diagnostic: Read compiler MSI with msi crate and dump all install-relevant tables.
use std::io::Cursor;
use msi::{Select, Value};

fn main() {
    let comp_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\test_compiler.msi";
    let comp_data = std::fs::read(comp_path).unwrap();
    
    eprintln!("=== Reading compiler MSI ({} bytes) ===", comp_data.len());
    
    let cursor = Cursor::new(comp_data);
    let mut pkg: msi::Package<Cursor<Vec<u8>>> = msi::Package::open(cursor).expect("open compiler MSI");
    
    // List all table names
    eprintln!("\n--- All tables ---");
    for table in pkg.tables() {
        eprintln!("  {} ({} cols)", table.name(), table.columns().len());
    }
    
    // Dump key tables using select_rows
    let key_tables = [
        "Property", "Directory", "Component", "File", "Media",
        "Feature", "FeatureComponents", "InstallExecuteSequence",
        "_Tables",
    ];
    
    for tname in &key_tables {
        eprintln!("\n=== {} ===", tname);
        match pkg.select_rows(Select::table(*tname)) {
            Ok(rows) => {
                for (i, row) in rows.enumerate() {
                    if i >= 30 {
                        eprintln!("  ... (truncated)");
                        break;
                    }
                    let mut vals = Vec::new();
                    for j in 0..row.len() {
                        vals.push(format!("{:?}", &row[j]));
                    }
                    eprintln!("  [{}] {}", i, vals.join(" | "));
                }
            }
            Err(e) => eprintln!("  ERROR: {}", e),
        }
    }
    
    // Decode _Columns bitfields for Component and File
    eprintln!("\n=== _Columns bitfield decode (Component + File) ===");
    match pkg.select_rows(Select::table("_Columns")) {
        Ok(rows) => {
            for row in rows {
                let table_name = format!("{:?}", &row[0]);
                if table_name.contains("Component") || table_name.contains("File") {
                    if let Value::Int(t) = row[3] {
                        let is_string = (t & 0x800) != 0;
                        let is_nullable = (t & 0x1000) != 0;
                        let is_pk = (t & 0x2000) != 0;
                        let size = t & 0xFF;
                        eprintln!("  {}.{}: raw=0x{:04X} size={} str={} null={} pk={}",
                            table_name, format!("{:?}", &row[2]), t, size, is_string, is_nullable, is_pk);
                    }
                }
            }
        }
        Err(e) => eprintln!("  ERROR: {}", e),
    }
    
    // Dump _Validation for Component
    eprintln!("\n=== _Validation for Component ===");
    match pkg.select_rows(Select::table("_Validation")) {
        Ok(rows) => {
            for row in rows {
                let table_name = format!("{:?}", &row[0]);
                if table_name.contains("Component") {
                    let mut vals = Vec::new();
                    for j in 0..row.len() {
                        vals.push(format!("{:?}", &row[j]));
                    }
                    eprintln!("  {}", vals.join(" | "));
                }
            }
        }
        Err(e) => eprintln!("  ERROR: {}", e),
    }
}
