//! HTTP file downloader with progress tracking and integrity verification.
//!
//! Uses WinHTTP for HTTP/HTTPS downloads, supporting:
//! - Progress callbacks for UI updates
//! - SHA256 hash verification
//! - Timeout handling
//!
//! Windows-only: uses WinHTTP API. Cross-platform HTTP will be provided
//! via the platform module.

#![cfg(target_os = "windows")]

use crate::error::{CoreError, Result};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Progress callback type: (bytes_downloaded, total_bytes_or_0, url)
pub type DownloadProgressCallback = Box<dyn Fn(u64, u64, &str) + Send>;

/// Download a file from a URL to a local path.
///
/// Returns the path to the downloaded file.
/// If `expected_sha256` is provided, verifies the hash after download.
pub fn download_file(
    url: &str,
    output_dir: &Path,
    filename: Option<&str>,
    expected_sha256: Option<&str>,
    progress: Option<&DownloadProgressCallback>,
) -> Result<PathBuf> {
    info!("Downloading: {}", url);

    // Determine output filename
    let fname = filename.map(|s| s.to_string()).unwrap_or_else(|| {
        sanitize_filename(
            url.rsplit('/')
                .next()
                .unwrap_or("download")
                .split('?')
                .next()
                .unwrap_or("download"),
        )
    });

    std::fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(&fname);

    // Download using WinHTTP
    let data = download_with_winhttp(url, None, progress)?;

    // Verify SHA256 if provided
    if let Some(expected) = expected_sha256 {
        let actual = compute_sha256(&data);
        let expected_lower = expected.to_lowercase();
        if actual != expected_lower {
            return Err(CoreError::other(
                "SHA256 verification",
                format!(
                    "SHA256 mismatch for {}: expected {}, got {}",
                    fname, expected_lower, actual
                ),
            ));
        }
        debug!("SHA256 verified: {}", actual);
    }

    // Write to file
    let mut file = std::fs::File::create(&output_path)?;
    file.write_all(&data)?;

    info!(
        "Downloaded {} bytes to {}",
        data.len(),
        output_path.display()
    );
    Ok(output_path)
}

/// Download content from a URL into memory (no file written to disk).
///
/// Returns the raw bytes downloaded. Useful for fetching small payloads
/// such as version-info JSON for the self-update mechanism.
pub fn download_to_memory(url: &str) -> Result<Vec<u8>> {
    info!("Downloading to memory: {}", url);
    let data = download_with_winhttp(url, None, None)?;
    info!("Downloaded {} bytes (in memory)", data.len());
    Ok(data)
}

/// Download a file with resume support.
///
/// If a partial download exists at the output path, attempts to resume
/// from where it left off using HTTP Range requests.
/// Returns the path to the downloaded file.
pub fn download_file_resumable(
    url: &str,
    output_dir: &Path,
    filename: Option<&str>,
    expected_sha256: Option<&str>,
    progress: Option<&DownloadProgressCallback>,
) -> Result<PathBuf> {
    let fname = filename.map(|s| s.to_string()).unwrap_or_else(|| {
        sanitize_filename(
            url.rsplit('/')
                .next()
                .unwrap_or("download")
                .split('?')
                .next()
                .unwrap_or("download"),
        )
    });

    std::fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(&fname);
    let partial_path = output_dir.join(format!("{}.partial", fname));

    // Check for existing partial download
    let existing_bytes = if partial_path.exists() {
        let meta = std::fs::metadata(&partial_path)?;
        let size = meta.len();
        info!("Found partial download: {} bytes", size);
        size
    } else {
        0
    };

    // Download data, attempting resume if we have a partial file
    let (new_data, resumed) = if existing_bytes > 0 {
        match download_with_winhttp_resume(url, existing_bytes, progress) {
            Ok((data, true)) => {
                // Server supported the range request — append to partial
                (data, true)
            }
            Ok((data, false)) => {
                // Server did NOT support range — restart from scratch
                info!("Server doesn't support range requests, restarting download");
                let _ = std::fs::remove_file(&partial_path);
                (data, false)
            }
            Err(e) => {
                // Resume failed — clean up partial and retry from scratch
                warn!("Resume failed ({}), restarting download", e);
                let _ = std::fs::remove_file(&partial_path);
                let data = download_with_winhttp(url, None, progress)?;
                (data, false)
            }
        }
    } else {
        let data = download_with_winhttp(url, None, progress)?;
        (data, false)
    };

    // Write the file
    if resumed {
        // Append new data to partial file, then rename to final
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&partial_path)?;
        file.write_all(&new_data)?;
        drop(file);
        std::fs::rename(&partial_path, &output_path)?;
    } else {
        // Fresh download — write directly to output
        let mut file = std::fs::File::create(&output_path)?;
        file.write_all(&new_data)?;
    }

    // Verify SHA256 if provided
    if let Some(expected) = expected_sha256 {
        let actual = compute_sha256_file(&output_path)?;
        let expected_lower = expected.to_lowercase();
        if actual != expected_lower {
            // Delete corrupt file
            let _ = std::fs::remove_file(&output_path);
            return Err(CoreError::other(
                "SHA256 verification",
                format!(
                    "SHA256 mismatch for {}: expected {}, got {}",
                    fname, expected_lower, actual
                ),
            ));
        }
        debug!("SHA256 verified: {}", actual);
    }

    // Clean up partial file if it still exists
    if partial_path.exists() {
        let _ = std::fs::remove_file(&partial_path);
    }

    info!("Download complete: {}", output_path.display());
    Ok(output_path)
}

