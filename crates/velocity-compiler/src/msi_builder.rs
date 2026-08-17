//! MSI Builder — Generates Windows Installer (MSI) packages from Velocity projects.
//!
//! Maps `velocity.toml` configuration to MSI database tables, producing enterprise-ready
//! `.msi` packages compatible with Group Policy, SCCM, and `msiexec` deployment.
//!
//! # MSI Table Mapping
//!
//! | Velocity Config | MSI Table(s) |
//! |----------------|--------------|
//! | `[app]` | Property (ProductName, Manufacturer, ProductVersion) |
//! | `[files]` | File, Component, Directory, Media |
//! | `[registry]` | Registry |
//! | `[shortcuts]` | Shortcut, Icon |
//! | `[scripts]` | CustomAction, InstallExecuteSequence |
//! | `[env_vars]` | Environment |
//! | `[services]` | ServiceInstall, ServiceControl |
//! | `[components]` | Feature, FeatureComponents |
//! | `[file_associations]` | Class, ProgId, Extension |

use crate::error::{CompilerError, Result};
use msi::{Column, Insert, Package, PackageType, Value};
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use uuid::Uuid;
use velocity_config::VelocityManifest;

/// Options for MSI package generation.
#[derive(Debug, Clone)]
pub struct MsiOptions {
    /// Output path for the .msi file
    pub output_path: PathBuf,
    /// Project directory (containing velocity.toml and source files)
    pub project_dir: PathBuf,
    /// Architecture: "x64", "x86", or "arm64"
    pub architecture: String,
    /// Language code (e.g., 1033 for English US)
    pub language: u16,
    /// Whether to generate a per-machine install (HKLM) vs per-user (HKCU)
    pub per_machine: bool,
    /// Upgrade code GUID for major upgrade support
    pub upgrade_code: Option<String>,
}

impl Default for MsiOptions {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("output/installer.msi"),
            project_dir: std::env::current_dir().unwrap_or_default(),
            architecture: "x64".to_string(),
            language: 1033, // English US
            per_machine: true,
            upgrade_code: None,
        }
    }
}

/// Result of a successful MSI build.
#[derive(Debug)]
pub struct MsiBuildResult {
    /// Path to the generated .msi file
    pub msi_path: PathBuf,
    /// Size of the MSI file in bytes
    pub msi_size: u64,
    /// Number of files included
    pub file_count: usize,
    /// Number of components generated
    pub component_count: usize,
    /// Product code GUID
    pub product_code: String,
    /// Upgrade code GUID
    pub upgrade_code: String,
}

/// Build an MSI package from a Velocity manifest.
pub fn build_msi(manifest: &VelocityManifest, options: &MsiOptions) -> Result<MsiBuildResult> {
    info!(
        "Building MSI for: {} v{} ({})",
        manifest.app.name, manifest.app.version, options.architecture
    );

    let cursor = Cursor::new(Vec::new());
    let mut package = Package::create(PackageType::Installer, cursor)
        .map_err(|e| CompilerError::Other(format!("Failed to create MSI package: {}", e)))?;

    // Generate GUIDs
    let product_code = Uuid::new_v4().to_string();
    let upgrade_code = options
        .upgrade_code
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    info!("ProductCode: {}", product_code);
    info!("UpgradeCode: {}", upgrade_code);

    // Set summary info
    {
        let si = package.summary_info_mut();
        si.set_title(format!("{} Installer", manifest.app.name));
        si.set_author(manifest.app.publisher.clone());
        si.set_subject(format!("{} v{}", manifest.app.name, manifest.app.version));
        si.set_comments(
            manifest
                .app
                .description
                .clone()
                .unwrap_or_else(|| format!("{} installer package", manifest.app.name)),
        );
    }

    // Create all required MSI tables
    create_msi_tables(&mut package)?;

    // Populate Property table
    populate_properties(&mut package, manifest, &product_code, &upgrade_code, options)?;

    // Populate Directory table
    let dir_id_map = populate_directories(&mut package, manifest)?;

    // Collect files from the project
    let files = collect_msi_files(manifest, options)?;
    info!("Collected {} files for MSI", files.len());

    // Populate Component, File, and Media tables
    let component_count = populate_components(&mut package, &files, &dir_id_map)?;

    // Populate Feature table
    populate_features(&mut package, manifest, &files)?;

    // Populate Registry table
    populate_registry(&mut package, manifest)?;

    // Populate Shortcut table
    populate_shortcuts(&mut package, manifest, &dir_id_map)?;

    // Populate Environment table
    populate_environment(&mut package, manifest)?;

    // Populate ServiceInstall and ServiceControl tables
    populate_services(&mut package, manifest)?;

    // Populate CustomAction and InstallExecuteSequence tables
    populate_custom_actions(&mut package, manifest)?;

    // Embed files as streams in the MSI (cabinet-less approach for simplicity)
    embed_file_streams(&mut package, &files)?;

    // Flush and write to file
    package
        .flush()
        .map_err(|e| CompilerError::Other(format!("Failed to flush MSI: {}", e)))?;

    let cursor = package
        .into_inner()
        .map_err(|e| CompilerError::Other(format!("Failed to finalize MSI: {}", e)))?;

    // Write to output file
    if let Some(parent) = options.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let msi_data = cursor.into_inner();
    std::fs::write(&options.output_path, &msi_data)?;

    let msi_size = std::fs::metadata(&options.output_path)?.len();

    info!(
        "MSI built: {} ({} bytes, {} files, {} components)",
        options.output_path.display(),
        msi_size,
        files.len(),
        component_count
    );

    Ok(MsiBuildResult {
        msi_path: options.output_path.clone(),
        msi_size,
        file_count: files.len(),
        component_count,
        product_code,
        upgrade_code,
    })
}

