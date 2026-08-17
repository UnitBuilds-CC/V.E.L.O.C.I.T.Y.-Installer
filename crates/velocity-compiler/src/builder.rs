//! Installer builder — compiles a project into a standalone installer .exe.

use crate::error::{CompilerError, Result};
use std::path::{Path, PathBuf};
use tracing::info;
use velocity_config::collect_files;

/// Options for the build process.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Path to the project directory (containing velocity.toml)
    pub project_dir: PathBuf,
    /// Output path for the installer .exe
    pub output_path: PathBuf,
    /// Compression level (0-22, default 3)
    pub compression_level: i32,
    /// Compression format: "zstd" (default, fast) or "lzma2" (smaller, slower)
    pub compression_format: String,
    /// Path to the runtime binary (if not found automatically)
    pub runtime_path: Option<PathBuf>,
    /// Whether to build in quiet mode (no output)
    pub quiet: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            project_dir: std::env::current_dir().unwrap_or_default(),
            output_path: PathBuf::from("output/installer.exe"),
            compression_level: 3,
            compression_format: "zstd".to_string(),
            runtime_path: None,
            quiet: false,
        }
    }
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildResult {
    /// Path to the generated installer
    pub installer_path: PathBuf,
    /// Total size of the installer in bytes
    pub installer_size: u64,
    /// Number of files included
    pub file_count: usize,
    /// Size of the payload (compressed) in bytes
    pub payload_size: u64,
    /// Size of the original files (uncompressed) in bytes
    pub original_size: u64,
}

/// Build a standalone installer from a Velocity project.
pub fn build_installer(options: &BuildOptions) -> Result<BuildResult> {
    info!("Building installer from: {}", options.project_dir.display());

    // Step 1: Parse the manifest
    let manifest_path = options.project_dir.join("velocity.toml");
    if !manifest_path.exists() {
        return Err(CompilerError::BuildFailed(
            "velocity.toml not found in project directory".to_string(),
        ));
    }

    let manifest = velocity_config::parse_manifest(&manifest_path)?;
    info!("Building installer for: {} v{}", manifest.app.name, manifest.app.version);

    // Step 2: Collect files to package
    let files = collect_files(&manifest, &options.project_dir)?;
    if files.is_empty() {
        return Err(CompilerError::NoFilesFound);
    }

    let original_size: u64 = files.iter()
        .filter_map(|(path, _)| std::fs::metadata(path).ok().map(|m| m.len()))
        .sum();

    info!("Collected {} files ({} bytes uncompressed)", files.len(), original_size);

    // Step 3: Create compressed archive with selected format
    let format = match options.compression_format.to_lowercase().as_str() {
        "lzma2" | "lzma" | "xz" => velocity_core::extract::CompressionFormat::Lzma2,
        _ => velocity_core::extract::CompressionFormat::Zstd,
    };
    let compressed_data = velocity_core::extract::create_archive_with_format(
        &files, options.compression_level, format,
    )?;
    info!("Compressed payload: {} bytes ({:?} format)", compressed_data.len(), format);

    // Step 3.5: Encrypt payload if password is set
    let final_payload_data = if !manifest.install.password.is_empty() {
        info!("Encrypting payload with password");
        velocity_core::encryption::encrypt(&compressed_data, &manifest.install.password)
    } else {
        compressed_data
    };

    // Step 4: Serialize manifest to JSON
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| CompilerError::Other(format!("Failed to serialize manifest: {}", e)))?;

    // Step 5: Find or build the runtime binary
    let runtime_exe = find_or_build_runtime(options)?;

    // Step 6: Create the output directory
    if let Some(parent) = options.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Step 7: Assemble the installer
    velocity_core::payload::create_payload(
        &runtime_exe,
        &manifest_json,
        &final_payload_data,
        &options.output_path,
    )?;

    // Step 8: Set custom icon if specified
    if let Some(ref icon_path) = manifest.app.icon {
        let full_icon_path = options.project_dir.join(icon_path);
        if full_icon_path.exists() {
            info!("Setting installer icon: {}", full_icon_path.display());
            match velocity_core::pe_icon::set_exe_icon(&options.output_path, &full_icon_path) {
                Ok(()) => info!("Icon set successfully"),
                Err(e) => {
                    tracing::warn!("Failed to set icon: {}", e);
                }
            }
        } else {
            tracing::warn!("Icon file not found: {}", full_icon_path.display());
        }
    }

    // Step 9: Set version info
    let default_desc = format!("{} Installer", manifest.app.name);
    let description = manifest.app.description.as_deref()
        .unwrap_or(&default_desc);
    let _ = velocity_core::pe_icon::set_exe_version_info(
        &options.output_path,
        &manifest.app.version,
        Some(&manifest.app.publisher),
        Some(description),
        None,
    );

    let installer_size = std::fs::metadata(&options.output_path)?.len();

    info!(
        "Installer built: {} ({} bytes, {} files, {:.1}% compression)",
        options.output_path.display(),
        installer_size,
        files.len(),
        (1.0 - final_payload_data.len() as f64 / original_size as f64) * 100.0
    );

    Ok(BuildResult {
        installer_path: options.output_path.clone(),
        installer_size,
        file_count: files.len(),
        payload_size: final_payload_data.len() as u64,
        original_size,
    })
}

