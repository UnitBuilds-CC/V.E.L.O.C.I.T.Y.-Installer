/// Debug: hex dump of _Columns stream to understand actual byte layout
use std::io::{Cursor, Read};
use velocity_msi::{MsiBuilder, Column, Value};

fn main() {
    let mut builder = MsiBuilder::new();
    builder.set_title("Test");
    builder.set_author("Test");
    builder.set_template("x64", 1033);

    // Minimal: just Property table
    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    builder.insert_rows("Property", vec![
        vec![Value::from("ProductName"), Value::from("Test")],
        vec![Value::from("ProductVersion"), Value::from("1.0")],
    ]).unwrap();

    let msi_data = builder.build().unwrap();
    
    let cursor = Cursor::new(&msi_data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    
    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();
    
    let pool = decode_string_pool(&mut comp, &streams);
    
    eprintln!("String pool:");
    for (i, s) in pool.iter().enumerate() {
        if !s.is_empty() {
            eprintln!("  [{}] = '{}'", i, s);
        }
    }
    
    // Find and hex dump _Columns
    let cols_target = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &streams {
        if name == &cols_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            
            eprintln!("\n_Columns: {} bytes", buf.len());
            // _Columns has 4 cols: Table(str,2), Number(int16,2), Name(str,2), Type(int16,2) = 10 bytes/row
            let row_count = buf.len() / 10;
            eprintln!("Row count: {}", row_count);
            
            // Hex dump with interpretation
            eprintln!("\n=== Raw hex dump ===");
            for (i, chunk) in buf.chunks(2).enumerate() {
                let val = u16::from_le_bytes([chunk[0], chunk.get(1).copied().unwrap_or(0)]);
                eprintln!("  offset {:3} (chunk {:2}): 0x{:04X} = {} (as signed: {})", 
                    i*2, i,
                    val, val,
                    (val ^ 0x8000) as i16);
            }
            
            // Try interpreting in SCHEMA order: Table, Number, Name, Type
            eprintln!("\n=== Schema order: Table(str), Number(int16), Name(str), Type(int16) ===");
            let mut off = 0;
            eprintln!("Column 0 (Table, str):");
            for r in 0..row_count {
                let id = u16::from_le_bytes([buf[off], buf[off+1]]);
                let s = if (id as usize) < pool.len() { &pool[id as usize] } else { "?" };
                eprintln!("  row {}: id={} '{}'", r, id, s);
                off += 2;
            }
            eprintln!("Column 1 (Number, int16):");
            for r in 0..row_count {
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                let decoded = (raw ^ 0x8000) as i16;
                eprintln!("  row {}: raw=0x{:04X} decoded={}", r, raw, decoded);
                off += 2;
            }
            eprintln!("Column 2 (Name, str):");
            for r in 0..row_count {
                let id = u16::from_le_bytes([buf[off], buf[off+1]]);
                let s = if (id as usize) < pool.len() { &pool[id as usize] } else { "?" };
                eprintln!("  row {}: id={} '{}'", r, id, s);
                off += 2;
            }
            eprintln!("Column 3 (Type, int16):");
            for r in 0..row_count {
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                let decoded = (raw ^ 0x8000) as i16;
                eprintln!("  row {}: raw=0x{:04X} decoded=0x{:04X}", r, raw, decoded as u16);
                off += 2;
            }
            
            return;
        }
    }
    eprintln!("_Columns stream not found!");
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
