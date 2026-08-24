/// Compare _Columns data between test MSI and compiler MSI
use std::io::{Cursor, Read};

fn main() {
    // Read both MSIs
    let test_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\replicate_compiler.msi";
    let comp_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";

    // Check if test MSI exists
    if !std::path::Path::new(test_path).exists() {
        eprintln!("Test MSI not found at {}. Run test_replicate_exact first.", test_path);
        // Build a simple test MSI using velocity-msi directly
        eprintln!("Building test MSI via velocity-msi...");
        build_test_msi();
    }

    let test_data = std::fs::read(test_path).expect("read test MSI");
    let comp_data = std::fs::read(comp_path).expect("read compiler MSI");

    let test_cols = get_stream(&test_data, "_Columns");
    let comp_cols = get_stream(&comp_data, "_Columns");

    eprintln!("Test _Columns: {} bytes", test_cols.len());
    eprintln!("Comp _Columns: {} bytes", comp_cols.len());

    // Get string pools for both
    let test_pool = decode_string_pool(&test_data);
    let comp_pool = decode_string_pool(&comp_data);

    // Decode _Columns for both
    eprintln!("\n=== Test MSI _Columns ===");
    decode_columns(&test_cols, &test_pool);

    eprintln!("\n=== Compiler MSI _Columns ===");
    decode_columns(&comp_cols, &comp_pool);
}

fn build_test_msi() {
    // Minimal test using velocity-msi
    let mut builder = velocity_msi::MsiBuilder::new();
    builder.set_title("Test");
    builder.set_author("Test");
    builder.set_template("x64", 1033);

    builder.create_table("Property", vec![
        velocity_msi::Column::build("Property").string(72).primary_key().build(),
        velocity_msi::Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    builder.insert_rows("Property", vec![
        vec![velocity_msi::Value::from("ProductName"), velocity_msi::Value::from("Test")],
        vec![velocity_msi::Value::from("ProductVersion"), velocity_msi::Value::from("1.0")],
        vec![velocity_msi::Value::from("Manufacturer"), velocity_msi::Value::from("Test")],
    ]).unwrap();

    let data = builder.build().unwrap();
    std::fs::write(r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\replicate_compiler.msi", &data).unwrap();
    eprintln!("Test MSI written ({} bytes)", data.len());
}

fn get_stream(msi_data: &[u8], table_name: &str) -> Vec<u8> {
    let cursor = Cursor::new(msi_data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    let target = velocity_msi::encode_stream_name(table_name, true);

    let stream_info: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    for (path, name) in &stream_info {
        if name == &target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            return buf;
        }
    }
    panic!("Stream {} not found", table_name);
}

fn decode_string_pool(msi_data: &[u8]) -> Vec<String> {
    let cursor = Cursor::new(msi_data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    let pool_target = velocity_msi::encode_stream_name("_StringPool", true);
    let data_target = velocity_msi::encode_stream_name("_StringData", true);

    let stream_info: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    let mut pool_data = None;
    let mut string_data = None;
    for (path, name) in &stream_info {
        let mut s = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        if name == &pool_target { pool_data = Some(buf); }
        if name == &data_target { string_data = Some(buf); }
    }

    let pool_data = pool_data.unwrap();
    let string_data = string_data.unwrap();

    let mut offset = 4; // skip header
    let mut entries = Vec::new();
    while offset + 4 <= pool_data.len() {
        let len = u16::from_le_bytes([pool_data[offset], pool_data[offset+1]]);
        entries.push(len as usize);
        offset += 4;
    }

    let mut strings = vec![String::new()]; // ID 0
    let mut data_off = 0;
    for len in &entries {
        if data_off + len > string_data.len() { break; }
        let s = String::from_utf8_lossy(&string_data[data_off..data_off+*len]).to_string();
        strings.push(s);
        data_off += len;
    }
    strings
}

fn decode_columns(data: &[u8], pool: &[String]) {
    // _Columns: 4 columns (Table=string, Number=int16, Name=string, Type=int32)
    // Column-major: all Table values, all Number values, all Name values, all Type values
    // Short string refs: 2 bytes each
    // Int16: 2 bytes, Int32: 4 bytes

    // Calculate row count:
    // Table col: row_count * 2 bytes
    // Number col: row_count * 2 bytes
    // Name col: row_count * 2 bytes
    // Type col: row_count * 4 bytes
    // Total = row_count * (2+2+2+4) = row_count * 10
    let row_count = data.len() / 10;
    eprintln!("Row count: {}", row_count);

    let mut off = 0;
    // Col 1: Table (string refs)
    let mut table_ids: Vec<u16> = Vec::new();
    for _ in 0..row_count {
        table_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    // Col 2: Number (int16, XOR encoded)
    let mut numbers: Vec<i32> = Vec::new();
    for _ in 0..row_count {
        let raw = u16::from_le_bytes([data[off], data[off+1]]);
        numbers.push((raw ^ 0x8000) as i16 as i32);
        off += 2;
    }
    // Col 3: Name (string refs)
    let mut name_ids: Vec<u16> = Vec::new();
    for _ in 0..row_count {
        name_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    // Col 4: Type (int32, XOR encoded)
    let mut types: Vec<i32> = Vec::new();
    for _ in 0..row_count {
        let raw = i32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]);
        types.push(raw ^ -0x80000000i32);
        off += 4;
    }

    // Print rows grouped by table
    let mut current_table = String::new();
    for i in 0..row_count {
        let tname = if (table_ids[i] as usize) < pool.len() { &pool[table_ids[i] as usize] } else { "?" };
        let cname = if (name_ids[i] as usize) < pool.len() { &pool[name_ids[i] as usize] } else { "?" };
        if tname != &current_table {
            current_table = tname.clone();
            eprintln!("\n  Table: {}", tname);
        }
        eprintln!("    Col {}: {} bitfield=0x{:04X}", numbers[i], cname, types[i] as u32);
    }
}
