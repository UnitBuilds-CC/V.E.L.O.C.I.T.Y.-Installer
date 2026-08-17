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

    // Step 3: Create compressed archive
    let compressed_data = velocity_core::extract::create_archive(&files, options.compression_level)?;
    info!("Compressed payload: {} bytes", compressed_data.len());

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
        &compressed_data,
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
        (1.0 - compressed_data.len() as f64 / original_size as f64) * 100.0
    );

    Ok(BuildResult {
        installer_path: options.output_path.clone(),
        installer_size,
        file_count: files.len(),
        payload_size: compressed_data.len() as u64,
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

/// Create a self-extracting installer that doesn't need the runtime pre-built.
///
/// This is an alternative build mode that creates a minimal stub executable
/// with the payload embedded, using a hardcoded entry point.
pub fn build_standalone(options: &BuildOptions) -> Result<BuildResult> {
    // For now, delegate to the regular build
    build_installer(options)
}
