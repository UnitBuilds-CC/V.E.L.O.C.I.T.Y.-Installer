//! Compare raw stream bytes between compiler and test MSIs
use std::io::{Cursor, Read};

fn main() {
    let comp_data = std::fs::read("test_compiler.msi").unwrap();
    let test_data = std::fs::read("replicate_compiler.msi").unwrap();

    println!("=== Raw stream comparison ===\n");

    let comp_streams = list_streams(&comp_data);
    let test_streams = list_streams(&test_data);

    println!("Compiler: {} streams", comp_streams.len());
    println!("Test:     {} streams", test_streams.len());

    // Compare stream by stream (they should be in the same order)
    let max_streams = comp_streams.len().max(test_streams.len());
    for i in 0..max_streams {
        match (comp_streams.get(i), test_streams.get(i)) {
            (Some((cn, cd)), Some((tn, td))) => {
                let same_name = cn == tn;
                let same_data = cd == td;
                let size_diff = cd.len() as i64 - td.len() as i64;
                
                if !same_name || !same_data {
                    println!("\nStream {} DIFFERS:", i);
                    println!("  Compiler: name={}, {} bytes", cn, cd.len());
                    println!("  Test:     name={}, {} bytes", tn, td.len());
                    if !same_name {
                        println!("  NAMES DIFFER!");
                    }
                    if size_diff != 0 {
                        println!("  Size diff: {} bytes", size_diff);
                        // Show first differing byte
                        for (j, (a, b)) in cd.iter().zip(td.iter()).enumerate() {
                            if a != b {
                                println!("  First byte diff at offset {}: comp=0x{:02x} test=0x{:02x}", j, a, b);
                                // Show surrounding context
                                let start = j.saturating_sub(8);
                                let end_c = (j + 32).min(cd.len());
                                let end_t = (j + 32).min(td.len());
                                println!("  Comp[{}..{}]: {:?}", start, end_c, &cd[start..end_c]);
                                println!("  Test[{}..{}]: {:?}", start, end_t, &td[start..end_t]);
                                break;
                            }
                        }
                    }
                } else {
                    println!("Stream {}: {} ({} bytes) - identical", i, cn, cd.len());
                }
            }
            (Some((n, d)), None) => println!("Stream {}: {} ({} bytes) - ONLY IN COMPILER", i, n, d.len()),
            (None, Some((n, d))) => println!("Stream {}: {} ({} bytes) - ONLY IN TEST", i, n, d.len()),
            (None, None) => break,
        }
    }
}

fn list_streams(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let cursor = Cursor::new(data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    let mut streams = Vec::new();
    
    // Collect paths first to avoid borrow issues
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
