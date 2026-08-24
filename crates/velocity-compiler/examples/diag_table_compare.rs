//! Compare Property table data between compiler and test MSIs
use std::io::{Cursor, Read};

fn main() {
    let comp_data = std::fs::read("test_compiler.msi").unwrap();
    let test_data = std::fs::read("replicate_compiler.msi").unwrap();

    println!("=== Compare table data ===\n");

    // Open both with msi crate and compare Property table rows
    compare_table(&comp_data, &test_data, "Property");
    compare_table(&comp_data, &test_data, "Directory");
    compare_table(&comp_data, &test_data, "Component");
    compare_table(&comp_data, &test_data, "File");
    compare_table(&comp_data, &test_data, "Media");
    compare_table(&comp_data, &test_data, "Feature");
    compare_table(&comp_data, &test_data, "FeatureComponents");
    compare_table(&comp_data, &test_data, "Registry");
    compare_table(&comp_data, &test_data, "CustomAction");
    compare_table(&comp_data, &test_data, "InstallExecuteSequence");
    compare_table(&comp_data, &test_data, "InstallUISequence");
    compare_table(&comp_data, &test_data, "Shortcut");
    compare_table(&comp_data, &test_data, "Environment");

    // Compare SummaryInfo bytes directly
    println!("\n=== Compare SummaryInfo bytes ===");
    compare_summary_info(&comp_data, &test_data);

    // Compare string pool bytes
    println!("\n=== Compare string pool ===");
    compare_string_pool(&comp_data, &test_data);
}

fn compare_table(comp: &[u8], test: &[u8], table_name: &str) {
    let comp_pkg = msi::Package::open(Cursor::new(comp.to_vec())).unwrap();
    let test_pkg = msi::Package::open(Cursor::new(test.to_vec())).unwrap();

    let comp_table = comp_pkg.tables().find(|t| t.name() == table_name);
    let test_table = test_pkg.tables().find(|t| t.name() == table_name);

    match (comp_table, test_table) {
        (Some(ct), Some(tt)) => {
            let comp_rows: Vec<_> = ct.rows().collect();
            let test_rows: Vec<_> = tt.rows().collect();
            
            if comp_rows.len() != test_rows.len() {
                println!("{}: DIFFERENT row count! Compiler={}, Test={}", 
                    table_name, comp_rows.len(), test_rows.len());
                return;
            }

            let mut diffs = 0;
            for (i, (cr, tr)) in comp_rows.iter().zip(test_rows.iter()).enumerate() {
                let comp_vals: Vec<String> = cr.values().map(|v| format!("{:?}", v)).collect();
                let test_vals: Vec<String> = tr.values().map(|v| format!("{:?}", v)).collect();
                if comp_vals != test_vals {
                    if diffs < 3 {
                        println!("{} row {} DIFFERS:", table_name, i);
                        println!("  Compiler: {:?}", comp_vals);
                        println!("  Test:     {:?}", test_vals);
                    }
                    diffs += 1;
                }
            }
            if diffs > 3 {
                println!("  ... and {} more different rows", diffs - 3);
            }
            if diffs == 0 {
                println!("{}: {} rows - IDENTICAL", table_name, comp_rows.len());
            } else {
                println!("{}: {} rows, {} DIFFERENCES", table_name, comp_rows.len(), diffs);
            }
        }
        (None, None) => println!("{}: not in either MSI", table_name),
        (Some(_), None) => println!("{}: only in Compiler", table_name),
        (None, Some(_)) => println!("{}: only in Test", table_name),
    }
}

fn compare_summary_info(comp: &[u8], test: &[u8]) {
    // Find SummaryInfo stream in both
    let comp_streams = list_streams(comp);
    let test_streams = list_streams(test);

    let comp_summary = comp_streams.iter().find(|(n, _)| n.contains("SummaryInformation"));
    let test_summary = test_streams.iter().find(|(n, _)| n.contains("SummaryInformation"));

    match (comp_summary, test_summary) {
        (Some((_, comp_data)), Some((_, test_data))) => {
            println!("Compiler SummaryInfo: {} bytes", comp_data.len());
            println!("Test SummaryInfo:     {} bytes", test_data.len());
            if comp_data == test_data {
                println!("IDENTICAL!");
            } else {
                println!("DIFFERENT!");
                // Show first difference
                for (i, (a, b)) in comp_data.iter().zip(test_data.iter()).enumerate() {
                    if a != b {
                        println!("  First diff at byte {}: compiler=0x{:02x}, test=0x{:02x}", i, a, b);
                        // Show context
                        let start = i.saturating_sub(4);
                        let end = (i + 16).min(comp_data.len()).min(test_data.len());
                        println!("  Compiler[{}..{}]: {:?}", start, end, &comp_data[start..end]);
                        println!("  Test[{}..{}]:     {:?}", start, end, &test_data[start..end]);
                        break;
                    }
                }
                if comp_data.len() != test_data.len() {
                    println!("  Size difference: compiler={} test={}", comp_data.len(), test_data.len());
                }
            }
        }
        _ => println!("SummaryInfo not found in one or both MSIs"),
    }
}

fn compare_string_pool(comp: &[u8], test: &[u8]) {
    let comp_streams = list_streams(comp);
    let test_streams = list_streams(test);

    // String pool streams have specific encoded names
    // _StringPool and _StringData
    // Find streams that are likely string pools (large, non-table)
    let comp_strpool = comp_streams.iter()
        .filter(|(n, _)| !n.contains("SummaryInformation"))
        .collect::<Vec<_>>();
    let test_strpool = test_streams.iter()
        .filter(|(n, _)| !n.contains("SummaryInformation"))
        .collect::<Vec<_>>();

    println!("Compiler has {} non-summary streams", comp_strpool.len());
    println!("Test has {} non-summary streams", test_strpool.len());
    
    // Compare stream sizes to find differences
    let mut size_diffs = 0;
    for (i, ((cn, cd), (tn, td))) in comp_strpool.iter().zip(test_strpool.iter()).enumerate() {
        if cd.len() != td.len() {
            println!("  Stream {}: size differs - compiler={} test={} (diff={})", 
                i, cd.len(), td.len(), cd.len() as i64 - td.len() as i64);
            size_diffs += 1;
        }
    }
    if size_diffs == 0 {
        println!("  All stream sizes identical!");
    }
}

fn list_streams(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let cursor = Cursor::new(data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();
    let mut streams = Vec::new();
    
    // Collect paths first
    let paths: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string(), e.len() as usize))
        .collect();
    
    for (path, name, _size) in paths {
        let mut stream = comp.open_stream(&path).unwrap();
        let mut data = Vec::new();
        stream.read_to_end(&mut data).unwrap();
        streams.push((name, data));
    }
    
    streams
}
