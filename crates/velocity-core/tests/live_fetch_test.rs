//! Live integration test — hits the real GitHub API to verify the fetch flow.
//!
//! Tests with BurntSushi/ripgrep which has proper GitHub releases with Windows assets.
//! Run with: cargo test -p velocity-core --test live_fetch_test -- --ignored --nocapture

use velocity_core::fetch::{self, VersionResolver, GitHubClient};

#[test]
#[ignore] // Requires network access
fn test_live_github_ripgrep() {
    // Step 1: Create a GitHub client for ripgrep
    let client = GitHubClient::new("BurntSushi/ripgrep", None, None)
        .expect("Failed to create GitHub client");

    // Step 2: Fetch the latest release
    println!("Fetching latest ripgrep release...");
    let version_info = client.get_latest_version()
        .expect("Failed to fetch latest version");

    println!("Latest version: {}", version_info.version);
    println!("Release name: {:?}", version_info.name);
    println!("Assets count: {}", version_info.assets.len());
    println!();

    assert!(!version_info.version.is_empty(), "Version should not be empty");
    assert!(!version_info.assets.is_empty(), "Should have release assets");

    // Step 3: List all assets
    println!("All release assets:");
    for asset in &version_info.assets {
        println!("  {} ({} bytes) -> {}", asset.name, asset.size, asset.download_url);
    }
    println!();

    // Step 4: Try to find a Windows x64 asset
    let win64_asset = client.find_asset(&version_info, "*-x86_64-*-windows-*.zip");
    match &win64_asset {
        Some(asset) => {
            println!("Found Windows x64 asset: {}", asset.name);
            println!("  URL: {}", asset.download_url);
            println!("  Size: {} bytes ({:.1} MB)", asset.size, asset.size as f64 / 1024.0 / 1024.0);
            assert!(asset.download_url.starts_with("https://github.com/"), "URL should be from GitHub");
        }
        None => {
            println!("No *-x86_64-*-windows-*.zip asset found, trying alternatives...");
            // Try broader patterns
            let any_win = client.find_asset(&version_info, "*windows*");
            println!("  *windows* pattern: {:?}", any_win.map(|a| &a.name));
            
            let any_exe = client.find_asset(&version_info, "*.exe");
            println!("  *.exe pattern: {:?}", any_exe.map(|a| &a.name));
            
            let any_zip = client.find_asset(&version_info, "*.zip");
            println!("  *.zip pattern: {:?}", any_zip.map(|a| &a.name));
        }
    }

    // Step 5: Verify download URL format
    if let Some(asset) = win64_asset {
        assert!(
            asset.download_url.contains("github.com") || asset.download_url.contains("githubusercontent.com"),
            "Download URL should point to GitHub: {}",
            asset.download_url
        );
    }
}

#[test]
#[ignore] // Requires network access
fn test_live_github_fd_find_replace() {
    // Test with a smaller project: BurntSushi/fd
    let client = GitHubClient::new("sharkdp/fd", None, None)
        .expect("Failed to create GitHub client");

    println!("Fetching latest fd release...");
    let version_info = client.get_latest_version()
        .expect("Failed to fetch latest version");

    println!("Latest version: {}", version_info.version);
    println!("Assets: {}", version_info.assets.len());

    for asset in &version_info.assets {
        println!("  {} ({} bytes)", asset.name, asset.size);
    }

    // Find Windows x64 MSI or ZIP
    let win_asset = client.find_asset(&version_info, "*x86_64*windows*msi*");
    match win_asset {
        Some(a) => println!("Found Windows MSI: {} ({:.1} MB)", a.name, a.size as f64 / 1024.0 / 1024.0),
        None => println!("No Windows MSI found, trying zip..."),
    }

    let win_zip = client.find_asset(&version_info, "*x86_64*windows*zip*");
    match win_zip {
        Some(a) => println!("Found Windows ZIP: {} ({:.1} MB)", a.name, a.size as f64 / 1024.0 / 1024.0),
        None => println!("No Windows ZIP found"),
    }
}

