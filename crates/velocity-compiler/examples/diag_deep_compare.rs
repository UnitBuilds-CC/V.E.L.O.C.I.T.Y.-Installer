//! Deep comparison of SummaryInfo and _Columns between compiler and test MSIs
use std::io::{Cursor, Read};

fn main() {
    let comp_data = std::fs::read("test_compiler.msi").unwrap();
    let test_data = std::fs::read("replicate_compiler.msi").unwrap();

    let comp_streams = list_streams(&comp_data);
    let test_streams = list_streams(&test_data);

    // Stream 19 = SummaryInfo (both 420 bytes)
    println!("=== SummaryInfo comparison ===");
    let comp_summary = &comp_streams[19].1;
    let test_summary = &test_streams[19].1;
    println!("Compiler: {} bytes", comp_summary.len());
    println!("Test:     {} bytes", test_summary.len());
    
    let mut summary_diffs = 0;
    for (i, (a, b)) in comp_summary.iter().zip(test_summary.iter()).enumerate() {
        if a != b {
            if summary_diffs < 10 {
                println!("  Byte {}: comp=0x{:02x} ({}) test=0x{:02x} ({})", i, a, a, b, b);
            }
            summary_diffs += 1;
        }
    }
    if summary_diffs == 0 {
        println!("  IDENTICAL!");
    } else {
        println!("  Total different bytes: {}", summary_diffs);
    }

    // Stream 4 = likely _Columns (80 vs 16 bytes)
    println!("\n=== Stream 4 (_Columns?) comparison ===");
    let comp_s4 = &comp_streams[4].1;
    let test_s4 = &test_streams[4].1;
    println!("Compiler: {} bytes", comp_s4.len());
    println!("Test:     {} bytes", test_s4.len());
    println!("Compiler hex: {:02x?}", comp_s4);
    println!("Test hex:     {:02x?}", test_s4);

    // Stream 0 = likely _Tables (180 bytes each, but data differs)
    println!("\n=== Stream 0 (_Tables?) comparison ===");
    let comp_s0 = &comp_streams[0].1;
    let test_s0 = &test_streams[0].1;
    println!("Compiler: {} bytes", comp_s0.len());
    println!("Test:     {} bytes", test_s0.len());
    let mut s0_diffs = 0;
    for (i, (a, b)) in comp_s0.iter().zip(test_s0.iter()).enumerate() {
        if a != b {
            if s0_diffs < 10 {
                println!("  Byte {}: comp=0x{:02x} test=0x{:02x}", i, a, b);
            }
            s0_diffs += 1;
        }
    }
    println!("  Total different bytes: {}", s0_diffs);

    // Stream 7 = 40 vs 36 bytes
    println!("\n=== Stream 7 (40 vs 36 bytes) ===");
    let comp_s7 = &comp_streams[7].1;
    let test_s7 = &test_streams[7].1;
    println!("Compiler hex: {:02x?}", comp_s7);
    println!("Test hex:     {:02x?}", test_s7);

    // Check if the OLE header/first 512 bytes differ
    println!("\n=== OLE header comparison ===");
    let comp_header = &comp_data[..512.min(comp_data.len())];
    let test_header = &test_data[..512.min(test_data.len())];
    let mut header_diffs = 0;
    for (i, (a, b)) in comp_header.iter().zip(test_header.iter()).enumerate() {
        if a != b {
            if header_diffs < 10 {
                println!("  Byte {}: comp=0x{:02x} test=0x{:02x}", i, a, b);
            }
            header_diffs += 1;
        }
    }
    println!("  Total different header bytes: {}", header_diffs);
}

fn list_streams(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let cursor = Cursor::new(data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    let mut streams = Vec::new();
    
    let paths: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();
    
    for (path, name) in paths {
        let mut stream = comp.open_stream(&path).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        streams.push((name, buf));
    }
    
    streams
}
