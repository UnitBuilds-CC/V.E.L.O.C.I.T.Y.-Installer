/// Inspect compiler MSI: decode Property table data and compare with progressive test
use std::io::{Cursor, Read};

fn main() {
    let comp_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\test_compiler.msi";
    let prog_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\prog_1_Property.msi";

    eprintln!("=== Compiler MSI ===");
    inspect_msi(comp_path);

    if std::path::Path::new(prog_path).exists() {
        eprintln!("\n=== Progressive Test MSI ===");
        inspect_msi(prog_path);
    }
}

fn inspect_msi(path: &str) {
    let data = std::fs::read(path).unwrap();
    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();

    // List all streams
    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();
    eprintln!("Streams: {}", streams.len());
    for (path, name) in &streams {
        let mut s = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        eprintln!("  {} ({} bytes)", name, buf.len());
    }

    // Decode string pool
    let pool = decode_string_pool(&mut comp, &streams);
    eprintln!("\nString pool: {} strings", pool.len());
    for (i, s) in pool.iter().enumerate().take(40) {
        eprintln!("  [{}] = '{}'", i, s);
    }
    if pool.len() > 40 {
        eprintln!("  ... ({} more)", pool.len() - 40);
    }

    // Decode _Tables
    eprintln!("\n_Tables:");
    let tables_names = decode_tables_stream(&mut comp, &streams, &pool);
    for name in &tables_names {
        eprintln!("  {}", name);
    }

    // Decode Property table
    eprintln!("\nProperty table:");
    decode_property_table(&mut comp, &streams, &pool);

    // Try opening with msi crate
    eprintln!("\nmsi crate:");
    match msi::Package::open(Cursor::new(&std::fs::read(path).unwrap())) {
        Ok(pkg) => {
            for table in pkg.tables() {
                eprintln!("  Table: {}", table.name());
            }
        }
        Err(e) => eprintln!("  FAILED: {}", e),
    }
}

fn read_stream_data(comp: &mut cfb::CompoundFile<Cursor<&Vec<u8>>>, path: &std::path::Path) -> Vec<u8> {
    let mut s = comp.open_stream(path).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    buf
}

fn decode_string_pool(comp: &mut cfb::CompoundFile<Cursor<&Vec<u8>>>, streams: &[(std::path::PathBuf, String)]) -> Vec<String> {
    let pool_target = velocity_msi::encode_stream_name("_StringPool", true);
    let data_target = velocity_msi::encode_stream_name("_StringData", true);

    let mut pool_data: Option<Vec<u8>> = None;
    let mut string_data: Option<Vec<u8>> = None;
    for (path, name) in streams {
        if name == &pool_target {
            pool_data = Some(read_stream_data(comp, path));
        }
        if name == &data_target {
            string_data = Some(read_stream_data(comp, path));
        }
    }

    let pool_data = pool_data.unwrap();
    let string_data = string_data.unwrap();

    let header = u32::from_le_bytes([pool_data[0], pool_data[1], pool_data[2], pool_data[3]]);
    let codepage = header & 0xFFFF;
    let long_refs = (header & 0x80000000) != 0;
    eprintln!("  Codepage: {}, Long refs: {}", codepage, long_refs);

    let mut offset = 4;
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

fn decode_tables_stream(comp: &mut cfb::CompoundFile<Cursor<&Vec<u8>>>, streams: &[(std::path::PathBuf, String)], pool: &[String]) -> Vec<String> {
    let target = velocity_msi::encode_stream_name("_Tables", true);
    for (path, name) in streams {
        if name == &target {
            let buf = read_stream_data(comp, path);
            let row_count = buf.len() / 2;
            let mut names = Vec::new();
            for i in 0..row_count {
                let id = u16::from_le_bytes([buf[i*2], buf[i*2+1]]) as usize;
                let name = if id < pool.len() { pool[id].clone() } else { format!("?{}", id) };
                names.push(name);
            }
            return names;
        }
    }
    vec![]
}

fn decode_property_table(comp: &mut cfb::CompoundFile<Cursor<&Vec<u8>>>, streams: &[(std::path::PathBuf, String)], pool: &[String]) {
    let target = velocity_msi::encode_stream_name("Property", true);
    for (path, name) in streams {
        if name == &target {
            let buf = read_stream_data(comp, path);
            eprintln!("  Raw data: {} bytes, hex: {:?}", buf.len(), &buf[..std::cmp::min(64, buf.len())]);
            let row_count = buf.len() / 4;
            eprintln!("  Row count: {}", row_count);

            // Column 1: Property (string refs, 2 bytes each)
            let mut prop_ids = Vec::new();
            for i in 0..row_count {
                let id = u16::from_le_bytes([buf[i*2], buf[i*2+1]]) as usize;
                prop_ids.push(id);
            }
            // Column 2: Value (string refs, 2 bytes each)
            let val_offset = row_count * 2;
            let mut val_ids = Vec::new();
            for i in 0..row_count {
                let id = u16::from_le_bytes([buf[val_offset + i*2], buf[val_offset + i*2+1]]) as usize;
                val_ids.push(id);
            }

            for i in 0..row_count {
                let pname = if prop_ids[i] < pool.len() { &pool[prop_ids[i]] } else { "?" };
                let vname = if val_ids[i] < pool.len() { &pool[val_ids[i]] } else { "?" };
                eprintln!("  Row {}: Property[{}]='{}' Value[{}]='{}'", i, prop_ids[i], pname, val_ids[i], vname);
            }
            return;
        }
    }
    eprintln!("  Property stream not found!");
}
