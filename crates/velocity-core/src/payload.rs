//! Payload management — creating and reading embedded payloads.
//!
//! The payload format uses a fixed trailer for O(1) lookup regardless of size:
//! ```text
//! [Original EXE bytes]
//! [PAYLOAD_MARKER: 16 bytes "VELOCITY_PKG_V1\0"]
//! [MANIFEST_LEN: 8 bytes, u64 little-endian]
//! [MANIFEST_JSON: manifest_len bytes]
//! [PAYLOAD_LEN: 8 bytes, u64 little-endian]
//! [PAYLOAD_DATA: payload_len bytes, zstd-compressed tar]
//! [TRAILER_OFFSET: 8 bytes, u64 LE — byte offset of PAYLOAD_MARKER from start]
//! ```
//!
//! The trailer (last 8 bytes) stores the absolute byte offset of the marker.
//! To read: grab the last 8 bytes → seek to that offset → verify marker → read data.
//! This is O(1) and works for installers of any size.
//!
//! A fallback scan is provided for backward compatibility with older installers
//! that were built without the trailer.

use crate::error::{CoreError, Result};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Magic marker appended before the payload data.
const PAYLOAD_MARKER: &[u8; 16] = b"VELOCITY_PKG_V1\0";

/// Size of the trailer (one u64 LE offset).
const TRAILER_SIZE: u64 = 8;

/// Minimum valid file size: marker(16) + manifest_len(8) + payload_len(8) + trailer(8) = 40
const MIN_PAYLOAD_FILE_SIZE: u64 = 40;

/// Create a payload by appending manifest + compressed files to a base executable.
///
/// Writes the runtime stub, marker, manifest, payload data, and a trailer
/// containing the marker offset as the last 8 bytes of the file.
pub fn create_payload(
    base_exe: &[u8],
    manifest_json: &[u8],
    compressed_data: &[u8],
    output: &Path,
) -> Result<()> {
    let mut file = std::fs::File::create(output)?;

    // Write the base executable (runtime stub)
    file.write_all(base_exe)?;

    // Record the marker position (this is the trailer value)
    let marker_offset = base_exe.len() as u64;

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

    // Write the trailer: absolute offset of the marker (last 8 bytes of file)
    file.write_all(&marker_offset.to_le_bytes())?;

    file.flush()?;
    Ok(())
}

/// Read the embedded manifest and payload from a Velocity installer executable.
///
/// Uses the trailer (last 8 bytes) for O(1) lookup. Falls back to a full-file
/// scan for backward compatibility with installers built before the trailer
/// was added.
pub fn read_payload(exe_path: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut file = std::fs::File::open(exe_path)?;
    let file_len = file.metadata()?.len();

    if file_len < MIN_PAYLOAD_FILE_SIZE {
        return Err(CoreError::InvalidPayload(format!(
            "File too small to be a Velocity installer ({} bytes, minimum {})",
            file_len, MIN_PAYLOAD_FILE_SIZE
        )));
    }

    // Fast path: read the trailer (last 8 bytes) to get the marker offset
    let marker_pos = read_trailer(&mut file, file_len)
        .or_else(|_| find_marker(&mut file, file_len))?;

    // Read past the marker
    file.seek(SeekFrom::Start(marker_pos + PAYLOAD_MARKER.len() as u64))?;

    // Read manifest length + manifest
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let manifest_len = u64::from_le_bytes(len_buf);

    // Sanity check: reject manifests larger than 100MB
    if manifest_len > 100 * 1024 * 1024 {
        return Err(CoreError::InvalidPayload(format!(
            "Manifest length {} exceeds maximum allowed size (100MB)",
            manifest_len
        )));
    }
    let manifest_len = manifest_len as usize;

    let mut manifest_data = vec![0u8; manifest_len];
    file.read_exact(&mut manifest_data)?;

    // Read payload length + payload
    file.read_exact(&mut len_buf)?;
    let payload_len = u64::from_le_bytes(len_buf);

    // Sanity check: reject payloads larger than 4GB
    if payload_len > 4 * 1024 * 1024 * 1024 {
        return Err(CoreError::InvalidPayload(format!(
            "Payload length {} exceeds maximum allowed size (4GB)",
            payload_len
        )));
    }
    let payload_len = payload_len as usize;

    let mut payload_data = vec![0u8; payload_len];
    file.read_exact(&mut payload_data)?;

    Ok((manifest_data, payload_data))
}