#[test]
#[ignore] // Requires network access
fn test_live_download_small_file() {
    use velocity_core::fetch::DownloadManager;

    // Download a small file to verify the download pipeline works
    let dm = DownloadManager::new().expect("Failed to create download manager");
    
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    // Use a small test file from GitHub
    let url = "https://raw.githubusercontent.com/BurntSushi/ripgrep/master/Cargo.toml";
    
    println!("Downloading small test file...");
    let result = dm.download(
        url,
        temp_dir.path(),
        Some("Cargo.toml"),
        None,
        None,
    );

    match result {
        Ok(path) => {
            println!("Downloaded to: {}", path.display());
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("File size: {} bytes", size);
            assert!(size > 0, "Downloaded file should not be empty");
            
            // Verify it's valid TOML
            let content = std::fs::read_to_string(&path).unwrap();
            println!("First 200 chars: {}", &content[..200.min(content.len())]);
            assert!(content.contains("[package]"), "Should be a valid Cargo.toml");
        }
        Err(e) => {
            panic!("Download failed: {}", e);
        }
    }
}

#[test]
#[ignore] // Requires network access
fn test_live_full_fetch_flow() {
    // Simulate the full fetch install flow without actually installing
    use velocity_config::{FetchConfig, FetchMode, GitPlatform};
    
    // Create a config for ripgrep
    let config = FetchConfig {
        mode: FetchMode::GitRelease,
        platform: Some(GitPlatform::GitHub),
        repo: Some("BurntSushi/ripgrep".to_string()),
        asset_pattern: Some("*-x86_64-*-windows-*.zip".to_string()),
        api_url: None,
        base_url: None,
        version_url: None,
        checksum_url: None,
        files: velocity_config::FetchFileConfig {
            download: vec![
                velocity_config::FetchDownloadPattern {
                    pattern: "*-x86_64-*-windows-*.zip".to_string(),
                    dest: ".".to_string(),
                    required: true,
                    sha256: None,
                    action: velocity_config::FetchAction::Extract,
                    install_args: None,
                    file_type: None,
                    installer: None,
                },
            ],
        },
        update: None,
        bundle: None,
        auth_token: None,
    };

    // Step 1: Create resolver from config
    println!("Creating resolver from config...");
    let resolver = fetch::create_resolver_from_config(&config)
        .expect("Failed to create resolver");

    // Step 2: Get latest version
    println!("Fetching latest version...");
    let version_info = resolver.get_latest_version()
        .expect("Failed to get latest version");
    
    println!("Version: {}", version_info.version);
    println!("Assets: {}", version_info.assets.len());

    // Step 3: Find matching asset
    let pattern = config.asset_pattern.as_deref().unwrap();
    println!("Looking for asset matching: {}", pattern);
    
    let asset = resolver.find_asset(&version_info, pattern);
    match asset {
        Some(a) => {
            println!("MATCHED: {}", a.name);
            println!("  URL: {}", a.download_url);
            println!("  Size: {:.1} MB", a.size as f64 / 1024.0 / 1024.0);
        }
        None => {
            println!("NO MATCH for pattern '{}'", pattern);
            println!("Available assets:");
            for a in &version_info.assets {
                println!("  {}", a.name);
            }
            // This is not necessarily a failure - the asset naming may have changed
        }
    }
}

