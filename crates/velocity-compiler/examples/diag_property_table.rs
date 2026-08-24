//! Focused diagnostic: check Property table string references
use std::io::{Cursor, Read};

fn main() {
    let comp_data = std::fs::read("test_minimal.msi").unwrap();
    analyze_msi(&comp_data, "Compiler (minimal)");
}

fn analyze_msi(data: &[u8], label: &str) {
    println!("=== {} ===", label);
    let cursor = Cursor::new(data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    
    let paths: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string(), e.len() as usize))
        .collect();
    
    // Decode string pool
    let pool_name = velocity_msi::encode_stream_name("_StringPool", true);
    let data_name = velocity_msi::encode_stream_name("_StringData", true);
    
    let mut pool_bytes: Option<Vec<u8>> = None;
    let mut sdata_bytes: Option<Vec<u8>> = None;
    
    for (path, name, _) in &paths {
        let mut stream = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        if name == &pool_name { pool_bytes = Some(buf); }
        else if name == &data_name { sdata_bytes = Some(buf); }
    }
    
    let pool = pool_bytes.unwrap();
    let sdata = sdata_bytes.unwrap();
    
    let header = u32::from_le_bytes([pool[0], pool[1], pool[2], pool[3]]);
    let codepage = header & 0xFFFF;
    let long_refs = (header >> 31) != 0;
    let entry_size = if long_refs { 5 } else { 4 };
    let num_strings = (pool.len() - 4) / entry_size;
    
    println!("Codepage: {}, Long refs: {}, Strings: {}", codepage, long_refs, num_strings);
    
    // Decode ALL strings
    let mut strings: Vec<String> = Vec::new();
    let mut offset = 0usize;
    
    for i in 0..num_strings {
        let entry_start = 4 + i * entry_size;
        let length = u16::from_le_bytes([pool[entry_start], pool[entry_start + 1]]) as usize;
        
        let s = if length > 0 && offset + length <= sdata.len() {
            let bytes = &sdata[offset..offset + length];
            let decoded: String = bytes.iter().map(|&b| {
                if b < 128 { b as char } else { '?' }
            }).collect();
            offset += length;
            decoded
        } else {
            String::new()
        };
        strings.push(s);
    }
    
    // Find specific strings
    println!("\nLooking for key strings:");
    for (i, s) in strings.iter().enumerate() {
        if s == "ProductName" || s == "ProductVersion" || s == "ProductCode" 
            || s == "Manufacturer" || s == "Minimal Test" || s == "1.0.0"
            || s == "ProductLanguage" || s == "Description" || s == "UpgradeCode" {
            println!("  [{}] = {:?}", i, s);
        }
    }
    
    // Now read the Property table stream
    let prop_enc = velocity_msi::encode_stream_name("Property", true);
    println!("\nProperty stream encoded name: {:?}", prop_enc);
    
    for (path, name, size) in &paths {
        if name == &prop_enc {
            println!("Found Property stream: {} bytes", size);
            let mut stream = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).unwrap();
            println!("Raw hex: {:02x?}", &buf);
            
            // The Property table has 2 string columns: Property (PK), Value
            // Column-major: first all Property IDs, then all Value IDs
            // Short string refs: 2 bytes each
            // Total = num_rows * 2 * 2 = num_rows * 4
            let total_entries = buf.len() / 2;
            let num_rows = total_entries / 2;
            println!("Column entries: {}, Rows: {}", total_entries, num_rows);
            
            // First half = Property column (PK), second half = Value column
            for i in 0..num_rows {
                let name_id = u16::from_le_bytes([buf[i*2], buf[i*2+1]]) as usize;
                let value_id = u16::from_le_bytes([buf[num_rows*2 + i*2], buf[num_rows*2 + i*2+1]]) as usize;
                
                let name_str = strings.get(name_id).map(|s| s.as_str()).unwrap_or("<OOB>");
                let value_str = strings.get(value_id).map(|s| s.as_str()).unwrap_or("<OOB>");
                
                println!("  Row {}: name_id={} ({:?}), value_id={} ({:?})", i, name_id, name_str, value_id, value_str);
            }
            break;
        }
    }
    
    // Also list ALL streams with their encoded names
    println!("\nAll streams:");
    for (path, name, size) in &paths {
        println!("  {:?} ({} bytes)", name, size);
    }
}
