/// Try to open compiler MSI with msi crate and read tables
use std::io::Cursor;

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");

    let cursor = Cursor::new(data);
    match msi::Package::open(cursor) {
        Ok(pkg) => {
            eprintln!("msi crate opened MSI successfully!");
            for table in pkg.tables() {
                eprintln!("Table: {}", table.name());
            }
        }
        Err(e) => {
            eprintln!("msi crate FAILED to open MSI: {}", e);
        }
    }
}
