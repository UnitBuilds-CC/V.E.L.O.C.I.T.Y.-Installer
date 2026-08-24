/// Parse SummaryInfo from MSI using cfb
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

    let summary_name = "\u{0005}SummaryInformation";
    for (path, name) in &stream_info {
        if name == summary_name {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            parse_summary(&buf);
            return;
        }
    }
    eprintln!("SummaryInfo stream not found!");
}

fn read_u16(d: &[u8], off: usize) -> u16 { u16::from_le_bytes([d[off], d[off+1]]) }
fn read_u32(d: &[u8], off: usize) -> u32 { u32::from_le_bytes([d[off], d[off+1], d[off+2], d[off+3]]) }

fn parse_summary(buf: &[u8]) {
    eprintln!("SummaryInfo: {} bytes", buf.len());

    // Header
    let bom = read_u16(buf, 0);
    let fmt_ver = read_u16(buf, 2);
    let os_major = buf[4];
    let os_minor = buf[5];
    let platform = read_u16(buf, 6);
    let section_count = read_u32(buf, 24);
    let section_offset = read_u32(buf, 44);

    eprintln!("BOM: 0x{:04x}, FmtVer: {}, OS: {}.{} platform={}", bom, fmt_ver, os_major, os_minor, platform);
    eprintln!("Sections: {}, first at offset {}", section_count, section_offset);

    let so = section_offset as usize;
    let sec_size = read_u32(buf, so);
    let num_props = read_u32(buf, so + 4);
    eprintln!("Section: size={}, properties={}", sec_size, num_props);

    // Read property index
    for i in 0..num_props as usize {
        let entry_off = so + 8 + i * 8;
        let pid = read_u32(buf, entry_off);
        let prop_off = read_u32(buf, entry_off + 4);
        let abs_off = so as u32 + prop_off;

        let ptype = read_u32(buf, abs_off as usize);
        let pid_name = match pid {
            1 => "Codepage",
            2 => "Title",
            3 => "Subject",
            4 => "Author",
            5 => "Keywords",
            6 => "Comments",
            7 => "Template",
            8 => "LastAuthor",
            9 => "RevNumber",
            12 => "CreateTime",
            13 => "LastSaveTime",
            14 => "Security",
            15 => "WordCount",
            18 => "CreatingApp",
            19 => "Category",
            _ => "Unknown",
        };

        match ptype {
            2 | 3 => { // VT_I2, VT_I4
                let val = read_u32(buf, abs_off as usize + 4) as i32;
                eprintln!("  PID {} ({}): VT_I4 = {}", pid, pid_name, val);
            }
            30 => { // VT_LPSTR
                let len = read_u32(buf, abs_off as usize + 4) as usize;
                let str_bytes = &buf[abs_off as usize + 8..abs_off as usize + 8 + len];
                let s = String::from_utf8_lossy(str_bytes).trim_end_matches('\0').to_string();
                eprintln!("  PID {} ({}): VT_LPSTR len={} = {:?}", pid, pid_name, len, s);
            }
            64 => { // VT_FILETIME
                let lo = read_u32(buf, abs_off as usize + 4);
                let hi = read_u32(buf, abs_off as usize + 8);
                eprintln!("  PID {} ({}): VT_FILETIME = 0x{:08x}{:08x}", pid, pid_name, hi, lo);
            }
            _ => {
                let val = read_u32(buf, abs_off as usize + 4);
                eprintln!("  PID {} ({}): type={} val=0x{:08x}", pid, pid_name, ptype, val);
            }
        }
    }
}
