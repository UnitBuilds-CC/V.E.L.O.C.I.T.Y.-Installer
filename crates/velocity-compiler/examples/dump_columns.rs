/// Dump ALL _Columns entries from the compiler MSI to find duplicates
use std::io::{Cursor, Read};

fn main() {
    let path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\test_compiler.msi";
    let data = std::fs::read(path).unwrap();
    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();

    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    // Decode string pool
    let pool = decode_string_pool(&mut comp, &streams);

    // Find _Columns stream
    let cols_target = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &streams {
        if name == &cols_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();

            // _Columns: Table(str,2) + Number(int16,2) + Name(str,2) + Type(int16,2) = 10 bytes/row
            let row_count = buf.len() / 10;
            eprintln!("_Columns: {} bytes, {} rows", buf.len(), row_count);

            // Column-major decode
            let mut off = 0;
            let mut table_ids = Vec::new();
            for _ in 0..row_count {
                table_ids.push(u16::from_le_bytes([buf[off], buf[off+1]]));
                off += 2;
            }
            let mut numbers = Vec::new();
            for _ in 0..row_count {
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                numbers.push((raw ^ 0x8000) as i16 as i32);
                off += 2;
            }
            let mut name_ids = Vec::new();
            for _ in 0..row_count {
                name_ids.push(u16::from_le_bytes([buf[off], buf[off+1]]));
                off += 2;
            }
            let mut types = Vec::new();
            for _ in 0..row_count {
                let raw = u16::from_le_bytes([buf[off], buf[off+1]]);
                types.push((raw ^ 0x8000) as i16 as i32);
                off += 2;
            }

            // Print all rows, looking for duplicates
            let mut seen = std::collections::HashMap::new();
            let mut current_table = String::new();
            for i in 0..row_count {
                let tname = if (table_ids[i] as usize) < pool.len() { pool[table_ids[i] as usize].as_str() } else { "?" };
                let cname = if (name_ids[i] as usize) < pool.len() { pool[name_ids[i] as usize].as_str() } else { "?" };
                let num = numbers[i];

                if tname != current_table.as_str() {
                    current_table = tname.to_string();
                    eprintln!("\n  Table: {}", tname);
                }

                let key = format!("({}, {})", tname, num);
                let count = seen.entry(key.clone()).or_insert(0);
                *count += 1;
                let dup = if *count > 1 { " *** DUPLICATE ***" } else { "" };

                eprintln!("    Col {}: {} type=0x{:04X}{}", num, cname, types[i] as u16, dup);
            }

            // Report duplicates
            eprintln!("\n=== Duplicates ===");
            for (key, count) in &seen {
                if *count > 1 {
                    eprintln!("  {} appears {} times", key, count);
                }
            }
            if seen.values().all(|&c| c == 1) {
                eprintln!("  No duplicates found!");
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
