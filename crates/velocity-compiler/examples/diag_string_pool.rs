//! Decode string pool from both MSIs and check Property table references
use std::io::{Cursor, Read};

fn main() {
    let comp_data = std::fs::read("test_minimal.msi").unwrap();
    let test_data = std::fs::read("replicate_compiler.msi").unwrap();

    println!("=== String pool analysis ===\n");
    
    analyze_msi(&comp_data, "Compiler (minimal)");
    println!("\n---\n");
    analyze_msi(&test_data, "Test");
}

fn analyze_msi(data: &[u8], label: &str) {
    println!("=== {} ===", label);
    let cursor = Cursor::new(data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    
    // Collect all streams
    let paths: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string(), e.len() as usize))
        .collect();
    
    // Find string pool streams (they're the largest table streams)
    // _StringPool and _StringData have specific encoded names
    let pool_name = velocity_msi::encode_stream_name("_StringPool", true);
    let data_name = velocity_msi::encode_stream_name("_StringData", true);
    
    println!("Looking for pool name: {:?}", pool_name);
    println!("Looking for data name: {:?}", data_name);
    
    let mut pool_data: Option<Vec<u8>> = None;
    let mut string_data: Option<Vec<u8>> = None;
    
    for (path, name, _size) in &paths {
        let mut stream = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        
        if name == &pool_name {
            pool_data = Some(buf);
        } else if name == &data_name {
            string_data = Some(buf);
        }
    }
    
    match (pool_data, string_data) {
        (Some(pool), Some(sdata)) => {
            println!("\nString pool: {} bytes", pool.len());
            println!("String data: {} bytes", sdata.len());
            
            // Decode pool header
            if pool.len() >= 4 {
                let header = u32::from_le_bytes([pool[0], pool[1], pool[2], pool[3]]);
                let codepage = header & 0xFFFF;
                let long_refs = (header >> 31) != 0;
                println!("Header: 0x{:08X}", header);
                println!("Codepage: {}", codepage);
                println!("Long refs: {}", long_refs);
                
                let entry_size = if long_refs { 5 } else { 4 };
                let num_strings = (pool.len() - 4) / entry_size;
                println!("Number of strings: {}", num_strings);
                
                // Decode string entries
                let mut strings: Vec<String> = Vec::new();
                let mut offset = 0usize;
                
                for i in 0..num_strings {
                    let entry_start = 4 + i * entry_size;
                    if entry_start + entry_size > pool.len() {
                        break;
                    }
                    let length = u16::from_le_bytes([pool[entry_start], pool[entry_start + 1]]) as usize;
                    let _refcount = u16::from_le_bytes([pool[entry_start + 2], pool[entry_start + 3]]);
                    
                    let s = if length > 0 && offset + length <= sdata.len() {
                        let bytes = &sdata[offset..offset + length];
                        // Decode as Windows-1252
                        let decoded: String = bytes.iter().map(|&b| {
                            // Simple ASCII/Windows-1252 decode
                            if b < 128 { b as char } else { '?' }
                        }).collect();
                        offset += length;
                        decoded
                    } else {
                        String::new()
                    };
                    
                    strings.push(s);
                }
                
                println!("\nFirst 30 strings:");
                for (i, s) in strings.iter().take(30).enumerate() {
                    let display = if s.is_empty() { "<EMPTY>" } else { s };
                    println!("  [{}] = {:?}", i, display);
                }
                
                // Now find and decode the Property table
                let prop_name = velocity_msi::encode_stream_name("Property", true);
                for (path, name, _size) in &paths {
                    if name == &prop_name {
                        let mut stream = comp.open_stream(path).unwrap();
                        let mut buf = Vec::new();
                        stream.read_to_end(&mut buf).unwrap();
                        
                        println!("\nProperty table: {} bytes", buf.len());
                        
                        // Property table has 2 columns: Property (string PK), Value (string nullable)
                        // Column-major: all Property names first, then all Values
                        // With short string refs: 2 bytes per entry
                        let num_rows = buf.len() / 4; // 2 bytes per name + 2 bytes per value
                        println!("Estimated rows: {}", num_rows);
                        
                        // Read property names (first half)
                        let names_size = num_rows * 2;
                        for i in 0..num_rows {
                            let name_offset = i * 2;
                            let value_offset = names_size + i * 2;
                            if value_offset + 2 > buf.len() { break; }
                            
                            let name_id = u16::from_le_bytes([buf[name_offset], buf[name_offset + 1]]) as usize;
                            let value_id = u16::from_le_bytes([buf[value_offset], buf[value_offset + 1]]) as usize;
                            
                            let name_str = if name_id < strings.len() { &strings[name_id] } else { "<INVALID>" };
                            let value_str = if value_id < strings.len() { &strings[value_id] } else { "<INVALID>" };
                            
                            println!("  {} = {} (name_id={}, value_id={})", name_str, value_str, name_id, value_id);
                        }
                        break;
                    }
                }
            }
        }
        _ => {
            println!("String pool streams not found!");
        }
    }
}