/// Download data from a URL using WinHTTP, with resume support.
/// Returns (data, did_resume). If the server doesn't support range requests,
/// returns the full content with `did_resume = false`.
fn download_with_winhttp_resume(
    url: &str,
    resume_from: u64,
    progress: Option<&DownloadProgressCallback>,
) -> Result<(Vec<u8>, bool)> {
    use windows::core::*;
    use windows::Win32::Networking::WinHttp::*;

    // SAFETY: WinHTTP session lifecycle — handles are closed in reverse order
    // (request → connection → session) on all paths including errors.
    // PCWSTR pointers derive from stack-allocated Vec<u16> that outlive the calls.
    unsafe {
        let agent: Vec<u16> = "Velocity Installer\0".encode_utf16().collect();
        let session = WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            None,
            None,
            0,
        );
        if session.is_null() {
            return Err(CoreError::other(
                "WinHttpOpen",
                format!("{}", std::io::Error::last_os_error()),
            ));
        }

        let (host, port, path, is_https) = parse_url(url)?;
        let host_wide: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        let connection = WinHttpConnect(session, PCWSTR(host_wide.as_ptr()), port, 0);
        if connection.is_null() {
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other("WinHttpConnect", "failed to connect"));
        }

        let flags = if is_https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };

        let request = WinHttpOpenRequest(
            connection,
            None,
            PCWSTR(path_wide.as_ptr()),
            PCWSTR::null(),
            None,
            std::ptr::null(),
            flags,
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other("WinHttpOpenRequest", "failed"));
        }

        if is_https {
            let sec_flags: u32 = 0;
            let buffer = sec_flags.to_ne_bytes();
            let _ = WinHttpSetOption(Some(request), WINHTTP_OPTION_SECURITY_FLAGS, Some(&buffer));
        }

        // Add Range header
        let range_header: Vec<u16> = format!("Range: bytes={}-\r\n", resume_from)
            .encode_utf16()
            .collect();
        let _ = WinHttpAddRequestHeaders(request, &range_header, WINHTTP_ADDREQ_FLAG_ADD);

        let send_result = WinHttpSendRequest(request, None, None, 0, 0, 0);
        if send_result.is_err() {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other(
                "WinHttpSendRequest",
                format!("{}", std::io::Error::last_os_error()),
            ));
        }

        let receive_result = WinHttpReceiveResponse(request, std::ptr::null_mut());
        if receive_result.is_err() {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other("WinHttpReceiveResponse", "failed"));
        }

        // Check if server returned 206 Partial Content (range supported)
        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        let mut index = 0u32;
        let got_status = WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            None,
            Some(&mut status_code as *mut u32 as *mut std::ffi::c_void),
            &mut status_size,
            &mut index,
        );

        let range_supported = got_status.is_ok() && status_code == 206;

        if !range_supported {
            // Server returned 200 OK — it ignored our Range header.
            // Read the full response body.
            let mut data = Vec::new();
            let mut bytes_read: u32 = 0;
            let mut buffer = [0u8; 8192];
            loop {
                let read_result = WinHttpReadData(
                    request,
                    buffer.as_mut_ptr() as *mut std::ffi::c_void,
                    buffer.len() as u32,
                    &mut bytes_read,
                );
                if read_result.is_err() || bytes_read == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..bytes_read as usize]);
            }
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Ok((data, false));
        }

        // 206 Partial Content — read the remaining data
        let mut data = Vec::new();
        let mut bytes_read: u32 = 0;
        let mut buffer = [0u8; 8192];
        loop {
            let read_result = WinHttpReadData(
                request,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                buffer.len() as u32,
                &mut bytes_read,
            );
            if read_result.is_err() || bytes_read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..bytes_read as usize]);
            if let Some(cb) = progress {
                cb(resume_from + data.len() as u64, 0, url);
            }
        }

        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connection);
        let _ = WinHttpCloseHandle(session);

        Ok((data, true))
    }
}

