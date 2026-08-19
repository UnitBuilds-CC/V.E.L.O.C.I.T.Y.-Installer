use std::fs;
use std::io::Read;

fn main() {
    let path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\installer.msi";
    let data = fs::read(path).expect("Failed to read MSI");
    
    let mut cursor = std::io::Cursor::new(&data);
    let mut compound = cfb::CompoundFile::open(&mut cursor).expect("Failed to open CFB");
    
    // Read string pool
    let pool_name = "\u{4840}\u{3f3f}\u{4577}\u{446c}\u{3e6a}\u{44b2}\u{482f}";
    let data_name = "\u{4840}\u{3f3f}\u{4577}\u{446c}\u{3b6a}\u{45e4}\u{4824}";
    
    let mut pool_stream = compound.open_stream(std::path::Path::new(pool_name)).unwrap();
    let mut pool_bytes = Vec::new();
    pool_stream.read_to_end(&mut pool_bytes).unwrap();
    
    let mut data_stream = compound.open_stream(std::path::Path::new(data_name)).unwrap();
    let mut data_bytes = Vec::new();
    data_stream.read_to_end(&mut data_bytes).unwrap();
    
    // Parse string pool
    let pool_header = u32::from_le_bytes([pool_bytes[0], pool_bytes[1], pool_bytes[2], pool_bytes[3]]);
    let long_refs = (pool_header >> 31) & 1 == 1;
    let codepage = pool_header & 0xFFFF;
    println!("String pool: codepage={}, long_refs={}", codepage, long_refs);
    
    let str_ref_size = if long_refs { 3 } else { 2 };
    
    let entry_size = 4;
    let num_strings = (pool_bytes.len() - 4) / entry_size;
    
    let mut strings: Vec<String> = Vec::new();
    strings.push(String::new()); // ID 0
    let mut offset = 4;
    let mut str_offset = 0;
    for _ in 0..num_strings {
        let len = u16::from_le_bytes([pool_bytes[offset], pool_bytes[offset + 1]]) as usize;
        let _refcount = u16::from_le_bytes([pool_bytes[offset + 2], pool_bytes[offset + 3]]);
        offset += entry_size;
        let s = if str_offset + len <= data_bytes.len() {
            String::from_utf8_lossy(&data_bytes[str_offset..str_offset + len]).to_string()
        } else {
            format!("<invalid offset {}>", str_offset)
        };
        str_offset += len;
        strings.push(s);
    }
    println!("String pool: {} strings", strings.len());
    
    // Read _Validation stream
    let val_name = "\u{4840}\u{3fff}\u{43e4}\u{41ec}\u{45e4}\u{44ac}\u{4831}";
    let mut stream = compound.open_stream(std::path::Path::new(val_name)).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    
    // _Validation columns with SHORT refs (2 bytes):
    // 0: Table (string64, PK) - 2 bytes
    // 1: Column (string64, PK) - 2 bytes
    // 2: Nullable (string4) - 2 bytes
    // 3: MinValue (int32, nullable) - 4 bytes
    // 4: MaxValue (int32, nullable) - 4 bytes
    // 5: KeyTable (string255, nullable) - 2 bytes
    // 6: KeyColumn (int16, nullable) - 2 bytes
    // 7: Category (string32, nullable) - 2 bytes
    // 8: Set (string255, nullable) - 2 bytes
    // 9: Description (string255, nullable) - 2 bytes
    // Total: 24 bytes per row
    
    let col_widths: Vec<usize> = vec![str_ref_size, str_ref_size, str_ref_size, 4, 4, str_ref_size, 2, str_ref_size, str_ref_size, str_ref_size];
    let col_names = ["Table", "Column", "Nullable", "MinValue", "MaxValue", "KeyTable", "KeyColumn", "Category", "Set", "Description"];
    let col_is_string = [true, true, true, false, false, true, false, true, true, true];
    let col_is_int16 = [false, false, false, false, false, false, true, false, false, false];
    
    let row_size: usize = col_widths.iter().sum();
    let num_rows = bytes.len() / row_size;
    println!("\n_Validation: {} bytes, {} rows, row_size={}, remainder={}", bytes.len(), num_rows, row_size, bytes.len() % row_size);
    
    // Parse column-major data
    let mut data_offset = 0;
    for (col_idx, (width, name)) in col_widths.iter().zip(col_names.iter()).enumerate() {
        println!("\n--- Column {}: {} (width={}) ---", col_idx, name, width);
        for row in 0..num_rows {
            let start = data_offset + row * width;
            let end = start + width;
            if end > bytes.len() { break; }
            let val_bytes = &bytes[start..end];
            
            if col_is_string[col_idx] {
                let id = if *width == 3 {
                    let low = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                    let high = val_bytes[2] as u32;
                    (high << 16) | (low as u32)
                } else {
                    u16::from_le_bytes([val_bytes[0], val_bytes[1]]) as u32
                };
                
                let s = if id == 0 {
                    "NULL".to_string()
                } else if (id as usize) < strings.len() {
                    format!("{:?}", strings[id as usize])
                } else {
                    format!("<INVALID id={}>", id)
                };
                
                if row < 10 || row >= num_rows - 3 {
                    println!("  [{}] {}", row, s);
                }
            } else if col_is_int16[col_idx] {
                let raw = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                let display = if raw == 0 { "NULL".to_string() } else {
                    let val = (raw as i16) ^ (-0x8000i32 as i16);
                    format!("{}", val)
                };
                if row < 10 { println!("  [{}] {}", row, display); }
            } else {
                let raw = u32::from_le_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]]);
                let display = if raw == 0 { "NULL".to_string() } else {
                    let val = (raw as i32) ^ (-0x80000000i32);
                    format!("{}", val)
                };
                if row < 10 { println!("  [{}] {}", row, display); }
            }
        }
        data_offset += num_rows * width;
    }
    
    println!("\nTotal bytes consumed: {}", data_offset);
    println!("Stream size: {}", bytes.len());
    println!("Match: {}", data_offset == bytes.len());
}
