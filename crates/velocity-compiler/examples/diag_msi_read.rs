/// Use msi crate to read compiler MSI and dump all table data
use std::io::Cursor;

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");

    let cursor = Cursor::new(data.clone());
    let pkg = msi::Package::open(cursor).expect("open MSI package");

    eprintln!("=== MSI Package Tables ===");
    for table in pkg.tables() {
        eprintln!("\nTable: {} ({} rows)", table.name(), table.rows().count());
        // Print column info
        for col in table.columns() {
            eprintln!("  Col: {} (type={:?}, pk={})", col.name(), col.type_ref(), col.is_primary_key());
        }
        // Print rows (limit to first 5)
        for (i, row) in table.rows().enumerate().take(5) {
            let vals: Vec<String> = row.values().map(|v| format!("{:?}", v)).collect();
            eprintln!("  Row {}: {:?}", i, vals);
        }
        if table.rows().count() > 5 {
            eprintln!("  ... ({} more rows)", table.rows().count() - 5);
        }
    }
}
