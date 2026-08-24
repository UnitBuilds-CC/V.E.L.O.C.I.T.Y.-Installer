/// Debug: check what tables are in all_tables during build
use std::io::{Cursor, Read};
use velocity_msi::{MsiBuilder, Column, Value};

fn main() {
    let mut builder = MsiBuilder::new();
    builder.set_title("Test");
    builder.set_author("Test");
    builder.set_template("x64", 1033);

    // Create tables matching the compiler
    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("Directory", vec![
        Column::build("Directory").string(72).primary_key().build(),
        Column::build("Directory_Parent").string(72).nullable().build(),
        Column::build("DefaultDir").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("Component", vec![
        Column::build("Component").string(72).primary_key().build(),
        Column::build("ComponentId").string(38).nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("KeyPath").string(72).nullable().build(),
    ]).unwrap();
    builder.create_table("CustomAction", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Type").int16().nullable().build(),
        Column::build("Source").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("InstallExecuteSequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).unwrap();

    // Only populate SOME tables (CustomAction stays empty)
    builder.insert_rows("Property", vec![
        vec![Value::from("ProductName"), Value::from("Test")],
    ]).unwrap();
    builder.insert_rows("Directory", vec![
        vec![Value::from("TARGETDIR"), Value::Null, Value::from("SourceDir")],
    ]).unwrap();
    builder.insert_rows("Component", vec![
        vec![Value::from("comp0"), Value::Null, Value::from("TARGETDIR"), Value::Int(0), Value::Null, Value::from("file_0")],
    ]).unwrap();
    // CustomAction: 0 rows (empty)
    builder.insert_rows("InstallExecuteSequence", vec![
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(800)],
    ]).unwrap();

    // Build and inspect
    let msi_data = builder.build().unwrap();
    
    let cursor = Cursor::new(&msi_data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    
    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();
    
    let pool = decode_string_pool(&mut comp, &streams);
    
    // Dump _Tables
    let tables_target = velocity_msi::encode_stream_name("_Tables", true);
    for (path, name) in &streams {
        if name == &tables_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            let row_count = buf.len() / 2;
            eprintln!("_Tables: {} bytes, {} rows", buf.len(), row_count);
            for i in 0..row_count {
                let id = u16::from_le_bytes([buf[i*2], buf[i*2+1]]);
                let tname = if (id as usize) < pool.len() { &pool[id as usize] } else { "?" };
                eprintln!("  [{}] {} (id={})", i, tname, id);
            }
            break;
        }
    }
    
    // Dump _Columns - proper column-major decode
    let cols_target = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &streams {
        if name == &cols_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            let row_count = buf.len() / 10;
            eprintln!("\n_Columns: {} bytes, {} rows", buf.len(), row_count);
            
            // Column-major order: Table(str), Number(int16), Name(str), Type(int16)
            let mut off = 0;
            // Column 0: Table (string refs, u16)
            let mut table_ids = Vec::new();
            for _ in 0..row_count { 
                table_ids.push(u16::from_le_bytes([buf[off], buf[off+1]])); 
                off += 2; 
            }
            // Column 1: Number (int16, XOR encoded)
            let mut numbers = Vec::new();
            for _ in 0..row_count { 
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                numbers.push((raw ^ 0x8000) as i16 as i32);
                off += 2; 
            }
            // Column 2: Name (string refs, u16)
            let mut name_ids = Vec::new();
            for _ in 0..row_count { 
                name_ids.push(u16::from_le_bytes([buf[off], buf[off+1]])); 
                off += 2; 
            }
            // Column 3: Type (int16, XOR encoded)
            let mut types = Vec::new();
            for _ in 0..row_count { 
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                types.push((raw ^ 0x8000) as i16 as i32);
                off += 2; 
            }
            
            // Print grouped by table
            let mut current_table = String::new();
            let mut seen = std::collections::HashMap::new();
            for i in 0..row_count {
                let tname = if (table_ids[i] as usize) < pool.len() { pool[table_ids[i] as usize].as_str() } else { "?" };
                let cname = if (name_ids[i] as usize) < pool.len() { pool[name_ids[i] as usize].as_str() } else { "?" };
                if tname != current_table.as_str() {
                    current_table = tname.to_string();
                    eprintln!("\n  Table: {} (pool_id={})", tname, table_ids[i]);
                }
                let key = format!("({}, {})", tname, numbers[i]);
                let count = seen.entry(key.clone()).or_insert(0);
                *count += 1;
                let dup = if *count > 1 { " *** DUP ***" } else { "" };
                eprintln!("    Col {}: {} type=0x{:04X}{}", numbers[i], cname, types[i] as u16, dup);
            }
            
            eprintln!("\n=== Duplicate check ===");
            let mut has_dups = false;
            for (key, count) in &seen {
                if *count > 1 {
                    eprintln!("  {} appears {} times", key, count);
                    has_dups = true;
                }
            }
            if !has_dups {
                eprintln!("  No duplicates!");
            }
            
            // Expected row count
            eprintln!("\nExpected: {} tables × avg columns", row_count);
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