/// Download data from a URL using WinHTTP.
/// If `resume_from` is Some(n), sends a Range header to resume from byte n.
fn download_with_winhttp(
    url: &str,
    resume_from: Option<u64>,
    progress: Option<&DownloadProgressCallback>,
) -> Result<Vec<u8>> {
    use windows::core::*;
    use windows::Win32::Networking::WinHttp::*;

    // SAFETY: WinHTTP session lifecycle — handles closed in reverse order on all paths.
    // PCWSTR pointers from stack-allocated Vec<u16> that outlive the calls.
    unsafe {
        // Open WinHTTP session — use default proxy
        let agent: Vec<u16> = "Velocity Installer\0".encode_utf16().collect();
        let session = WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            None,
            None,
            0,
        );
        if session.is_null() {
            return Err(CoreError::other(
                "WinHttpOpen",
                format!("{}", std::io::Error::last_os_error()),
            ));
        }

        // Parse URL to get host and path
        let (host, port, path, is_https) = parse_url(url)?;

        let host_wide: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // Connect to server
        let connection = WinHttpConnect(session, PCWSTR(host_wide.as_ptr()), port, 0);
        if connection.is_null() {
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other("WinHttpConnect", "failed to connect"));
        }

        // Determine flags for HTTPS
        let flags = if is_https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };

        // Open request
        let request = WinHttpOpenRequest(
            connection,
            None,                       // pwszverb (GET)
            PCWSTR(path_wide.as_ptr()), // pwszobjectname
            PCWSTR::null(),             // pwszversion
            None,                       // pwszreferrer
            std::ptr::null(),           // ppwszaccepttypes (null array)
            flags,                      // dwflags
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other(
                "WinHttpOpenRequest",
                "failed to open request",
            ));
        }

        // For HTTPS, enable standard certificate validation.
        // We rely on the Windows certificate store for CA trust verification.
        // No certificate errors are ignored — all downloads must use valid TLS.
        if is_https {
            let sec_flags: u32 = 0; // Use default secure flags (full validation)
            let buffer = sec_flags.to_ne_bytes();
            let _ = WinHttpSetOption(Some(request), WINHTTP_OPTION_SECURITY_FLAGS, Some(&buffer));
        }

        // Add Range header for resume support
        if let Some(offset) = resume_from {
            let range_header: Vec<u16> = format!("Range: bytes={}-\r\n", offset)
                .encode_utf16()
                .collect();
            let _ = WinHttpAddRequestHeaders(request, &range_header, WINHTTP_ADDREQ_FLAG_ADD);
            debug!("Resuming download from byte {}", offset);
        }

        // Send request (6 args: request, headers, headers_len, optional, optional_len, context)
        let send_result = WinHttpSendRequest(request, None, None, 0, 0, 0);
        if send_result.is_err() {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other(
                "WinHttpSendRequest",
                format!("{}", std::io::Error::last_os_error()),
            ));
        }

        // Receive response
        let receive_result = WinHttpReceiveResponse(request, std::ptr::null_mut());
        if receive_result.is_err() {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return Err(CoreError::other("WinHttpReceiveResponse", "failed"));
        }

        // Get content length
        let mut content_len: u64 = 0;
        let mut len_buf = [0u16; 32];
        let mut len_size = std::mem::size_of_val(&len_buf) as u32;
        let mut index = 0u32;
        if WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_CONTENT_LENGTH,
            None,
            Some(len_buf.as_mut_ptr() as *mut std::ffi::c_void),
            &mut len_size,
            &mut index,
        )
        .is_ok()
        {
            let len_str = String::from_utf16_lossy(&len_buf[..(len_size as usize / 2)]);
            content_len = len_str.trim().parse().unwrap_or(0);
        }

        // Read data
        let mut data = Vec::new();
        let mut bytes_read: u32 = 0;
        let mut buffer = [0u8; 8192];

        loop {
            let read_result = WinHttpReadData(
                request,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                buffer.len() as u32,
                &mut bytes_read,
            );
            if read_result.is_err() || bytes_read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..bytes_read as usize]);

            if let Some(cb) = progress {
                cb(data.len() as u64, content_len, url);
            }
        }

        // Cleanup
        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connection);
        let _ = WinHttpCloseHandle(session);

        if data.is_empty() {
            warn!("Downloaded 0 bytes from {}", url);
        }

        Ok(data)
    }
}