/// Find the pre-built runtime binary or build it.
fn find_or_build_runtime(options: &BuildOptions) -> Result<Vec<u8>> {
    // Check if a custom runtime path is specified
    if let Some(path) = &options.runtime_path {
        if path.exists() {
            info!("Using custom runtime: {}", path.display());
            return Ok(std::fs::read(path)?);
        }
        return Err(CompilerError::RuntimeNotFound(path.display().to_string()));
    }

    // Look for the runtime in common locations
    let candidates = [
        // In the workspace target directory (if building from source)
        options.project_dir.join("target/release/velocity-runtime.exe"),
        options.project_dir.join("target/debug/velocity-runtime.exe"),
        // Relative to the compiler crate
        PathBuf::from("target/release/velocity-runtime.exe"),
        PathBuf::from("target/debug/velocity-runtime.exe"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            info!("Found runtime: {}", candidate.display());
            return Ok(std::fs::read(candidate)?);
        }
    }

    // Try to build the runtime
    info!("Runtime not found, attempting to build...");
    build_runtime(&options.project_dir)?;

    // Try again after build
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(std::fs::read(candidate)?);
        }
    }

    Err(CompilerError::RuntimeNotFound(
        "Could not find or build velocity-runtime. Run `cargo build --release -p velocity-runtime` first.".to_string(),
    ))
}

/// Build the runtime binary using cargo.
fn build_runtime(project_dir: &Path) -> Result<()> {
    info!("Building velocity-runtime...");

    let status = std::process::Command::new("cargo")
        .args(["build", "--release", "-p", "velocity-runtime"])
        .current_dir(project_dir)
        .status()
        .map_err(|e| CompilerError::BuildFailed(format!("Failed to run cargo: {}", e)))?;

    if !status.success() {
        return Err(CompilerError::BuildFailed(
            "cargo build failed for velocity-runtime".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_options_default() {
        let opts = BuildOptions::default();
        assert_eq!(opts.compression_level, 3);
        assert!(!opts.quiet);
        assert!(opts.runtime_path.is_none());
    }

    #[test]
    fn test_manifest_parsing_for_build() {
        // Verify that a manifest can be parsed and has expected structure
        let toml_str = r#"
[app]
name = "Test App"
version = "1.0.0"
publisher = "Test"

[install]
default_dir = "{autopf}\\Test App"

[files]
source = ["bin/**/*"]

[shortcuts]
desktop = false
start_menu = false

[ui]
theme = "classic"
"#;
        let manifest: velocity_config::VelocityManifest =
            velocity_config::parse_manifest_str(toml_str).unwrap();
        assert_eq!(manifest.app.name, "Test App");
        assert_eq!(manifest.files.source.len(), 1);
        assert_eq!(manifest.ui.theme, "classic");
    }

    #[test]
    fn test_compression_roundtrip() {
        use velocity_core::extract;

        let temp_dir = std::env::temp_dir().join("velocity_compress_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test files
        std::fs::write(temp_dir.join("test.txt"), "Hello Velocity Installer!").unwrap();
        std::fs::write(temp_dir.join("data.bin"), vec![0u8, 1, 2, 3, 4, 5]).unwrap();

        let files = vec![
            (temp_dir.join("test.txt"), "test.txt".to_string()),
            (temp_dir.join("data.bin"), "data.bin".to_string()),
        ];

        // Create archive with various compression levels
        for level in [1, 3, 10, 19] {
            let archive = extract::create_archive(&files, level).unwrap();
            assert!(!archive.is_empty(), "Archive should not be empty at level {}", level);

            // Extract and verify
            let extract_dir = temp_dir.join(format!("extract_l{}", level));
            let extracted = extract::extract_archive(&archive, &extract_dir, None).unwrap();
            assert_eq!(extracted.len(), 2);

            let content = std::fs::read_to_string(extract_dir.join("test.txt")).unwrap();
            assert_eq!(content, "Hello Velocity Installer!");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