/// Read the marker offset from the trailer (last 8 bytes of the file).
///
/// Validates that the offset points to a valid marker in the file.
fn read_trailer(file: &mut std::fs::File, file_len: u64) -> Result<u64> {
    // Seek to the last 8 bytes
    file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
    let mut trailer_buf = [0u8; 8];
    file.read_exact(&mut trailer_buf)?;
    let marker_offset = u64::from_le_bytes(trailer_buf);

    // Validate: offset must be before the trailer and after position 0
    let trailer_start = file_len - TRAILER_SIZE;
    if marker_offset == 0 || marker_offset >= trailer_start {
        return Err(CoreError::InvalidPayload(
            "Trailer offset out of range".to_string(),
        ));
    }

    // Seek to the claimed marker position and verify the marker bytes
    file.seek(SeekFrom::Start(marker_offset))?;
    let mut marker_buf = [0u8; 16];
    file.read_exact(&mut marker_buf)?;
    if &marker_buf != PAYLOAD_MARKER.as_slice() {
        return Err(CoreError::InvalidPayload(
            "Trailer offset does not point to a valid marker".to_string(),
        ));
    }

    Ok(marker_offset)
}

/// Find the payload marker by scanning the entire file (fallback for legacy installers).
fn find_marker(file: &mut std::fs::File, file_len: u64) -> Result<u64> {
    let mut buf = vec![0u8; file_len as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut buf)?;

    // Search backwards from the end for the last occurrence of the marker
    for i in (0..=buf.len().saturating_sub(PAYLOAD_MARKER.len())).rev() {
        if &buf[i..i + PAYLOAD_MARKER.len()] == PAYLOAD_MARKER.as_slice() {
            return Ok(i as u64);
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

    if file_len < MIN_PAYLOAD_FILE_SIZE {
        return Err(CoreError::InvalidPayload(format!(
            "File too small to be a Velocity installer ({} bytes)",
            file_len
        )));
    }

    let marker_pos = read_trailer(&mut file, file_len)
        .or_else(|_| find_marker(&mut file, file_len))?;
    Ok(marker_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_create_and_read_large_payload() {
        // Simulate a large installer (5 MB fake runtime + 2 MB payload)
        let temp_dir = std::env::temp_dir();
        let base_exe = vec![0xABu8; 5 * 1024 * 1024]; // 5 MB fake runtime
        let manifest = b"{\"app\":\"large-test\"}";
        let payload = vec![0xCDu8; 2 * 1024 * 1024]; // 2 MB fake payload
        let output = temp_dir.join("velocity_test_large_payload.exe");

        create_payload(&base_exe, manifest, &payload, &output).unwrap();

        let (read_manifest, read_payload) = read_payload(&output).unwrap();
        assert_eq!(read_manifest, manifest);
        assert_eq!(read_payload, payload);

        let base_size = get_base_exe_size(&output).unwrap();
        assert_eq!(base_size, base_exe.len() as u64);

        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn test_trailer_validates_marker() {
        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join("velocity_test_bad_trailer.exe");

        // Write a file with a bogus trailer that doesn't point to a valid marker
        let mut file = std::fs::File::create(&output).unwrap();
        file.write_all(b"SOME_DATA_HERE!!").unwrap(); // 16 bytes
        file.write_all(&[0u8; 32]).unwrap(); // padding
        file.write_all(&999u64.to_le_bytes()).unwrap(); // bad trailer offset
        drop(file);

        let result = read_payload(&output);
        assert!(result.is_err());

        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn test_read_payload_invalid_too_small() {
        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join("velocity_test_tiny.exe");
        std::fs::write(&output, b"tiny").unwrap();

        let result = read_payload(&output);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));

        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn test_read_payload_invalid_no_marker() {
        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join("velocity_test_no_marker.exe");
        // Write enough bytes to pass the min size check but with no valid marker
        std::fs::write(&output, vec![0u8; 100]).unwrap();

        let result = read_payload(&output);
        assert!(result.is_err());

        std::fs::remove_file(&output).ok();
    }
}
