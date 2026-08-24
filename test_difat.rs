use velocity_msi::ole::{build_ole_file, OleStream};

fn main() {
    println!("Testing DIFAT support with 8MB file...");
    
    // Create an 8MB stream (requires DIFAT)
    let large_data = vec![0xAB; 8 * 1024 * 1024];
    let streams = vec![OleStream {
        name: "LargeFile".to_string(),
        data: large_data.clone(),
    }];
    
    let ole_data = build_ole_file(&streams);
    
    // Check header
    let num_fat = u32::from_le_bytes([ole_data[44], ole_data[45], ole_data[46], ole_data[47]]);
    let num_difat = u32::from_le_bytes([ole_data[68], ole_data[69], ole_data[70], ole_data[71]]);
    let first_difat = u32::from_le_bytes([ole_data[64], ole_data[65], ole_data[66], ole_data[67]]);
    
    println!("FAT sectors: {}", num_fat);
    println!("DIFAT sectors: {}", num_difat);
    println!("First DIFAT sector: {}", first_difat);
    
    if num_fat > 109 && num_difat > 0 && first_difat != 0xFFFFFFFF {
        println!("✓ DIFAT support working correctly!");
        println!("✓ File size: {} bytes ({:.2} MB)", ole_data.len(), ole_data.len() as f64 / 1024.0 / 1024.0);
    } else {
        println!("✗ DIFAT support NOT working!");
    }
}
