/// Definitive diagnostic: decode string pool + Property table from compiler MSI
use std::io::{Cursor, Read};

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");

    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).expect("open CFB");

    // Collect all stream paths and names
    let stream_info: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    let pool_enc = velocity_msi::encode_stream_name("_StringPool", true);
    let data_enc = velocity_msi::encode_stream_name("_StringData", true);
    let prop_enc = velocity_msi::encode_stream_name("Property", true);

    let mut pool_data = None;
    let mut string_data = None;
    let mut property_data = None;

    for (path, name) in &stream_info {
        let mut stream = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();

        if *name == pool_enc {
            eprintln!("Found _StringPool: {} bytes", buf.len());
            pool_data = Some(buf);
        } else if *name == data_enc {
            eprintln!("Found _StringData: {} bytes", buf.len());
            string_data = Some(buf);
        } else if *name == prop_enc {
            eprintln!("Found Property: {} bytes", buf.len());
            property_data = Some(buf);
        }
    }

    let pool_data = pool_data.expect("_StringPool not found");
    let string_data = string_data.expect("_StringData not found");
    let property_data = property_data.expect("Property table not found");

    // Decode string pool header
    let header = u32::from_le_bytes([pool_data[0], pool_data[1], pool_data[2], pool_data[3]]);
    let codepage = header & 0xFFFF;
    let long_refs = (header >> 31) & 1 == 1;
    eprintln!("\nString pool: codepage={}, long_refs={}", codepage, long_refs);

    // Parse entries: each is u16 length + u16 refcount
    let mut offset = 4;
    let mut entries: Vec<(u16, u16)> = Vec::new();
    while offset + 4 <= pool_data.len() {
        let len = u16::from_le_bytes([pool_data[offset], pool_data[offset + 1]]);
        let rc = u16::from_le_bytes([pool_data[offset + 2], pool_data[offset + 3]]);
        entries.push((len, rc));
        offset += 4;
    }
    eprintln!("Pool entries: {}", entries.len());

    // Decode strings from data using entry lengths
    let mut strings: Vec<String> = Vec::new();
    let mut data_off = 0usize;
    strings.push(String::new()); // ID 0

    for (i, (len, _rc)) in entries.iter().enumerate() {
        let len = *len as usize;
        if data_off + len > string_data.len() {
            eprintln!("ERROR: String {} beyond data (off={}, len={}, data={})", i+1, data_off, len, string_data.len());
            break;
        }
        let s = String::from_utf8_lossy(&string_data[data_off..data_off+len]).to_string();
        strings.push(s);
        data_off += len;
    }
    eprintln!("Decoded {} strings, consumed {}/{} data bytes", strings.len(), data_off, string_data.len());

    // Print all strings
    eprintln!("\n--- String Pool ---");
    for (i, s) in strings.iter().enumerate() {
        eprintln!("  [{}] = {:?}", i, s);
    }

    // Decode Property table (column-major, 2 string cols, short refs = 2 bytes each)
    let row_count = property_data.len() / 4;
    eprintln!("\n--- Property Table ({} rows, {} bytes) ---", row_count, property_data.len());
    eprintln!("Hex: {:02x?}", &property_data);

    // Col 1: Property names
    let mut col1: Vec<u16> = Vec::new();
    for i in 0..row_count {
        let off = i * 2;
        col1.push(u16::from_le_bytes([property_data[off], property_data[off+1]]));
    }
    // Col 2: Property values
    let mut col2: Vec<u16> = Vec::new();
    for i in 0..row_count {
        let off = row_count * 2 + i * 2;
        col2.push(u16::from_le_bytes([property_data[off], property_data[off+1]]));
    }

    eprintln!("\nDecoded rows:");
    for i in 0..row_count {
        let nid = col1[i];
        let vid = col2[i];
        let name = if (nid as usize) < strings.len() { &strings[nid as usize] } else { "<BAD>" };
        let val = if (vid as usize) < strings.len() { &strings[vid as usize] } else { "<BAD>" };
        eprintln!("  Row {}: name_id={} ({:?}) -> value_id={} ({:?})", i, nid, name, vid, val);
    }

    // Verify string data length
    let expected: usize = entries.iter().map(|(l,_)| *l as usize).sum();
    eprintln!("\n--- Verification ---");
    eprintln!("Expected data len: {}, actual: {}, match: {}", expected, string_data.len(), expected == string_data.len());
}
