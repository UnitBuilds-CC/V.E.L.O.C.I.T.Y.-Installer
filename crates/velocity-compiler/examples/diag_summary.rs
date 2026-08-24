/// Check SummaryInfo and _Tables stream from compiler MSI
use std::io::{Cursor, Read};

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");

    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).expect("open CFB");

    let stream_info: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    eprintln!("All streams:");
    for (path, name) in &stream_info {
        let mut s = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        eprintln!("  {} = {} bytes", name, buf.len());
    }

    // Read SummaryInfo
    let summary_enc = "\u{0005}SummaryInformation".to_string();
    for (path, name) in &stream_info {
        if *name == summary_enc {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            eprintln!("\n--- SummaryInfo ({} bytes) ---", buf.len());
            eprintln!("Hex (first 128 bytes):");
            for chunk in buf[..128.min(buf.len())].chunks(16) {
                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                eprintln!("  {}", hex.join(" "));
            }

            // Parse property set header
            // Byte 0-1: byte order (0xFF 0xFE = little endian)
            // Byte 2-3: format version
            // Byte 4-7: OS version
            // Byte 8-23: class ID
            // Byte 24-27: offset to first section
            // Byte 28-31: number of sections
            if buf.len() >= 32 {
                let os_ver = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                let section_offset = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
                let num_sections = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
                eprintln!("OS version: {}, section offset: {}, num sections: {}", os_ver, section_offset, num_sections);

                // Parse first section (SummaryInformation properties)
                let sec_off = section_offset as usize;
                if sec_off + 8 <= buf.len() {
                    let sec_size = u32::from_le_bytes([buf[sec_off], buf[sec_off+1], buf[sec_off+2], buf[sec_off+3]]);
                    let num_props = u32::from_le_bytes([buf[sec_off+4], buf[sec_off+5], buf[sec_off+6], buf[sec_off+7]]);
                    eprintln!("Section: size={}, num_properties={}", sec_size, num_props);

                    // Each property: u32 PID + u32 offset
                    for i in 0..num_props as usize {
                        let prop_off = sec_off + 8 + i * 8;
                        if prop_off + 8 > buf.len() { break; }
                        let pid = u32::from_le_bytes([buf[prop_off], buf[prop_off+1], buf[prop_off+2], buf[prop_off+3]]);
                        let poff = u32::from_le_bytes([buf[prop_off+4], buf[prop_off+5], buf[prop_off+6], buf[prop_off+7]]);
                        let abs_off = sec_off as u32 + poff;

                        // Read property type and value
                        if (abs_off as usize) + 8 <= buf.len() {
                            let ptype = u32::from_le_bytes([buf[abs_off as usize], buf[abs_off as usize + 1], buf[abs_off as usize + 2], buf[abs_off as usize + 3]]);
                            let val = u32::from_le_bytes([buf[abs_off as usize + 4], buf[abs_off as usize + 5], buf[abs_off as usize + 6], buf[abs_off as usize + 7]]);

                            match ptype {
                                2 => eprintln!("  PID {}: VT_I4 = {}", pid, val as i32),
                                3 => eprintln!("  PID {}: VT_I4 = {}", pid, val as i32),
                                19 => {
                                    // VT_FILETIME - 8 bytes
                                    let lo = val;
                                    let hi = u32::from_le_bytes([buf[abs_off as usize + 8], buf[abs_off as usize + 9], buf[abs_off as usize + 10], buf[abs_off as usize + 11]]);
                                    eprintln!("  PID {}: VT_FILETIME = 0x{:08x}{:08x}", pid, hi, lo);
                                },
                                30 => {
                                    // VT_LPSTR - length-prefixed string
                                    let str_len = val as usize;
                                    if (abs_off as usize + 8 + str_len) <= buf.len() {
                                        let s = String::from_utf8_lossy(&buf[abs_off as usize + 8..abs_off as usize + 8 + str_len]).trim_end_matches('\0').to_string();
                                        eprintln!("  PID {}: VT_LPSTR = {:?}", pid, s);
                                    } else {
                                        eprintln!("  PID {}: VT_LPSTR (truncated, len={})", pid, str_len);
                                    }
                                },
                                _ => eprintln!("  PID {}: type={} val=0x{:08x}", pid, ptype, val),
                            }
                        }
                    }
                }
            }
        }
    }

    // Read _Tables stream
    let tables_enc = velocity_msi::encode_stream_name("_Tables", true);
    for (path, name) in &stream_info {
        if *name == tables_enc {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            eprintln!("\n--- _Tables stream ({} bytes) ---", buf.len());
            eprintln!("Hex: {:02x?}", &buf);
        }
    }

    // Read _Columns stream (first 128 bytes)
    let cols_enc = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &stream_info {
        if *name == cols_enc {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            eprintln!("\n--- _Columns stream ({} bytes) ---", buf.len());
            eprintln!("Hex (first 128): {:02x?}", &buf[..128.min(buf.len())]);
        }
    }
}
