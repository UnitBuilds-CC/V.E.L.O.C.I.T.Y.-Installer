//! Payload management — creating and reading embedded payloads.
//!
//! The payload format is:
//! ```text
//! [Original EXE bytes]
//! [PAYLOAD_MARKER: 16 bytes "VELOCITY_PKG_V1\0"]
//! [MANIFEST_LEN: 8 bytes, u64 little-endian]
//! [MANIFEST_JSON: manifest_len bytes]
//! [PAYLOAD_LEN: 8 bytes, u64 little-endian]
//! [PAYLOAD_DATA: payload_len bytes, zstd-compressed tar]
//! ```

use crate::error::{CoreError, Result};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Magic marker appended before the payload data.
const PAYLOAD_MARKER: &[u8; 16] = b"VELOCITY_PKG_V1\0";

/// Create a payload by appending manifest + compressed files to a base executable.
pub fn create_payload(
    base_exe: &[u8],
    manifest_json: &[u8],
    compressed_data: &[u8],
    output: &Path,
) -> Result<()> {
    let mut file = std::fs::File::create(output)?;

    // Write the base executable (runtime stub)
    file.write_all(base_exe)?;

    // Write the marker
    file.write_all(PAYLOAD_MARKER)?;

    // Write manifest length + manifest data
    let manifest_len = manifest_json.len() as u64;
    file.write_all(&manifest_len.to_le_bytes())?;
    file.write_all(manifest_json)?;

    // Write payload length + payload data
    let payload_len = compressed_data.len() as u64;
    file.write_all(&payload_len.to_le_bytes())?;
    file.write_all(compressed_data)?;

    file.flush()?;
    Ok(())
}

/// Read the embedded manifest and payload from a Velocity installer executable.
pub fn read_payload(exe_path: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut file = std::fs::File::open(exe_path)?;
    let file_len = file.metadata()?.len();

    // Search backwards for the marker
    let marker_pos = find_marker(&mut file, file_len)?;

    // Read past the marker
    file.seek(SeekFrom::Start(marker_pos + PAYLOAD_MARKER.len() as u64))?;

    // Read manifest length + manifest
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let manifest_len = u64::from_le_bytes(len_buf) as usize;

    let mut manifest_data = vec![0u8; manifest_len];
    file.read_exact(&mut manifest_data)?;

    // Read payload length + payload
    file.read_exact(&mut len_buf)?;
    let payload_len = u64::from_le_bytes(len_buf) as usize;

    let mut payload_data = vec![0u8; payload_len];
    file.read_exact(&mut payload_data)?;

    Ok((manifest_data, payload_data))
}

/// Find the payload marker by scanning backwards from the end of the file.
fn find_marker(file: &mut std::fs::File, file_len: u64) -> Result<u64> {
    // The marker should be near the end. Search the last 256KB at most.
    let search_start = if file_len > 262144 {
        file_len - 262144
    } else {
        0
    };

    let mut buf = vec![0u8; (file_len - search_start) as usize];
    file.seek(SeekFrom::Start(search_start))?;
    file.read_exact(&mut buf)?;

    // Search for the marker pattern
    for i in (0..buf.len().saturating_sub(PAYLOAD_MARKER.len())).rev() {
        if &buf[i..i + PAYLOAD_MARKER.len()] == PAYLOAD_MARKER.as_slice() {
            return Ok(search_start + i as u64);
        }
    }

    Err(CoreError::InvalidPayload(
        "No Velocity payload marker found".to_string(),
    ))
}

/// Get the size of the base executable (everything before the payload marker).
pub fn get_base_exe_size(exe_path: &Path) -> Result<u64> {
    let mut file = std::fs::File::open(exe_path)?;
    let file_len = file.metadata()?.len();
    let marker_pos = find_marker(&mut file, file_len)?;
    Ok(marker_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_create_and_read_payload() {
        let temp_dir = std::env::temp_dir();
        let base_exe = b"FAKE_EXE_CONTENT";
        let manifest = b"{\"app\":\"test\"}";
        let payload = b"compressed_data_here";
        let output = temp_dir.join("velocity_test_payload.exe");

        create_payload(base_exe, manifest, payload, &output).unwrap();

        let (read_manifest, read_payload) = read_payload(&output).unwrap();
        assert_eq!(read_manifest, manifest);
        assert_eq!(read_payload, payload);

        let base_size = get_base_exe_size(&output).unwrap();
        assert_eq!(base_size, base_exe.len() as u64);

        std::fs::remove_file(&output).ok();
    }
}
