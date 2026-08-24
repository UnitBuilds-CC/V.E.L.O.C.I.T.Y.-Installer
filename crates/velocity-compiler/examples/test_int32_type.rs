/// Test: minimal MSI with _Columns.Type = Int32, test with msiexec
use velocity_msi::{MsiBuilder, Column, Value};
use std::process::Command;

fn main() {
    let mut builder = MsiBuilder::new();
    builder.set_title("Test Product");
    builder.set_author("Test Author");
    builder.set_template("x64", 1033);

    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    builder.insert_rows("Property", vec![
        vec![Value::from("ProductName"), Value::from("Test Product")],
        vec![Value::from("ProductVersion"), Value::from("1.0")],
        vec![Value::from("Manufacturer"), Value::from("Test")],
        vec![Value::from("ProductCode"), Value::from("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}")],
        vec![Value::from("UpgradeCode"), Value::from("{11111111-2222-3333-4444-555555555555}")],
        vec![Value::from("ProductLanguage"), Value::from("1033")],
    ]).unwrap();

    let msi_data = builder.build().unwrap();
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-app/output");
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = out_dir.join("test_int32_type.msi");
    std::fs::write(&path, &msi_data).unwrap();
    eprintln!("Wrote {} bytes to {}", msi_data.len(), path.display());

    // Test with msi crate
    let cursor = std::io::Cursor::new(&msi_data);
    match msi::Package::open(cursor) {
        Ok(pkg) => {
            eprintln!("msi crate: OK - {} tables", pkg.tables().count());
            for t in pkg.tables() {
                eprintln!("  Table: {}", t.name());
            }
        }
        Err(e) => eprintln!("msi crate: FAILED: {}", e),
    }

    // Test with msiexec
    let msi_path = std::fs::canonicalize(&path).unwrap();
    let msi_str = msi_path.to_str().unwrap().trim_start_matches(r"\\?\").to_string();
    eprintln!("msi path: {}", msi_str);
    let status = Command::new("msiexec")
        .args(&["/i", &msi_str, "/qn", "/norestart"])
        .status()
        .expect("Failed to run msiexec");
    eprintln!("msiexec exit code: {}", status.code().unwrap_or(-1));

    // Try to uninstall
    let status2 = Command::new("msiexec")
        .args(&["/x", "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}", "/qn", "/norestart"])
        .status();
    if let Ok(s) = status2 {
        eprintln!("uninstall exit code: {}", s.code().unwrap_or(-1));
    }
}
