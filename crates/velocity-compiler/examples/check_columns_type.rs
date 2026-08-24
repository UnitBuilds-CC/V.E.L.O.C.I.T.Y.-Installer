/// Check _Columns Type column width in velocity-msi output
use std::io::{Cursor, Read};

fn main() {
    // Build a minimal MSI with velocity-msi
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
    ]).unwrap();
    let data = builder.build().unwrap();

    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();

    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    // Get string pool
    let pool = decode_string_pool(&mut comp, &streams);

    // Find _Columns stream
    let cols_target = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &streams {
        if name == &cols_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();

            eprintln!("_Columns stream: {} bytes", buf.len());
            eprintln!("buf.len() % 10 = {} (0 = clean Int16 Type)", buf.len() % 10);
            eprintln!("buf.len() % 12 = {} (0 = clean Int32 Type)", buf.len() % 12);

            // Try decoding as Int16 Type (10 bytes per row)
            if buf.len() % 10 == 0 {
                let row_count = buf.len() / 10;
                eprintln!("\nDecoding as Int16 Type: {} rows", row_count);
                decode_columns_int16(&buf, row_count, &pool);
            }

            // Try decoding as Int32 Type (12 bytes per row)
            if buf.len() % 12 == 0 {
                let row_count = buf.len() / 12;
                eprintln!("\nDecoding as Int32 Type: {} rows", row_count);
                decode_columns_int32(&buf, row_count, &pool);
            }

            // Also check the _Columns column definitions in our code
            eprintln!("\n=== Expected column definitions ===");
            eprintln!("_Columns.Type is defined as Int16 in our code");
            eprintln!("MSI spec says _Columns.Type should be LONG (Int32, 4 bytes)");
            eprintln!("This means we write 2 bytes per Type value instead of 4!");

            return;
        }
    }
    eprintln!("_Columns stream not found!");
}

fn decode_columns_int16(data: &[u8], row_count: usize, pool: &[String]) {
    let mut off = 0;
    let mut table_ids = Vec::new();
    for _ in 0..row_count {
        table_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    let mut numbers = Vec::new();
    for _ in 0..row_count {
        let raw = u16::from_le_bytes([data[off], data[off+1]]);
        numbers.push((raw ^ 0x8000) as i16 as i32);
        off += 2;
    }
    let mut name_ids = Vec::new();
    for _ in 0..row_count {
        name_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    let mut types = Vec::new();
    for _ in 0..row_count {
        let raw = u16::from_le_bytes([data[off], data[off+1]]);
        types.push((raw ^ 0x8000) as i16 as i32);
        off += 2;
    }

    let mut current_table = String::new();
    for i in 0..row_count {
        let tname = if (table_ids[i] as usize) < pool.len() { pool[table_ids[i] as usize].as_str() } else { "?" };
        let cname = if (name_ids[i] as usize) < pool.len() { pool[name_ids[i] as usize].as_str() } else { "?" };
        if tname != current_table.as_str() {
            current_table = tname.to_string();
            eprintln!("  Table: {}", tname);
        }
        eprintln!("    Col {}: {} type=0x{:04X}", numbers[i], cname, types[i] as u16);
    }
}

fn decode_columns_int32(data: &[u8], row_count: usize, pool: &[String]) {
    let mut off = 0;
    let mut table_ids = Vec::new();
    for _ in 0..row_count {
        table_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    let mut numbers = Vec::new();
    for _ in 0..row_count {
        let raw = u16::from_le_bytes([data[off], data[off+1]]);
        numbers.push((raw ^ 0x8000) as i16 as i32);
        off += 2;
    }
    let mut name_ids = Vec::new();
    for _ in 0..row_count {
        name_ids.push(u16::from_le_bytes([data[off], data[off+1]]));
        off += 2;
    }
    let mut types = Vec::new();
    for _ in 0..row_count {
        let raw = i32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]]);
        types.push(raw ^ -0x80000000i32);
        off += 4;
    }

    let mut current_table = String::new();
    for i in 0..row_count {
        let tname = if (table_ids[i] as usize) < pool.len() { pool[table_ids[i] as usize].as_str() } else { "?" };
        let cname = if (name_ids[i] as usize) < pool.len() { pool[name_ids[i] as usize].as_str() } else { "?" };
        if tname != current_table.as_str() {
            current_table = tname.to_string();
            eprintln!("  Table: {}", tname);
        }
        eprintln!("    Col {}: {} type=0x{:08X}", numbers[i], cname, types[i] as u32);
    }
}

fn decode_string_pool(comp: &mut cfb::CompoundFile<Cursor<&Vec<u8>>>, streams: &[(std::path::PathBuf, String)]) -> Vec<String> {
    let pool_target = velocity_msi::encode_stream_name("_StringPool", true);
    let data_target = velocity_msi::encode_stream_name("_StringData", true);
    let mut pool_data: Option<Vec<u8>> = None;
    let mut string_data: Option<Vec<u8>> = None;
    for (path, name) in streams {
        if name == &pool_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            pool_data = Some(buf);
        }
        if name == &data_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            string_data = Some(buf);
        }
    }
    let pool_data = pool_data.unwrap();
    let string_data = string_data.unwrap();
    let mut offset = 4;
    let mut entries = Vec::new();
    while offset + 4 <= pool_data.len() {
        let len = u16::from_le_bytes([pool_data[offset], pool_data[offset+1]]);
        entries.push(len as usize);
        offset += 4;
    }
    let mut strings = vec![String::new()];
    let mut data_off = 0;
    for len in &entries {
        if data_off + len > string_data.len() { break; }
        let s = String::from_utf8_lossy(&string_data[data_off..data_off+*len]).to_string();
        strings.push(s);
        data_off += len;
    }
    strings
}