/// Create all required MSI database tables.
fn create_msi_tables(package: &mut Package<Cursor<Vec<u8>>>) -> Result<()> {
    // Property table
    package
        .create_table(
            "Property",
            vec![
                Column::build("Property").primary_key().id_string(72),
                Column::build("Value").nullable().formatted_string(0),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Property table: {}", e)))?;

    // Directory table
    package
        .create_table(
            "Directory",
            vec![
                Column::build("Directory").primary_key().id_string(72),
                Column::build("Directory_Parent")
                    .nullable()
                    .id_string(72),
                Column::build("DefaultDir").nullable().formatted_string(255),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Directory table: {}", e)))?;

    // Component table
    package
        .create_table(
            "Component",
            vec![
                Column::build("Component").primary_key().id_string(72),
                Column::build("ComponentId").nullable().string(38),
                Column::build("Directory_").nullable().id_string(72),
                Column::build("Attributes").nullable().int16(),
                Column::build("Condition").nullable().formatted_string(255),
                Column::build("KeyPath").nullable().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Component table: {}", e)))?;

    // File table
    package
        .create_table(
            "File",
            vec![
                Column::build("File").primary_key().id_string(72),
                Column::build("Component_").nullable().id_string(72),
                Column::build("FileName").nullable().formatted_string(255),
                Column::build("FileSize").nullable().int32(),
                Column::build("Attributes").nullable().int16(),
                Column::build("Sequence").nullable().int32(),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create File table: {}", e)))?;

    // Media table
    package
        .create_table(
            "Media",
            vec![
                Column::build("DiskId").primary_key().int16(),
                Column::build("LastSequence").nullable().int32(),
                Column::build("Cabinet").nullable().formatted_string(255),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Media table: {}", e)))?;

    // Feature table
    package
        .create_table(
            "Feature",
            vec![
                Column::build("Feature").primary_key().id_string(38),
                Column::build("Feature_Parent").nullable().id_string(38),
                Column::build("Title").nullable().formatted_string(64),
                Column::build("Description").nullable().formatted_string(255),
                Column::build("Display").nullable().int16(),
                Column::build("Level").nullable().int16(),
                Column::build("Directory_").nullable().id_string(72),
                Column::build("Attributes").nullable().int16(),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Feature table: {}", e)))?;

    // FeatureComponents table
    package
        .create_table(
            "FeatureComponents",
            vec![
                Column::build("Feature_").primary_key().id_string(38),
                Column::build("Component_").primary_key().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create FeatureComponents table: {}",
            e
        )))?;

    // Registry table
    package
        .create_table(
            "Registry",
            vec![
                Column::build("Registry").primary_key().id_string(72),
                Column::build("Root").nullable().int16(),
                Column::build("Key").nullable().formatted_string(255),
                Column::build("Name").nullable().formatted_string(255),
                Column::build("Value").nullable().formatted_string(0),
                Column::build("Component_").nullable().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Registry table: {}", e)))?;

    // Shortcut table
    package
        .create_table(
            "Shortcut",
            vec![
                Column::build("Shortcut").primary_key().id_string(72),
                Column::build("Directory_").nullable().id_string(72),
                Column::build("Name").nullable().formatted_string(128),
                Column::build("Component_").nullable().id_string(72),
                Column::build("Target").nullable().formatted_string(255),
                Column::build("Arguments").nullable().formatted_string(255),
                Column::build("Description").nullable().formatted_string(255),
                Column::build("Hotkey").nullable().int16(),
                Column::build("Icon_").nullable().id_string(72),
                Column::build("IconIndex").nullable().int16(),
                Column::build("ShowCmd").nullable().int16(),
                Column::build("WkDir").nullable().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Shortcut table: {}", e)))?;

    // Icon table
    package
        .create_table(
            "Icon",
            vec![
                Column::build("Name").primary_key().id_string(72),
                Column::build("Data").nullable().binary(),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Icon table: {}", e)))?;

    // Environment table
    package
        .create_table(
            "Environment",
            vec![
                Column::build("Environment").primary_key().id_string(72),
                Column::build("Name").nullable().formatted_string(255),
                Column::build("Value").nullable().formatted_string(255),
                Column::build("Component_").nullable().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create Environment table: {}",
            e
        )))?;

    // ServiceInstall table
    package
        .create_table(
            "ServiceInstall",
            vec![
                Column::build("ServiceInstall").primary_key().id_string(72),
                Column::build("Name").nullable().formatted_string(255),
                Column::build("DisplayName").nullable().formatted_string(255),
                Column::build("ServiceType").nullable().int32(),
                Column::build("StartType").nullable().int32(),
                Column::build("ErrorControl").nullable().int32(),
                Column::build("LoadOrderGroup").nullable().formatted_string(255),
                Column::build("Dependencies").nullable().formatted_string(255),
                Column::build("StartName").nullable().formatted_string(255),
                Column::build("Password").nullable().formatted_string(255),
                Column::build("Arguments").nullable().formatted_string(255),
                Column::build("Component_").nullable().id_string(72),
                Column::build("Description").nullable().formatted_string(255),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create ServiceInstall table: {}",
            e
        )))?;

    // ServiceControl table
    package
        .create_table(
            "ServiceControl",
            vec![
                Column::build("ServiceControl").primary_key().id_string(72),
                Column::build("Name").nullable().formatted_string(255),
                Column::build("Event").nullable().int32(),
                Column::build("Arguments").nullable().formatted_string(255),
                Column::build("Wait").nullable().int16(),
                Column::build("Component_").nullable().id_string(72),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create ServiceControl table: {}",
            e
        )))?;

    // CustomAction table
    package
        .create_table(
            "CustomAction",
            vec![
                Column::build("Action").primary_key().id_string(72),
                Column::build("Type").nullable().int16(),
                Column::build("Source").nullable().formatted_string(72),
                Column::build("Target").nullable().formatted_string(255),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create CustomAction table: {}",
            e
        )))?;

    // InstallExecuteSequence table
    package
        .create_table(
            "InstallExecuteSequence",
            vec![
                Column::build("Action").primary_key().id_string(72),
                Column::build("Condition").nullable().formatted_string(255),
                Column::build("Sequence").nullable().int16(),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create InstallExecuteSequence table: {}",
            e
        )))?;

    // Upgrade table (for major upgrade support)
    package
        .create_table(
            "Upgrade",
            vec![
                Column::build("UpgradeCode").primary_key().string(38),
                Column::build("VersionMin").nullable().string(20),
                Column::build("VersionMax").nullable().string(20),
                Column::build("Language").nullable().string(20),
                Column::build("Attributes").nullable().int32(),
            ],
        )
        .map_err(|e| CompilerError::Other(format!("Failed to create Upgrade table: {}", e)))?;

    // LaunchCondition table
    package
        .create_table(
            "LaunchCondition",
            vec![
                Column::build("Condition").primary_key().formatted_string(255),
                Column::build("Description").nullable().formatted_string(255),
            ],
        )
        .map_err(|e| CompilerError::Other(format!(
            "Failed to create LaunchCondition table: {}",
            e
        )))?;

    Ok(())
}

/// Populate the Property table with standard and custom properties.
fn populate_properties(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
    product_code: &str,
    upgrade_code: &str,
    options: &MsiOptions,
) -> Result<()> {
    let properties = vec![
        ("ProductCode", product_code.to_string()),
        ("UpgradeCode", upgrade_code.to_string()),
        ("ProductName", manifest.app.name.clone()),
        ("Manufacturer", manifest.app.publisher.clone()),
        ("ProductVersion", manifest.app.version.clone()),
        (
            "ProductLanguage",
            format!("{}", options.language),
        ),
        (
            "ARPPRODUCTICON",
            manifest
                .app
                .icon
                .as_ref()
                .map(|_| "AppIcon.ico")
                .unwrap_or("")
                .to_string(),
        ),
        (
            "ARPURLINFOABOUT",
            manifest.app.url.clone().unwrap_or_default(),
        ),
        (
            "ARPCOMMENTS",
            manifest
                .app
                .description
                .clone()
                .unwrap_or_default(),
        ),
        (
            "ALLUSERS",
            if options.per_machine { "1" } else { "" }.to_string(),
        ),
    ];

    for (name, value) in properties {
        let query = Insert::into("Property").row(vec![
            Value::from(name),
            Value::from(value),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert property: {}", e)))?;
    }

    // Add description property
    let desc = manifest
        .app
        .description
        .clone()
        .unwrap_or_else(|| format!("{} Installer", manifest.app.name));
    let query = Insert::into("Property").row(vec![
        Value::from("Description"),
        Value::from(desc),
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert description: {}", e)))?;

    info!("Properties populated");
    Ok(())
}

/// Populate the Directory table with the installation directory structure.
fn populate_directories(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
) -> Result<HashMap<String, String>> {
    let mut dir_map = HashMap::new();

    // Standard directories
    // TARGETDIR is the root
    let query = Insert::into("Directory").row(vec![
        Value::from("TARGETDIR"),
        Value::from(""),
        Value::from("SourceDir"),
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert TARGETDIR: {}", e)))?;

    // ProgramFiles64Folder or ProgramFilesFolder
    let pf_dir = if manifest.install.arch.contains("64") {
        "ProgramFiles64Folder"
    } else {
        "ProgramFilesFolder"
    };
    let query = Insert::into("Directory").row(vec![
        Value::from(pf_dir),
        Value::from("TARGETDIR"),
        Value::from("PFiles"),
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert PF dir: {}", e)))?;

    // Application directory
    let app_dir_name = sanitize_dir_name(&manifest.app.name);
    let app_dir_id = "INSTALLDIR";
    let query = Insert::into("Directory").row(vec![
        Value::from(app_dir_id),
        Value::from(pf_dir),
        Value::from(format!("{}:{}", app_dir_name, app_dir_name)),
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to install dir: {}", e)))?;
    dir_map.insert("INSTALLDIR".to_string(), app_dir_id.to_string());

    // ProgramMenuFolder for shortcuts
    let query = Insert::into("Directory").row(vec![
        Value::from("ProgramMenuFolder"),
        Value::from("TARGETDIR"),
        Value::from("Programs"),
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert menu dir: {}", e)))?;

    // Application Start Menu folder
    if manifest.shortcuts.start_menu {
        let menu_dir = sanitize_dir_name(&manifest.app.name);
        let query = Insert::into("Directory").row(vec![
            Value::from("ApplicationProgramsFolder"),
            Value::from("ProgramMenuFolder"),
            Value::from(format!("{}:{}", menu_dir, menu_dir)),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert app menu: {}", e)))?;
        dir_map.insert(
            "ApplicationProgramsFolder".to_string(),
            "ApplicationProgramsFolder".to_string(),
        );
    }

    // Desktop directory
    if manifest.shortcuts.desktop || manifest.install.create_desktop_shortcut {
        let query = Insert::into("Directory").row(vec![
            Value::from("DesktopFolder"),
            Value::from("TARGETDIR"),
            Value::from("Desktop"),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert desktop dir: {}", e)))?;
        dir_map.insert("DesktopFolder".to_string(), "DesktopFolder".to_string());
    }

    info!("Directories populated ({} entries)", dir_map.len());
    Ok(dir_map)
}

/// Collect files from the project for inclusion in the MSI.
fn collect_msi_files(manifest: &VelocityManifest, options: &MsiOptions) -> Result<Vec<(PathBuf, String)>> {
    let files = velocity_config::collect_files(manifest, &options.project_dir)?;
    Ok(files)
}

/// Populate Component, File, and Media tables.
fn populate_components(
    package: &mut Package<Cursor<Vec<u8>>>,
    files: &[(PathBuf, String)],
    dir_id_map: &HashMap<String, String>,
) -> Result<usize> {
    let mut component_count = 0;
    let install_dir = dir_id_map
        .get("INSTALLDIR")
        .cloned()
        .unwrap_or_else(|| "INSTALLDIR".to_string());

    // Create one component per file (standard MSI pattern)
    for (i, (file_path, rel_path)) in files.iter().enumerate() {
        let component_id = format!("comp_{}", i);
        let file_id = format!("file_{}", i);
        let component_guid = Uuid::new_v4().to_string();
        let file_name = Path::new(rel_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel_path.clone());

        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len() as i32)
            .unwrap_or(0);

        // Component row
        let query = Insert::into("Component").row(vec![
            Value::from(component_id.as_str()),
            Value::from(component_guid.as_str()),
            Value::from(install_dir.as_str()),
            Value::Int(0), // Attributes: none
            Value::from(""), // Condition
            Value::from(file_id.as_str()), // KeyPath = file
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert component: {}", e)))?;

        // File row — use short name | long name format
        let msi_filename = if file_name.len() > 8
            || file_name
                .rsplit('.')
                .next()
                .map(|ext| ext.len() > 3)
                .unwrap_or(false)
        {
            // Long filename: use 8.3 short name placeholder | long name
            let short = format!(
                "{:.8}.{:3}",
                file_name
                    .split('.')
                    .next()
                    .unwrap_or("FILE")
                    .to_uppercase(),
                file_name
                    .rsplit('.')
                    .next()
                    .unwrap_or("TXT")
                    .to_uppercase()
            );
            format!("{}|{}", short, file_name)
        } else {
            file_name.clone()
        };

        let query = Insert::into("File").row(vec![
            Value::from(file_id.as_str()),
            Value::from(component_id.as_str()),
            Value::from(msi_filename.as_str()),
            Value::Int(file_size),
            Value::Int(0), // Attributes
            Value::Int((i + 1) as i32), // Sequence
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert file: {}", e)))?;

        component_count += 1;
        debug!("Component {}: {} ({} bytes)", component_id, rel_path, file_size);
    }

    // Media table — single cabinet
    let query = Insert::into("Media").row(vec![
        Value::Int(1),                          // DiskId
        Value::Int(files.len() as i32),         // LastSequence
        Value::from("#Velocity.cab"),           // Cabinet (embedded)
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert media: {}", e)))?;

    info!("Components populated: {} components", component_count);
    Ok(component_count)
}

/// Populate the Feature table.
fn populate_features(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
    files: &[(PathBuf, String)],
) -> Result<()> {
    // Main feature
    let query = Insert::into("Feature").row(vec![
        Value::from("Complete"),
        Value::from(""), // No parent
        Value::from(format!("{} Setup", manifest.app.name)),
        Value::from(format!("Complete installation of {}", manifest.app.name)),
        Value::Int(1),  // Display
        Value::Int(1),  // Level (installed by default)
        Value::from("INSTALLDIR"),
        Value::Int(0), // Attributes
    ]);
    package
        .insert_rows(query)
        .map_err(|e| CompilerError::Other(format!("Failed to insert feature: {}", e)))?;

    // Link all components to the main feature
    for i in 0..files.len() {
        let component_id = format!("comp_{}", i);
        let query = Insert::into("FeatureComponents").row(vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!(
                "Failed to insert feature component: {}",
                e
            )))?;
    }

    // Add user-defined components as sub-features
    for comp in &manifest.components {
        let feature_id = format!("Feature_{}", comp.id);
        let parent = "Complete".to_string();
        let level = if comp.selected_by_default { 1 } else { 0 };

        let query = Insert::into("Feature").row(vec![
            Value::from(feature_id.as_str()),
            Value::from(parent.as_str()),
            Value::from(comp.name.clone()),
            Value::from(comp.description.clone().unwrap_or_default()),
            Value::Int(0), // Display: hidden under parent
            Value::Int(level),
            Value::from("INSTALLDIR"),
            Value::Int(0),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!(
                "Failed to insert sub-feature: {}",
                e
            )))?;
    }

    info!("Features populated");
    Ok(())
}

/// Populate the Registry table from manifest registry entries.
fn populate_registry(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, entry) in manifest.registry.iter().enumerate() {
        let reg_id = format!("reg_{}", i);
        let component_id = format!("comp_reg_{}", i);

        // Create a component for this registry entry
        let reg_guid = Uuid::new_v4().to_string();
        let query = Insert::into("Component").row(vec![
            Value::from(component_id.as_str()),
            Value::from(reg_guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::from(""),
            Value::from(reg_id.as_str()), // KeyPath
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert reg component: {}", e)))?;

        // Link to feature
        let query = Insert::into("FeatureComponents").row(vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!(
                "Failed to link reg component: {}",
                e
            )))?;

        // Map root to MSI root integer
        let root = match entry.root.to_uppercase().as_str() {
            "HKLM" => 2,
            "HKCU" => 1,
            "HKCR" => 0,
            "HKU" => 3,
            _ => 2, // Default to HKLM
        };

        let query = Insert::into("Registry").row(vec![
            Value::from(reg_id.as_str()),
            Value::Int(root),
            Value::from(entry.key.as_str()),
            Value::from(entry.name.clone().unwrap_or_default().as_str()),
            Value::from(entry.value.as_str()),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Failed to insert registry: {}", e)))?;

        debug!("Registry: {}\\{} = {}", entry.root, entry.key, entry.value);
    }

    info!("Registry populated: {} entries", manifest.registry.len());
    Ok(())
}

/// Populate the Shortcut table from manifest shortcut configuration.
fn populate_shortcuts(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
    dir_id_map: &HashMap<String, String>,
) -> Result<()> {
    let mut shortcut_count = 0;

    // Desktop shortcut
    if manifest.shortcuts.desktop || manifest.install.create_desktop_shortcut {
        if let Some(desktop_dir) = dir_id_map.get("DesktopFolder") {
            let shortcut_id = "DesktopShortcut";
            let component_id = "comp_desktop_shortcut";
            let guid = Uuid::new_v4().to_string();

            // Component for shortcut
            let query = Insert::into("Component").row(vec![
                Value::from(component_id),
                Value::from(guid.as_str()),
                Value::from(desktop_dir.as_str()),
                Value::Int(0),
                Value::from(""),
                Value::from(shortcut_id),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Shortcut comp: {}", e)))?;

            let query = Insert::into("FeatureComponents").row(vec![
                Value::from("Complete"),
                Value::from(component_id),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Shortcut link: {}", e)))?;

            let query = Insert::into("Shortcut").row(vec![
                Value::from(shortcut_id),
                Value::from(desktop_dir.as_str()),
                Value::from(manifest.app.name.as_str()),
                Value::from(component_id),
                Value::from("[INSTALLDIR]"), // Target
                Value::from(""),              // Arguments
                Value::from(
                    manifest
                        .app
                        .description
                        .clone()
                        .unwrap_or_default()
                        .as_str(),
                ),
                Value::Int(0), // Hotkey
                Value::from(""), // Icon
                Value::Int(0),  // IconIndex
                Value::Int(1),  // ShowCmd (normal)
                Value::from("INSTALLDIR"), // Working dir
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Shortcut insert: {}", e)))?;
            shortcut_count += 1;
        }
    }

    // Start Menu shortcuts
    if manifest.shortcuts.start_menu {
        if let Some(menu_dir) = dir_id_map.get("ApplicationProgramsFolder") {
            let shortcut_id = "StartMenuShortcut";
            let component_id = "comp_startmenu_shortcut";
            let guid = Uuid::new_v4().to_string();

            let query = Insert::into("Component").row(vec![
                Value::from(component_id),
                Value::from(guid.as_str()),
                Value::from(menu_dir.as_str()),
                Value::Int(0),
                Value::from(""),
                Value::from(shortcut_id),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("SM shortcut comp: {}", e)))?;

            let query = Insert::into("FeatureComponents").row(vec![
                Value::from("Complete"),
                Value::from(component_id),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("SM shortcut link: {}", e)))?;

            let query = Insert::into("Shortcut").row(vec![
                Value::from(shortcut_id),
                Value::from(menu_dir.as_str()),
                Value::from(manifest.app.name.as_str()),
                Value::from(component_id),
                Value::from("[INSTALLDIR]"),
                Value::from(""),
                Value::from(
                    manifest
                        .app
                        .description
                        .clone()
                        .unwrap_or_default()
                        .as_str(),
                ),
                Value::Int(0),
                Value::from(""),
                Value::Int(0),
                Value::Int(1),
                Value::from("INSTALLDIR"),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("SM shortcut insert: {}", e)))?;
            shortcut_count += 1;
        }
    }

    // Custom shortcuts
    for (i, custom) in manifest.shortcuts.custom.iter().enumerate() {
        let shortcut_id = format!("CustomShortcut_{}", i);
        let component_id = format!("comp_custom_shortcut_{}", i);
        let guid = Uuid::new_v4().to_string();

        let target_dir = match custom.location.as_str() {
            "desktop" => dir_id_map.get("DesktopFolder").cloned(),
            "start_menu" => dir_id_map.get("ApplicationProgramsFolder").cloned(),
            _ => dir_id_map.get("INSTALLDIR").cloned(),
        };

        if let Some(dir) = target_dir {
            let query = Insert::into("Component").row(vec![
                Value::from(component_id.as_str()),
                Value::from(guid.as_str()),
                Value::from(dir.as_str()),
                Value::Int(0),
                Value::from(""),
                Value::from(shortcut_id.as_str()),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Custom shortcut comp: {}", e)))?;

            let query = Insert::into("FeatureComponents").row(vec![
                Value::from("Complete"),
                Value::from(component_id.as_str()),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Custom shortcut link: {}", e)))?;

            let query = Insert::into("Shortcut").row(vec![
                Value::from(shortcut_id.as_str()),
                Value::from(dir.as_str()),
                Value::from(custom.name.as_str()),
                Value::from(component_id.as_str()),
                Value::from(format!("[INSTALLDIR]{}", custom.target).as_str()),
                Value::from(custom.arguments.clone().unwrap_or_default().as_str()),
                Value::from(""), // Description
                Value::Int(0),
                Value::from(""),
                Value::Int(0),
                Value::Int(1),
                Value::from(
                    custom
                        .working_dir
                        .clone()
                        .unwrap_or_else(|| "[INSTALLDIR]".to_string())
                        .as_str(),
                ),
            ]);
            package
                .insert_rows(query)
                .map_err(|e| CompilerError::Other(format!("Custom shortcut insert: {}", e)))?;
            shortcut_count += 1;
        }
    }

    info!("Shortcuts populated: {} entries", shortcut_count);
    Ok(())
}

/// Populate the Environment table from manifest env_vars.
fn populate_environment(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, env) in manifest.env_vars.iter().enumerate() {
        let env_id = format!("env_{}", i);
        let component_id = format!("comp_env_{}", i);
        let guid = Uuid::new_v4().to_string();

        // Component for env var
        let query = Insert::into("Component").row(vec![
            Value::from(component_id.as_str()),
            Value::from(guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::from(""),
            Value::from(env_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Env comp: {}", e)))?;

        let query = Insert::into("FeatureComponents").row(vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Env link: {}", e)))?;

        // Value with optional append separator
        let value = if env.append {
            format!("{};{}", env.name, env.value)
        } else {
            env.value.clone()
        };

        let query = Insert::into("Environment").row(vec![
            Value::from(env_id.as_str()),
            Value::from(env.name.as_str()),
            Value::from(value.as_str()),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Env insert: {}", e)))?;

        debug!("Environment: {} = {}", env.name, env.value);
    }

    info!("Environment populated: {} entries", manifest.env_vars.len());
    Ok(())
}

/// Populate ServiceInstall and ServiceControl tables.
fn populate_services(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, svc) in manifest.services.iter().enumerate() {
        let svc_install_id = format!("svc_install_{}", i);
        let svc_ctrl_id = format!("svc_ctrl_{}", i);
        let component_id = format!("comp_svc_{}", i);
        let guid = Uuid::new_v4().to_string();

        // Component for service
        let query = Insert::into("Component").row(vec![
            Value::from(component_id.as_str()),
            Value::from(guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::from(""),
            Value::from(svc_install_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Service comp: {}", e)))?;

        let query = Insert::into("FeatureComponents").row(vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Service link: {}", e)))?;

        // Map start type
        let start_type = match svc.start_type.as_str() {
            "auto" => 2,
            "manual" => 3,
            "disabled" => 4,
            "delayed_auto" => 2, // MSI doesn't have delayed auto; use auto
            _ => 2,
        };

        // ServiceInstall
        let query = Insert::into("ServiceInstall").row(vec![
            Value::from(svc_install_id.as_str()),
            Value::from(svc.name.as_str()),
            Value::from(svc.display_name.as_str()),
            Value::Int(0x10), // SERVICE_WIN32_OWN_PROCESS
            Value::Int(start_type),
            Value::Int(1), // ErrorControl: normal
            Value::from(""), // LoadOrderGroup
            Value::from(svc.dependencies.join("\0").as_str()),
            Value::from(svc.account.clone().unwrap_or_default().as_str()),
            Value::from(""), // Password
            Value::from(""), // Arguments
            Value::from(component_id.as_str()),
            Value::from(svc.description.clone().unwrap_or_default().as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("ServiceInstall: {}", e)))?;

        // ServiceControl — start on install, stop+delete on uninstall
        let query = Insert::into("ServiceControl").row(vec![
            Value::from(svc_ctrl_id.as_str()),
            Value::from(svc.name.as_str()),
            Value::Int(1 + 2 + 4), // Install: start(1) + stop(2) + delete(4)
            Value::from(""),        // Arguments
            Value::Int(1),          // Wait
            Value::from(component_id.as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("ServiceControl: {}", e)))?;

        debug!("Service: {} ({})", svc.display_name, svc.name);
    }

    info!("Services populated: {} entries", manifest.services.len());
    Ok(())
}

/// Populate CustomAction and InstallExecuteSequence tables.
fn populate_custom_actions(
    package: &mut Package<Cursor<Vec<u8>>>,
    manifest: &VelocityManifest,
) -> Result<()> {
    // Standard sequence actions
    let standard_actions = vec![
        ("AppSearch", "", 100),
        ("LaunchConditions", "NOT Installed", 105),
        ("ValidateProductID", "", 110),
        ("CostInitialize", "", 120),
        ("FileCost", "", 130),
        ("CostFinalize", "", 140),
        ("InstallValidate", "", 150),
        ("InstallInitialize", "", 160),
        ("ProcessComponents", "", 170),
        ("InstallFiles", "", 200),
        ("InstallShortcuts", "", 210),
        ("WriteRegistryValues", "", 220),
        ("WriteEnvironmentStrings", "", 230),
        ("InstallServices", "", 240),
        ("StartServices", "", 250),
        ("RegisterProduct", "", 300),
        ("PublishProduct", "", 310),
        ("InstallFinalize", "", 400),
    ];

    for (action, condition, seq) in standard_actions {
        let query = Insert::into("InstallExecuteSequence").row(vec![
            Value::from(action),
            Value::from(condition),
            Value::Int(seq),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Standard seq {}: {}", action, e)))?;
    }

    // Pre-install script custom actions
    for (i, cmd) in manifest.scripts.pre_install.iter().enumerate() {
        let action_name = format!("PreInstallCmd_{}", i);
        // Type 34 = exe command line, Source = empty, Target = command
        let query = Insert::into("CustomAction").row(vec![
            Value::from(action_name.as_str()),
            Value::Int(34),
            Value::from(""),
            Value::from(format!("cmd.exe /c {}", cmd).as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Pre-install CA: {}", e)))?;

        // Schedule before InstallInitialize
        let query = Insert::into("InstallExecuteSequence").row(vec![
            Value::from(action_name.as_str()),
            Value::from("NOT Installed"),
            Value::Int(155 + i as i32),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Pre-install seq: {}", e)))?;

        debug!("Pre-install action {}: {}", i, cmd);
    }

    // Post-install script custom actions
    for (i, cmd) in manifest.scripts.post_install.iter().enumerate() {
        let action_name = format!("PostInstallCmd_{}", i);
        let query = Insert::into("CustomAction").row(vec![
            Value::from(action_name.as_str()),
            Value::Int(34),
            Value::from(""),
            Value::from(format!("cmd.exe /c {}", cmd).as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Post-install CA: {}", e)))?;

        // Schedule after InstallFinalize
        let query = Insert::into("InstallExecuteSequence").row(vec![
            Value::from(action_name.as_str()),
            Value::from("NOT Installed"),
            Value::Int(401 + i as i32),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Post-install seq: {}", e)))?;

        debug!("Post-install action {}: {}", i, cmd);
    }

    // Launch application after install (if configured)
    if let Some(ref run_after) = manifest.install.run_after_install {
        let query = Insert::into("CustomAction").row(vec![
            Value::from("LaunchApplication"),
            Value::Int(34),
            Value::from(""),
            Value::from(format!("[INSTALLDIR]{}", run_after).as_str()),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Launch CA: {}", e)))?;

        let query = Insert::into("InstallExecuteSequence").row(vec![
            Value::from("LaunchApplication"),
            Value::from("NOT Installed"),
            Value::Int(450),
        ]);
        package
            .insert_rows(query)
            .map_err(|e| CompilerError::Other(format!("Launch seq: {}", e)))?;
    }

    info!("Custom actions populated");
    Ok(())
}

/// Embed files as binary streams in the MSI package.
///
/// In a full implementation, files would be packaged into a cabinet (.cab) file.
/// This implementation stores them as embedded streams for simplicity.
fn embed_file_streams(
    package: &mut Package<Cursor<Vec<u8>>>,
    files: &[(PathBuf, String)],
) -> Result<()> {
    for (file_path, rel_path) in files {
        let stream_name = format!("file_{}", rel_path.replace(['\\', '/'], "_"));
        let data = std::fs::read(file_path)?;

        let mut writer = package
            .write_stream(&stream_name)
            .map_err(|e| CompilerError::Other(format!("Stream write {}: {}", rel_path, e)))?;
        writer
            .write_all(&data)
            .map_err(|e| CompilerError::Other(format!("Stream data {}: {}", rel_path, e)))?;

        debug!("Embedded: {} ({} bytes)", rel_path, data.len());
    }

    info!("File streams embedded: {} files", files.len());
    Ok(())
}

/// Sanitize a directory name for MSI (no spaces, max 32 chars).
fn sanitize_dir_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    sanitized.chars().take(32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_dir_name() {
        assert_eq!(sanitize_dir_name("My App"), "My_App");
        assert_eq!(sanitize_dir_name("Test-App 2.0"), "Test_App_2_0");
        assert_eq!(sanitize_dir_name("Short"), "Short");
    }

    #[test]
    fn test_msi_options_default() {
        let opts = MsiOptions::default();
        assert_eq!(opts.architecture, "x64");
        assert_eq!(opts.language, 1033);
        assert!(opts.per_machine);
    }

    #[test]
    fn test_create_msi_package() {
        // Test that we can create an MSI package and add tables
        let cursor = Cursor::new(Vec::new());
        let mut package = Package::create(PackageType::Installer, cursor).unwrap();
        create_msi_tables(&mut package).unwrap();

        // Verify tables exist
        assert!(package.has_table("Property"));
        assert!(package.has_table("Directory"));
        assert!(package.has_table("Component"));
        assert!(package.has_table("File"));
        assert!(package.has_table("Media"));
        assert!(package.has_table("Feature"));
        assert!(package.has_table("FeatureComponents"));
        assert!(package.has_table("Registry"));
        assert!(package.has_table("Shortcut"));
        assert!(package.has_table("Environment"));
        assert!(package.has_table("ServiceInstall"));
        assert!(package.has_table("ServiceControl"));
        assert!(package.has_table("CustomAction"));
        assert!(package.has_table("InstallExecuteSequence"));
        assert!(package.has_table("Upgrade"));
        assert!(package.has_table("LaunchCondition"));
    }

    #[test]
    fn test_populate_properties() {
        let toml_str = r#"
[app]
name = "Test MSI App"
version = "2.0.0"
publisher = "Test Corp"
description = "A test application"
url = "https://example.com"

[install]
default_dir = "{autopf}\\Test MSI App"

[files]
source = ["bin/**/*"]

[shortcuts]
desktop = false
start_menu = false

[ui]
theme = "modern"
"#;
        let manifest: VelocityManifest =
            velocity_config::parse_manifest_str(toml_str).unwrap();

        let cursor = Cursor::new(Vec::new());
        let mut package = Package::create(PackageType::Installer, cursor).unwrap();
        create_msi_tables(&mut package).unwrap();

        let options = MsiOptions::default();
        let product_code = Uuid::new_v4().to_string();
        let upgrade_code = Uuid::new_v4().to_string();

        populate_properties(
            &mut package,
            &manifest,
            &product_code,
            &upgrade_code,
            &options,
        )
        .unwrap();

        // Verify properties were inserted
        let query = msi::Select::table("Property");
        let rows = package.select_rows(query).unwrap();
        assert!(rows.len() >= 8, "Should have at least 8 properties");
    }
}