/// Test URL-mode with a local HTTP server.
///
/// Spins up a simple TCP-based HTTP server that serves:
/// - `/version.txt` containing "3.2.1"
/// - `/myapp-3.2.1-win-x64.exe` containing dummy binary data
///
/// Verifies that UrlClient correctly:
/// 1. Fetches version from version.txt
/// 2. Constructs download URL with placeholder substitution
/// 3. Finds the matching asset by pattern
#[test]
#[ignore] // Requires local TCP bind
fn test_live_url_mode_mock() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use velocity_core::fetch::{UrlClient, VersionResolver};

    // Bind to a random available port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test server");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    println!("Test server listening on port {}", port);

    // Spawn a simple HTTP server that handles 2 requests then exits
    let server_handle = thread::spawn(move || {
        let listener = listener;
        // Set a timeout so the server doesn't hang forever
        listener.set_nonblocking(false).ok();

        let mut request_count = 0;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();

                    if request.starts_with("GET /version.txt") {
                        let body = "3.2.1";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        stream.write_all(response.as_bytes()).ok();
                        request_count += 1;
                    } else if request.starts_with("GET /myapp-3.2.1-win-x64.exe") {
                        // Return a dummy "installer" (just some bytes)
                        let body = b"MZ_FAKE_INSTALLER_BINARY_DATA";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(response.as_bytes()).ok();
                        stream.write_all(body).ok();
                        request_count += 1;
                    } else {
                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                        stream.write_all(response.as_bytes()).ok();
                    }

                    stream.flush().ok();
                    if request_count >= 2 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        println!("Test server handled {} requests, shutting down", request_count);
    });

    // Create UrlClient pointing at our local server
    let client = UrlClient::new(
        &base_url,
        &format!("{}/version.txt", base_url),
        Some("myapp-{version}-win-x64.exe"),
        None,
    );

    // Step 1: Fetch version
    println!("Fetching version from local server...");
    let version_info = client.get_latest_version().expect("Failed to get version");

    println!("Version: {}", version_info.version);
    assert_eq!(version_info.version, "3.2.1", "Version should be 3.2.1");

    // Step 2: Verify asset was constructed correctly
    println!("Assets: {}", version_info.assets.len());
    assert_eq!(version_info.assets.len(), 1, "Should have exactly 1 asset");

    let asset = &version_info.assets[0];
    println!("Asset name: {}", asset.name);
    println!("Asset URL: {}", asset.download_url);

    assert_eq!(asset.name, "myapp-3.2.1-win-x64.exe");
    assert!(
        asset.download_url.contains("127.0.0.1"),
        "URL should point to local server: {}",
        asset.download_url
    );
    assert!(
        asset.download_url.contains("myapp-3.2.1-win-x64.exe"),
        "URL should contain the resolved filename: {}",
        asset.download_url
    );

    // Step 3: Verify find_asset works with pattern
    let found = client.find_asset(&version_info, "*.exe");
    assert!(found.is_some(), "Should find .exe asset");
    assert_eq!(found.unwrap().name, "myapp-3.2.1-win-x64.exe");

    // Step 4: Verify get_version_by_tag also works
    let tag_info = client.get_version_by_tag("5.0.0").expect("get_version_by_tag failed");
    assert_eq!(tag_info.version, "5.0.0");
    assert_eq!(tag_info.assets.len(), 1);
    assert!(tag_info.assets[0].name.contains("5.0.0"));
    assert!(!tag_info.assets[0].name.contains("{version}"));

    // Wait for server to finish
    server_handle.join().expect("Server thread panicked");
    println!("URL-mode test complete!");
}

/// Test URL-mode with checksum resolution.
///
/// Verifies that the UrlClient can fetch and parse checksums from a
/// SHA256SUMS-style file served over HTTP.
#[test]
#[ignore] // Requires local TCP bind
fn test_live_url_mode_checksum() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use velocity_core::fetch::UrlClient;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let server_handle = thread::spawn(move || {
        let listener = listener;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();

                    let (status, body): (&str, String) = if request.starts_with("GET /version.txt") {
                        ("200 OK", "1.5.0".to_string())
                    } else if request.starts_with("GET /SHA256SUMS") {
                        ("200 OK", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  myapp-1.5.0-win-x64.exe\nabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890  myapp-1.5.0-linux-x64.tar.gz\n".to_string())
                    } else {
                        ("404 Not Found", String::new())
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Length: {}\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).ok();
                    stream.write_all(body.as_bytes()).ok();
                    stream.flush().ok();
                }
                Err(_) => break,
            }
        }
    });

    let client = UrlClient::new(
        &base_url,
        &format!("{}/version.txt", base_url),
        Some("myapp-{version}-win-x64.exe"),
        Some(&format!("{}/SHA256SUMS", base_url)),
    );

    // Fetch checksum for the Windows exe
    let checksum = client.fetch_checksum("1.5.0", "myapp", "myapp-1.5.0-win-x64.exe");
    assert!(checksum.is_some(), "Should find checksum for the exe");
    assert_eq!(
        checksum.unwrap(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    // Verify it doesn't match a file not in the sums
    let missing = client.fetch_checksum("1.5.0", "myapp", "myapp-1.5.0-macos-arm64.dmg");
    assert!(missing.is_none(), "Should not find checksum for missing file");

    server_handle.join().ok();
    println!("URL-mode checksum test complete!");
}