/// Sanitize a string for use as a filename.
/// Removes path separators, null bytes, and other dangerous characters.
fn sanitize_filename(name: &str) -> String {
    // Take only the last path component to prevent directory traversal
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);

    let sanitized: String = basename
        .chars()
        .map(|c| {
            match c {
                // Allow alphanumeric, hyphens, underscores, dots
                c if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' => c,
                // Replace everything else with underscore
                _ => '_',
            }
        })
        .collect();

    // Strip leading dots to prevent path traversal (e.g., "..hidden" → "hidden")
    let sanitized = sanitized.trim_start_matches('.').to_string();

    // Reject empty or dot-only names
    if sanitized.is_empty() {
        return "download".to_string();
    }

    // Limit length to 255 characters (common filesystem limit)
    if sanitized.len() > 255 {
        sanitized[..255].to_string()
    } else {
        sanitized
    }
}

/// Parse a URL into (host, port, path, is_https).
/// Handles IPv6 addresses in bracket notation (e.g., `http://[::1]:8080/path`).
fn parse_url(url: &str) -> Result<(String, u16, String, bool)> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else {
        return Err(CoreError::other(
            "URL parsing",
            format!("Unsupported URL scheme: {}", url),
        ));
    };

    let is_https = scheme == "https";
    let default_port: u16 = if is_https { 443 } else { 80 };

    // Split host and path
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    // Handle IPv6 bracket notation: [::1] or [::1]:port
    let (host, port) = if host_port.starts_with('[') {
        // IPv6 address
        match host_port.find(']') {
            Some(bracket_end) => {
                let ipv6_host = &host_port[..=bracket_end];
                let after_bracket = &host_port[bracket_end + 1..];
                if let Some(port_str) = after_bracket.strip_prefix(':') {
                    match port_str.parse::<u16>() {
                        Ok(p) => (ipv6_host, p),
                        Err(_) => (ipv6_host, default_port),
                    }
                } else {
                    (ipv6_host, default_port)
                }
            }
            None => (host_port, default_port),
        }
    } else {
        // Standard host:port
        match host_port.rfind(':') {
            Some(idx) => {
                let port_str = &host_port[idx + 1..];
                match port_str.parse::<u16>() {
                    Ok(p) => (&host_port[..idx], p),
                    Err(_) => (host_port, default_port),
                }
            }
            None => (host_port, default_port),
        }
    };

    Ok((host.to_string(), port, path.to_string(), is_https))
}

/// Compute SHA256 hash of data, returning lowercase hex string.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compute SHA256 hash of a file using streaming (no full-file allocation).
pub fn compute_sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, &mut file);
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)?;
    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_https() {
        let (host, port, path, is_https) = parse_url("https://example.com/file.exe").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/file.exe");
        assert!(is_https);
    }

    #[test]
    fn test_parse_url_http_with_port() {
        let (host, port, path, is_https) = parse_url("http://localhost:8080/api/download").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert_eq!(path, "/api/download");
        assert!(!is_https);
    }

    #[test]
    fn test_parse_url_no_path() {
        let (host, port, path, _) = parse_url("https://example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_compute_sha256_empty() {
        let hash = compute_sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_parse_url_ipv6() {
        let (host, port, path, is_https) = parse_url("http://[::1]:8080/api/download").unwrap();
        assert_eq!(host, "[::1]");
        assert_eq!(port, 8080);
        assert_eq!(path, "/api/download");
        assert!(!is_https);
    }

    #[test]
    fn test_parse_url_ipv6_no_port() {
        let (host, port, path, _) = parse_url("https://[::1]/path").unwrap();
        assert_eq!(host, "[::1]");
        assert_eq!(port, 443);
        assert_eq!(path, "/path");
    }

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("installer.exe"), "installer.exe");
    }

    #[test]
    fn test_sanitize_filename_path_traversal() {
        let result = sanitize_filename("../../etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.contains(".."));
    }

    #[test]
    fn test_sanitize_filename_empty() {
        assert_eq!(sanitize_filename(""), "download");
    }
}
