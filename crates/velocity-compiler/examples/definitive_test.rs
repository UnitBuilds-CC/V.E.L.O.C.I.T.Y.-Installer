/// Definitive test: build MSI with velocity-msi, test with msiexec
use std::io::{Cursor, Read};

fn main() {
    // === Build MSI with velocity-msi ===
    eprintln!("=== Building MSI with velocity-msi ===");
    let data = build_velocity_msi();
    let path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\velocity_test.msi";
    std::fs::write(path, &data).unwrap();
    eprintln!("Written: {} bytes -> {}", data.len(), path);

    // === Dump OLE structure ===
    eprintln!("\n=== OLE Structure ===");
    dump_ole_structure(&data);

    // === Try opening with msi crate ===
    eprintln!("\n=== Opening with msi crate ===");
    match msi::Package::open(Cursor::new(&data)) {
        Ok(pkg) => {
            eprintln!("OPENED successfully!");
            for table in pkg.tables() {
                eprintln!("  Table: {}", table.name());
            }
        }
        Err(e) => eprintln!("FAILED to open: {}", e),
    }

    // === Test with msiexec ===
    eprintln!("\n=== Testing with msiexec ===");
    let abs_path = std::fs::canonicalize(path).unwrap();
    let path_str = abs_path.to_str().unwrap().trim_start_matches(r"\\?\");
    let log_path = path.replace(".msi", ".log");

    let output = std::process::Command::new("msiexec.exe")
        .args(&["/i", path_str, "/qn", "/norestart", "/l*v", &log_path])
        .output()
        .expect("run msiexec");

    let code = output.status.code().unwrap_or(-1);
    eprintln!("msiexec exit code: {}", code);
    match code {
        0 => eprintln!("  *** SUCCESS ***"),
        1620 => eprintln!("  CANNOT open package"),
        1613 => eprintln!("  Cannot be installed"),
        1603 => eprintln!("  Fatal error during install"),
        1708 => eprintln!("  Installation failed"),
        _ => eprintln!("  Other error"),
    }

    // Read log tail
    if let Ok(log) = std::fs::read_to_string(&log_path) {
        let lines: Vec<&str> = log.lines().collect();
        let start = lines.len().saturating_sub(20);
        eprintln!("\nLog tail ({} lines):", lines.len() - start);
        for line in &lines[start..] {
            eprintln!("  {}", line);
        }
    }
}

fn build_velocity_msi() -> Vec<u8> {
    use velocity_msi::{Column, MsiBuilder, Value};

    let mut builder = MsiBuilder::new();
    builder.set_title("Velocity Test");
    builder.set_author("Test");
    builder.set_subject("Velocity MSI Test v1.0");
    builder.set_comments("Test package");
    builder.set_template("x64", 1033);

    // Property table
    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().localizable().build(),
    ]).unwrap();
    builder.insert_rows("Property", vec![
        vec![Value::from("ProductName"), Value::from("Velocity Test Product")],
        vec![Value::from("ProductVersion"), Value::from("1.0.0")],
        vec![Value::from("Manufacturer"), Value::from("Test Corp")],
        vec![Value::from("ProductCode"), Value::from("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}")],
        vec![Value::from("UpgradeCode"), Value::from("{11111111-2222-3333-4444-555555555555}")],
        vec![Value::from("ProductLanguage"), Value::from("1033")],
    ]).unwrap();

    builder.build().unwrap()
}

fn dump_ole_structure(data: &[u8]) {
    let mut comp = cfb::CompoundFile::open(Cursor::new(data)).unwrap();

    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    eprintln!("Streams: {}", streams.len());
    for (path, name) in &streams {
        let mut s = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        eprintln!("  {} ({} bytes): {:?}", name, buf.len(),
            if buf.len() <= 32 { &buf[..] } else { &buf[..16] });
    }
}
