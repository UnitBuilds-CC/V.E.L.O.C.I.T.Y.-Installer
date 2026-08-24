use crate::error::{ConfigError, Result};
use crate::manifest::*;
use std::path::Path;
use tracing::{debug, info};
use walkdir::WalkDir;

/// Auto-generate a `VelocityManifest` by scanning a project directory.
///
/// This attempts to detect:
/// - Application name and version from common project files
/// - Files to include from build output directories
/// - Application icon
/// - Executable to launch after install
pub fn auto_generate(project_dir: &Path) -> Result<VelocityManifest> {
    info!("Auto-generating manifest from: {}", project_dir.display());

    let app_info = detect_app_info(project_dir)?;
    let files = detect_files(project_dir)?;

    let manifest = VelocityManifest {
        app: AppConfig {
            name: app_info.name.clone(),
            version: app_info.version,
            publisher: app_info.publisher,
            icon: app_info.icon,
            url: None,
            id: None,
            license: None,
            description: None,
        },
        install: InstallConfig {
            default_dir: format!("{{autopf}}/{}", app_info.name),
            start_menu: Some(app_info.name.clone()),
            ..Default::default()
        },
        files,
        shortcuts: ShortcutConfig {
            desktop: true,
            start_menu: true,
            ..Default::default()
        },
        registry: Vec::new(),
        uninstall: UninstallConfig::default(),
        ui: UiConfig::default(),
        pages: Vec::new(),
        scripts: ScriptsConfig::default(),
        env_vars: Vec::new(),
        services: Vec::new(),
        file_associations: Vec::new(),
        dependencies: Vec::new(),
        bundled_apps: Vec::new(),
        components: Vec::new(),
        localization: crate::LocalizationConfig::default(),
        fetch: None,
    };

    debug!("Auto-generated manifest: {:?}", manifest.app.name);
    Ok(manifest)
}

/// Detected application information.
struct DetectedAppInfo {
    name: String,
    version: String,
    publisher: String,
    icon: Option<std::path::PathBuf>,
}

/// Detect application info from project files.
fn detect_app_info(project_dir: &Path) -> Result<DetectedAppInfo> {
    let mut name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "My Application".to_string());

    let mut version = "1.0.0".to_string();
    let mut publisher = String::new();
    let mut icon = None;

    // Try to read from Cargo.toml
    let cargo_toml = project_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if let Ok(value) = content.parse::<toml::Value>() {
                if let Some(package) = value.get("package") {
                    if let Some(pkg_name) = package.get("name").and_then(|v| v.as_str()) {
                        name = to_title_case(pkg_name);
                    }
                    if let Some(pkg_version) = package.get("version").and_then(|v| v.as_str()) {
                        version = pkg_version.to_string();
                    }
                    if let Some(authors) = package.get("authors").and_then(|v| v.as_array()) {
                        if let Some(first_author) = authors.first().and_then(|v| v.as_str()) {
                            publisher = first_author
                                .split('<')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_string();
                        }
                    }
                }
            }
        }
    }

    // Try to read from package.json
    let package_json = project_dir.join("package.json");
    if package_json.exists() && publisher.is_empty() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(value) = content.parse::<serde_json::Value>() {
                if let Some(pkg_name) = value.get("name").and_then(|v| v.as_str()) {
                    name = to_title_case(pkg_name);
                }
                if let Some(pkg_version) = value.get("version").and_then(|v| v.as_str()) {
                    version = pkg_version.to_string();
                }
                if let Some(pkg_author) = value.get("author").and_then(|v| v.as_str()) {
                    publisher = pkg_author.to_string();
                }
            }
        }
    }

    // Look for an icon file
    let icon_candidates = [
        "assets/icon.ico",
        "assets/app.ico",
        "icon.ico",
        "app.ico",
        "resources/icon.ico",
    ];
    for candidate in &icon_candidates {
        let path = project_dir.join(candidate);
        if path.exists() {
            icon = Some(path);
            break;
        }
    }

    Ok(DetectedAppInfo {
        name,
        version,
        publisher,
        icon,
    })
}

