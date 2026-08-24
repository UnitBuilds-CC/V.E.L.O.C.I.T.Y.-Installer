/// Check OLE CLSID and root entry of compiler MSI
use std::io::{Cursor, Read};

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");

    // Check OLE header
    // Signature: 0xD0CF11E0A1B11AE1 (8 bytes)
    let sig = &data[0..8];
    eprintln!("OLE signature: {:02x?}", sig);
    assert_eq!(sig, &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);

    // CLSID: 16 bytes at offset 8 (should be zeros for our file)
    let clsid = &data[8..24];
    eprintln!("CLSID (header): {:02x?}", clsid);

    // Version: u16 at offset 24 (0x003E for V3)
    let version = u16::from_le_bytes([data[24], data[25]]);
    eprintln!("OLE version: 0x{:04X} ({})", version, if version == 0x003E { "V3" } else { "V4" });

    // DLL version: u16 at offset 26
    let dll_ver = u16::from_le_bytes([data[26], data[27]]);
    eprintln!("DLL version: 0x{:04X}", dll_ver);

    // Byte order: u16 at offset 28 (0xFFFE = little endian)
    let byte_order = u16::from_le_bytes([data[28], data[29]]);
    eprintln!("Byte order: 0x{:04X}", byte_order);

    // Sector size: u16 at offset 30 (0x0009 = 512 bytes for V3)
    let sector_shift = u16::from_le_bytes([data[30], data[31]]);
    let sector_size = 1u32 << sector_shift;
    eprintln!("Sector shift: {} (sector size: {} bytes)", sector_shift, sector_size);

    // Mini sector size
    let mini_shift = u16::from_le_bytes([data[32], data[33]]);
    eprintln!("Mini sector shift: {}", mini_shift);

    // Root entry CLSID is stored in the root directory entry
    // The first directory entry is at the first directory sector
    let fat_sectors = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);
    eprintln!("FAT sectors: {}", fat_sectors);

    let first_dir_sector = u32::from_le_bytes([data[48], data[49], data[50], data[51]]);
    eprintln!("First directory sector: {}", first_dir_sector);

    // Read root directory entry to get CLSID
    let dir_offset = (first_dir_sector as usize + 1) * sector_size as usize;
    if dir_offset + 128 <= data.len() {
        let root_clsid = &data[dir_offset + 80..dir_offset + 96];
        eprintln!("\nRoot entry CLSID: {:02x?}", root_clsid);

        // MSI CLSID: {000C1084-0000-0000-C000-000000000046}
        let msi_clsid: [u8; 16] = [
            0x00, 0x0C, 0x10, 0x84, 0x00, 0x00, 0x00, 0x00,
            0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ];
        if root_clsid == msi_clsid {
            eprintln!("CLSID matches MSI package CLSID ✓");
        } else if root_clsid.iter().all(|&b| b == 0) {
            eprintln!("CLSID is all zeros (NOT SET!) ✗");
            eprintln!("Expected MSI CLSID: {:02x?}", &msi_clsid);
        } else {
            eprintln!("CLSID is set but doesn't match MSI CLSID ✗");
            eprintln!("Expected MSI CLSID: {:02x?}", &msi_clsid);
        }

        // Also check root entry name
        let name_len = u16::from_le_bytes([data[dir_offset + 64], data[dir_offset + 65]]) as usize;
        let name_bytes = &data[dir_offset..dir_offset + name_len.min(64)];
        let name: String = name_bytes.chunks(2)
            .map(|c| if c.len() == 2 { char::from_u32(u16::from_le_bytes([c[0], c[1]]) as u32).unwrap_or('?') } else { '?' })
            .collect();
        eprintln!("Root entry name: {:?} ({} bytes)", name.trim_end_matches('\0'), name_len);
    }
}