/// Detect files to include from common build output directories.
fn detect_files(project_dir: &Path) -> Result<FilesConfig> {
    let mut source_patterns = Vec::new();
    let mut exclude_patterns = Vec::new();

    // Common build output directories
    let build_dirs = [
        "target/release",
        "target/debug",
        "build",
        "dist",
        "output",
        "bin/Release",
        "bin/Debug",
        "out",
        "publish",
    ];

    for dir in &build_dirs {
        let path = project_dir.join(dir);
        if path.exists() && path.is_dir() {
            source_patterns.push(format!("./{}/**", dir));
            debug!("Detected build output: {}", dir);
            break; // Use the first one found
        }
    }

    // If no build directory found, include everything in current dir
    if source_patterns.is_empty() {
        source_patterns.push("./**".to_string());
    }

    // Common exclusions
    let common_excludes = [
        "*.pdb",           // Debug symbols
        "*.tmp",           // Temp files
        "*.log",           // Log files
        ".git/**",         // Git data
        "node_modules/**", // Node modules
        "*.rs.bk",         // Rust backup files
    ];

    for exc in &common_excludes {
        exclude_patterns.push(exc.to_string());
    }

    Ok(FilesConfig {
        source: source_patterns,
        base_dir: Some(project_dir.to_path_buf()),
        mappings: Vec::new(),
        exclude: exclude_patterns,
        compression: Default::default(),
    })
}

/// Convert a kebab-case or snake_case name to Title Case.
pub fn to_title_case(input: &str) -> String {
    input
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{}{}", upper, chars.collect::<String>())
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collect all files matching the manifest's file configuration.
pub fn collect_files(
    manifest: &VelocityManifest,
    base_dir: &Path,
) -> Result<Vec<(std::path::PathBuf, String)>> {
    let mut files = Vec::new();

    let search_base = manifest
        .files
        .base_dir
        .as_ref()
        .map(|p| base_dir.join(p))
        .unwrap_or_else(|| base_dir.to_path_buf());

    for pattern in &manifest.files.source {
        let full_pattern = if Path::new(pattern).is_absolute() {
            pattern.clone()
        } else {
            search_base.join(pattern).to_string_lossy().to_string()
        };
        // Normalize path separators for the glob crate (requires forward slashes)
        let full_pattern = full_pattern.replace('\\', "/");

        let paths = glob::glob(&full_pattern).map_err(ConfigError::GlobError)?;

        for entry in paths {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        // Calculate relative path from search base
                        let relative = path
                            .strip_prefix(&search_base)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();

                        // Check exclusions
                        let excluded = manifest.files.exclude.iter().any(|exc| {
                            glob::Pattern::new(exc)
                                .map(|p| p.matches(&relative))
                                .unwrap_or(false)
                        });

                        if !excluded {
                            files.push((path, relative));
                        }
                    }
                }
                Err(e) => {
                    debug!("Glob error: {}", e);
                }
            }
        }
    }

    // Add explicit mappings
    for mapping in &manifest.files.mappings {
        let source_path = search_base.join(&mapping.source);
        if source_path.is_file() {
            files.push((source_path, mapping.dest.clone()));
        } else if source_path.is_dir() {
            for entry in WalkDir::new(&source_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(&source_path)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .to_string();
                    let dest = format!("{}/{}", mapping.dest, relative);
                    files.push((entry.path().to_path_buf(), dest));
                }
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_title_case() {
        assert_eq!(to_title_case("my-app"), "My App");
        assert_eq!(to_title_case("my_app"), "My App");
        assert_eq!(to_title_case("myapp"), "Myapp");
        assert_eq!(to_title_case("my-cool-app"), "My Cool App");
    }
}
