//! MSI Builder — Generates Windows Installer (MSI) packages from Velocity projects.
//!
//! Maps `velocity.toml` configuration to MSI database tables, producing enterprise-ready
//! `.msi` packages compatible with Group Policy, SCCM, and `msiexec` deployment.
//!
//! Uses the `velocity-msi` crate for clean-room MSI generation with a from-scratch
//! OLE V4 compound file writer. No dependency on the `msi` (rust-msi) crate.
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
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;
use velocity_config::VelocityManifest;
use velocity_msi::{Column, MsiBuilder as VelocityMsi, Value};

/// Helper: insert a single row into a table
fn insert_row(builder: &mut VelocityMsi, table: &str, row: Vec<Value>) -> Result<()> {
    builder
        .insert_rows(table, vec![row])
        .map_err(|e| CompilerError::Other(format!("Failed to insert into {}: {}", table, e)))
}

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

    let mut builder = VelocityMsi::new();

    // Generate deterministic GUIDs using SHA-1 hash
    // This ensures the same product+version always gets the same ProductCode,
    // allowing uninstall via the MSI file even after rebuilds.
    use sha1::{Sha1, Digest};
    
    // ProductCode: deterministic based on name + version + arch
    // Format: {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} (uppercase with braces)
    let product_code_input = format!("{} {} {}", manifest.app.name, manifest.app.version, options.architecture);
    let mut hasher = Sha1::new();
    hasher.update(product_code_input.as_bytes());
    let hash = hasher.finalize();
    let product_code = format!(
        "{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
        u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]),
        u16::from_be_bytes([hash[4], hash[5]]),
        u16::from_be_bytes([hash[6], hash[7]]) & 0x0FFF | 0x5000, // Version 5 UUID
        u16::from_be_bytes([hash[8], hash[9]]) & 0x3FFF | 0x8000, // Variant 1
        u64::from_be_bytes([0, 0, hash[10], hash[11], hash[12], hash[13], hash[14], hash[15]])
    );
    
    // UpgradeCode: deterministic based on name only (stays same across versions)
    let upgrade_code = options
        .upgrade_code
        .clone()
        .unwrap_or_else(|| {
            let mut hasher = Sha1::new();
            hasher.update(format!("{}-upgrade", manifest.app.name).as_bytes());
            let hash = hasher.finalize();
            format!(
                "{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
                u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]),
                u16::from_be_bytes([hash[4], hash[5]]),
                u16::from_be_bytes([hash[6], hash[7]]) & 0x0FFF | 0x5000,
                u16::from_be_bytes([hash[8], hash[9]]) & 0x3FFF | 0x8000,
                u64::from_be_bytes([0, 0, hash[10], hash[11], hash[12], hash[13], hash[14], hash[15]])
            )
        });

    info!("ProductCode: {}", product_code);
    info!("UpgradeCode: {}", upgrade_code);

    // Set summary info
    builder.set_title(&format!("{} Installer", manifest.app.name));
    builder.set_author(&manifest.app.publisher);
    builder.set_subject(&format!("{} v{}", manifest.app.name, manifest.app.version));
    builder.set_comments(
        &manifest
            .app
            .description
            .clone()
            .unwrap_or_else(|| format!("{} installer package", manifest.app.name)),
    );
    builder.set_template(&options.architecture, options.language);

    // Create all required MSI tables
    create_msi_tables(&mut builder)?;

    // Populate Property table
    populate_properties(
        &mut builder,
        manifest,
        &product_code,
        &upgrade_code,
        options,
    )?;

    // Populate Directory table
    let dir_id_map = populate_directories(&mut builder, manifest)?;

    // Collect files from the project
    let files = collect_msi_files(manifest, options)?;
    info!("Collected {} files for MSI", files.len());

    // Populate Component, File, and Media tables
    let component_count = populate_components(&mut builder, &files, &dir_id_map)?;

    // Populate Feature table
    populate_features(&mut builder, manifest, &files)?;

    // Populate Registry table
    populate_registry(&mut builder, manifest)?;

    // Populate Shortcut table
    populate_shortcuts(&mut builder, manifest, &dir_id_map)?;

    // Populate Environment table
    populate_environment(&mut builder, manifest)?;

    // Populate ServiceInstall and ServiceControl tables
    populate_services(&mut builder, manifest)?;

    // Populate CustomAction and InstallExecuteSequence tables
    populate_custom_actions(&mut builder, manifest)?;

    // Build and embed cabinet file (standard MSI packaging)
    build_and_embed_cabinet(&mut builder, &files)?;

    // Build the MSI file
    let msi_data = builder
        .build()
        .map_err(|e| CompilerError::Other(format!("Failed to build MSI: {}", e)))?;

    // Write to output file
    if let Some(parent) = options.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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
fn create_msi_tables(builder: &mut VelocityMsi) -> Result<()> {
    // Property table
    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Property table: {}", e)))?;

    // Directory table
    builder.create_table("Directory", vec![
        Column::build("Directory").string(72).primary_key().build(),
        Column::build("Directory_Parent").string(72).nullable().build(),
        Column::build("DefaultDir").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Directory table: {}", e)))?;

    // Component table
    builder.create_table("Component", vec![
        Column::build("Component").string(72).primary_key().build(),
        Column::build("ComponentId").string(38).nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("KeyPath").string(72).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Component table: {}", e)))?;

    // File table
    builder.create_table("File", vec![
        Column::build("File").string(72).primary_key().build(),
        Column::build("Component_").string(72).nullable().build(),
        Column::build("FileName").string(255).nullable().build(),
        Column::build("FileSize").int32().nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Sequence").int32().nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("File table: {}", e)))?;

    // Media table
    builder.create_table("Media", vec![
        Column::build("DiskId").int16().primary_key().build(),
        Column::build("LastSequence").int32().nullable().build(),
        Column::build("Cabinet").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Media table: {}", e)))?;

    // Feature table
    builder.create_table("Feature", vec![
        Column::build("Feature").string(38).primary_key().build(),
        Column::build("Feature_Parent").string(38).nullable().build(),
        Column::build("Title").string(64).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
        Column::build("Display").int16().nullable().build(),
        Column::build("Level").int16().nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Feature table: {}", e)))?;

    // FeatureComponents table
    builder.create_table("FeatureComponents", vec![
        Column::build("Feature_").string(38).primary_key().build(),
        Column::build("Component_").string(72).primary_key().build(),
    ]).map_err(|e| CompilerError::Other(format!("FeatureComponents table: {}", e)))?;

    // Registry table
    builder.create_table("Registry", vec![
        Column::build("Registry").string(72).primary_key().build(),
        Column::build("Root").int16().nullable().build(),
        Column::build("Key").string(255).nullable().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Registry table: {}", e)))?;

    // Shortcut table
    builder.create_table("Shortcut", vec![
        Column::build("Shortcut").string(72).primary_key().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Name").string(128).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
        Column::build("Arguments").string(255).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
        Column::build("Hotkey").int16().nullable().build(),
        Column::build("Icon_").string(72).nullable().build(),
        Column::build("IconIndex").int16().nullable().build(),
        Column::build("ShowCmd").int16().nullable().build(),
        Column::build("WkDir").string(72).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Shortcut table: {}", e)))?;

    // Icon table
    builder.create_table("Icon", vec![
        Column::build("Name").string(72).primary_key().build(),
        Column::build("Data").binary().nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Icon table: {}", e)))?;

    // Environment table
    builder.create_table("Environment", vec![
        Column::build("Environment").string(72).primary_key().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Environment table: {}", e)))?;

    // ServiceInstall table
    builder.create_table("ServiceInstall", vec![
        Column::build("ServiceInstall").string(72).primary_key().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("DisplayName").string(255).nullable().build(),
        Column::build("ServiceType").int32().nullable().build(),
        Column::build("StartType").int32().nullable().build(),
        Column::build("ErrorControl").int32().nullable().build(),
        Column::build("LoadOrderGroup").string(255).nullable().build(),
        Column::build("Dependencies").string(255).nullable().build(),
        Column::build("StartName").string(255).nullable().build(),
        Column::build("Password").string(255).nullable().build(),
        Column::build("Arguments").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("ServiceInstall table: {}", e)))?;

    // ServiceControl table
    builder.create_table("ServiceControl", vec![
        Column::build("ServiceControl").string(72).primary_key().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Event").int32().nullable().build(),
        Column::build("Arguments").string(255).nullable().build(),
        Column::build("Wait").int16().nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("ServiceControl table: {}", e)))?;

    // CustomAction table
    builder.create_table("CustomAction", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Type").int16().nullable().build(),
        Column::build("Source").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("CustomAction table: {}", e)))?;

    // InstallExecuteSequence table
    builder.create_table("InstallExecuteSequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("InstallExecuteSequence table: {}", e)))?;

    // Upgrade table
    builder.create_table("Upgrade", vec![
        Column::build("UpgradeCode").string(38).primary_key().build(),
        Column::build("VersionMin").string(20).nullable().build(),
        Column::build("VersionMax").string(20).nullable().build(),
        Column::build("Language").string(20).nullable().build(),
        Column::build("Attributes").int32().nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("Upgrade table: {}", e)))?;

    // LaunchCondition table
    builder.create_table("LaunchCondition", vec![
        Column::build("Condition").string(255).primary_key().build(),
        Column::build("Description").string(255).nullable().build(),
    ]).map_err(|e| CompilerError::Other(format!("LaunchCondition table: {}", e)))?;

    Ok(())
}

/// Populate the Property table with standard and custom properties.
fn populate_properties(
    package: &mut VelocityMsi,
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
        ("ProductLanguage", format!("{}", options.language)),
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
            manifest.app.description.clone().unwrap_or_default(),
        ),
        (
            "ALLUSERS",
            if options.per_machine { "1" } else { "" }.to_string(),
        ),
    ];

    for (name, value) in properties {
        // Skip properties with empty values — they cause MSI validation errors
        if value.is_empty() {
            debug!("Skipping empty property: {}", name);
            continue;
        }
        insert_row(package, "Property", vec![Value::from(name), Value::from(value)])?;
    }

    // Add description property
    let desc = manifest
        .app
        .description
        .clone()
        .unwrap_or_else(|| format!("{} Installer", manifest.app.name));
    insert_row(package, "Property", vec![Value::from("Description"), Value::from(desc)])?;

    info!("Properties populated");
    Ok(())
}

/// Populate the Directory table with the installation directory structure.
fn populate_directories(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
) -> Result<HashMap<String, String>> {
    let mut dir_map = HashMap::new();

    // Standard directories
    // TARGETDIR is the root — Directory_Parent must be null
    insert_row(package, "Directory", vec![
        Value::from("TARGETDIR"),
        Value::Null,
        Value::from("SourceDir"),
    ])?;

    // ProgramFiles64Folder or ProgramFilesFolder
    let pf_dir = if manifest.install.arch.contains("64") {
        "ProgramFiles64Folder"
    } else {
        "ProgramFilesFolder"
    };
    insert_row(package, "Directory", vec![
        Value::from(pf_dir),
        Value::from("TARGETDIR"),
        Value::from("PFiles"),
    ])?;

    // Application directory
    let app_dir_name = sanitize_dir_name(&manifest.app.name);
    let app_dir_id = "INSTALLDIR";
    insert_row(package, "Directory", vec![
        Value::from(app_dir_id),
        Value::from(pf_dir),
        Value::from(format!("{}:{}", app_dir_name, app_dir_name)),
    ])?;
    dir_map.insert("INSTALLDIR".to_string(), app_dir_id.to_string());

    // ProgramMenuFolder for shortcuts
    insert_row(package, "Directory", vec![
        Value::from("ProgramMenuFolder"),
        Value::from("TARGETDIR"),
        Value::from("Programs"),
    ])?;

    // Application Start Menu folder
    if manifest.shortcuts.start_menu {
        let menu_dir = sanitize_dir_name(&manifest.app.name);
        insert_row(package, "Directory", vec![
            Value::from("ApplicationProgramsFolder"),
            Value::from("ProgramMenuFolder"),
            Value::from(format!("{}:{}", menu_dir, menu_dir)),
        ])?;
        dir_map.insert(
            "ApplicationProgramsFolder".to_string(),
            "ApplicationProgramsFolder".to_string(),
        );
    }

    // Desktop directory
    if manifest.shortcuts.desktop || manifest.install.create_desktop_shortcut {
        insert_row(package, "Directory", vec![
            Value::from("DesktopFolder"),
            Value::from("TARGETDIR"),
            Value::from("Desktop"),
        ])?;
        dir_map.insert("DesktopFolder".to_string(), "DesktopFolder".to_string());
    }

    info!("Directories populated ({} entries)", dir_map.len());
    Ok(dir_map)
}

/// Collect files from the project for inclusion in the MSI.
fn collect_msi_files(
    manifest: &VelocityManifest,
    options: &MsiOptions,
) -> Result<Vec<(PathBuf, String)>> {
    let files = velocity_config::collect_files(manifest, &options.project_dir)?;
    Ok(files)
}

/// Populate Component, File, and Media tables.
fn populate_components(
    package: &mut VelocityMsi,
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
        insert_row(package, "Component", vec![
            Value::from(component_id.as_str()),
            Value::from(component_guid.as_str()),
            Value::from(install_dir.as_str()),
            Value::Int(0),                 // Attributes: none
            Value::Null,                   // Condition (nullable)
            Value::from(file_id.as_str()), // KeyPath = file
        ])?;

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
                file_name.split('.').next().unwrap_or("FILE").to_uppercase(),
                file_name.rsplit('.').next().unwrap_or("TXT").to_uppercase()
            );
            format!("{}|{}", short, file_name)
        } else {
            file_name.clone()
        };

        insert_row(package, "File", vec![
            Value::from(file_id.as_str()),
            Value::from(component_id.as_str()),
            Value::from(msi_filename.as_str()),
            Value::Int(file_size),
            Value::Int(0),              // Attributes
            Value::Int((i + 1) as i32), // Sequence
        ])?;

        component_count += 1;
        debug!(
            "Component {}: {} ({} bytes)",
            component_id, rel_path, file_size
        );
    }

    // Media table — single cabinet
    insert_row(package, "Media", vec![
        Value::Int(1),                  // DiskId
        Value::Int(files.len() as i32), // LastSequence
        Value::from("#Velocity.cab"),   // Cabinet (embedded)
    ])?;

    info!("Components populated: {} components", component_count);
    Ok(component_count)
}

/// Populate the Feature table.
fn populate_features(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
    files: &[(PathBuf, String)],
) -> Result<()> {
    // Main feature — Feature_Parent must be null (root feature)
    insert_row(package, "Feature", vec![
        Value::from("Complete"),
        Value::Null, // No parent — root feature
        Value::from(format!("{} Setup", manifest.app.name)),
        Value::from(format!("Complete installation of {}", manifest.app.name)),
        Value::Int(1), // Display
        Value::Int(1), // Level (installed by default)
        Value::from("INSTALLDIR"),
        Value::Int(0), // Attributes
    ])?;

    // Link all components to the main feature
    for i in 0..files.len() {
        let component_id = format!("comp_{}", i);
        insert_row(package, "FeatureComponents", vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ])?;
    }

    // Add user-defined components as sub-features
    for comp in &manifest.components {
        let feature_id = format!("Feature_{}", comp.id);
        let parent = "Complete".to_string();
        let level = if comp.selected_by_default { 1 } else { 0 };

        insert_row(package, "Feature", vec![
            Value::from(feature_id.as_str()),
            Value::from(parent.as_str()),
            Value::from(comp.name.clone()),
            Value::from(comp.description.clone().unwrap_or_default()),
            Value::Int(0), // Display: hidden under parent
            Value::Int(level),
            Value::from("INSTALLDIR"),
            Value::Int(0),
        ])?;
    }

    info!("Features populated");
    Ok(())
}

/// Populate the Registry table from manifest registry entries.
fn populate_registry(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, entry) in manifest.registry.iter().enumerate() {
        let reg_id = format!("reg_{}", i);
        let component_id = format!("comp_reg_{}", i);

        // Create a component for this registry entry
        let reg_guid = Uuid::new_v4().to_string();
        insert_row(package, "Component", vec![
            Value::from(component_id.as_str()),
            Value::from(reg_guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::Null,
            Value::from(reg_id.as_str()), // KeyPath
        ])?;

        // Link to feature
        insert_row(package, "FeatureComponents", vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ])?;

        // Map root to MSI root integer
        let root = match entry.root.to_uppercase().as_str() {
            "HKLM" => 2,
            "HKCU" => 1,
            "HKCR" => 0,
            "HKU" => 3,
            _ => 2, // Default to HKLM
        };

        insert_row(package, "Registry", vec![
            Value::from(reg_id.as_str()),
            Value::Int(root),
            Value::from(entry.key.as_str()),
            Value::from(entry.name.clone().unwrap_or_default().as_str()),
            Value::from(entry.value.as_str()),
            Value::from(component_id.as_str()),
        ])?;

        debug!("Registry: {}\\{} = {}", entry.root, entry.key, entry.value);
    }

    info!("Registry populated: {} entries", manifest.registry.len());
    Ok(())
}

/// Populate the Shortcut table from manifest shortcut configuration.
fn populate_shortcuts(
    package: &mut VelocityMsi,
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
            insert_row(package, "Component", vec![
                Value::from(component_id),
                Value::from(guid.as_str()),
                Value::from(desktop_dir.as_str()),
                Value::Int(0),
                Value::Null,
                Value::from(shortcut_id),
            ])?;

            insert_row(package, "FeatureComponents", vec![Value::from("Complete"), Value::from(component_id)])?;

            insert_row(package, "Shortcut", vec![
                Value::from(shortcut_id),
                Value::from(desktop_dir.as_str()),
                Value::from(manifest.app.name.as_str()),
                Value::from(component_id),
                Value::from("[INSTALLDIR]"), // Target
                Value::Null,                 // Arguments
                Value::from(
                    manifest
                        .app
                        .description
                        .clone()
                        .unwrap_or_default()
                        .as_str(),
                ),
                Value::Int(0),             // Hotkey
                Value::Null,               // Icon_ (id_string — must be null, not empty)
                Value::Int(0),             // IconIndex
                Value::Int(1),             // ShowCmd (normal)
                Value::from("INSTALLDIR"), // Working dir
            ])?;
            shortcut_count += 1;
        }
    }

    // Start Menu shortcuts
    if manifest.shortcuts.start_menu {
        if let Some(menu_dir) = dir_id_map.get("ApplicationProgramsFolder") {
            let shortcut_id = "StartMenuShortcut";
            let component_id = "comp_startmenu_shortcut";
            let guid = Uuid::new_v4().to_string();

            insert_row(package, "Component", vec![
                Value::from(component_id),
                Value::from(guid.as_str()),
                Value::from(menu_dir.as_str()),
                Value::Int(0),
                Value::Null,
                Value::from(shortcut_id),
            ])?;

            insert_row(package, "FeatureComponents", vec![Value::from("Complete"), Value::from(component_id)])?;

            insert_row(package, "Shortcut", vec![
                Value::from(shortcut_id),
                Value::from(menu_dir.as_str()),
                Value::from(manifest.app.name.as_str()),
                Value::from(component_id),
                Value::from("[INSTALLDIR]"),
                Value::Null,
                Value::from(
                    manifest
                        .app
                        .description
                        .clone()
                        .unwrap_or_default()
                        .as_str(),
                ),
                Value::Int(0),
                Value::Null,  // Icon_ (id_string — must be null, not empty)
                Value::Int(0),
                Value::Int(1),
                Value::from("INSTALLDIR"),
            ])?;
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
            insert_row(package, "Component", vec![
                Value::from(component_id.as_str()),
                Value::from(guid.as_str()),
                Value::from(dir.as_str()),
                Value::Int(0),
                Value::Null,
                Value::from(shortcut_id.as_str()),
            ])?;

            insert_row(package, "FeatureComponents", vec![
                Value::from("Complete"),
                Value::from(component_id.as_str()),
            ])?;

            insert_row(package, "Shortcut", vec![
                Value::from(shortcut_id.as_str()),
                Value::from(dir.as_str()),
                Value::from(custom.name.as_str()),
                Value::from(component_id.as_str()),
                Value::from(format!("[INSTALLDIR]{}", custom.target).as_str()),
                Value::Null, // Arguments
                Value::Null, // Description
                Value::Int(0),
                Value::Null, // Icon_ (id_string — must be null)
                Value::Int(0),
                Value::Int(1),
                Value::from(
                    custom
                        .working_dir
                        .clone()
                        .unwrap_or_else(|| "[INSTALLDIR]".to_string())
                        .as_str(),
                ),
            ])?;
            shortcut_count += 1;
        }
    }

    info!("Shortcuts populated: {} entries", shortcut_count);
    Ok(())
}

/// Populate the Environment table from manifest env_vars.
fn populate_environment(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, env) in manifest.env_vars.iter().enumerate() {
        let env_id = format!("env_{}", i);
        let component_id = format!("comp_env_{}", i);
        let guid = Uuid::new_v4().to_string();

        // Component for env var
        insert_row(package, "Component", vec![
            Value::from(component_id.as_str()),
            Value::from(guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::Null,
            Value::from(env_id.as_str()),
        ])?;

        insert_row(package, "FeatureComponents", vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ])?;

        // Value with optional append separator
        let value = if env.append {
            format!("{};{}", env.name, env.value)
        } else {
            env.value.clone()
        };

        insert_row(package, "Environment", vec![
            Value::from(env_id.as_str()),
            Value::from(env.name.as_str()),
            Value::from(value.as_str()),
            Value::from(component_id.as_str()),
        ])?;

        debug!("Environment: {} = {}", env.name, env.value);
    }

    info!("Environment populated: {} entries", manifest.env_vars.len());
    Ok(())
}

/// Populate ServiceInstall and ServiceControl tables.
fn populate_services(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
) -> Result<()> {
    for (i, svc) in manifest.services.iter().enumerate() {
        let svc_install_id = format!("svc_install_{}", i);
        let svc_ctrl_id = format!("svc_ctrl_{}", i);
        let component_id = format!("comp_svc_{}", i);
        let guid = Uuid::new_v4().to_string();

        // Component for service
        insert_row(package, "Component", vec![
            Value::from(component_id.as_str()),
            Value::from(guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::Null,
            Value::from(svc_install_id.as_str()),
        ])?;

        insert_row(package, "FeatureComponents", vec![
            Value::from("Complete"),
            Value::from(component_id.as_str()),
        ])?;

        // Map start type
        let start_type = match svc.start_type.as_str() {
            "auto" => 2,
            "manual" => 3,
            "disabled" => 4,
            "delayed_auto" => 2, // MSI doesn't have delayed auto; use auto
            _ => 2,
        };

        // ServiceInstall
        insert_row(package, "ServiceInstall", vec![
            Value::from(svc_install_id.as_str()),
            Value::from(svc.name.as_str()),
            Value::from(svc.display_name.as_str()),
            Value::Int(0x10), // SERVICE_WIN32_OWN_PROCESS
            Value::Int(start_type),
            Value::Int(1),   // ErrorControl: normal
            Value::Null,     // LoadOrderGroup
            Value::from(svc.dependencies.join("\0").as_str()),
            Value::from(svc.account.clone().unwrap_or_default().as_str()),
            Value::Null,     // Password
            Value::Null,     // Arguments
            Value::from(component_id.as_str()),
            Value::from(svc.description.clone().unwrap_or_default().as_str()),
        ])?;

        // ServiceControl — start on install, stop+delete on uninstall
        insert_row(package, "ServiceControl", vec![
            Value::from(svc_ctrl_id.as_str()),
            Value::from(svc.name.as_str()),
            Value::Int(1 + 2 + 4), // Install: start(1) + stop(2) + delete(4)
            Value::Null,           // Arguments
            Value::Int(1),         // Wait
            Value::from(component_id.as_str()),
        ])?;

        debug!("Service: {} ({})", svc.display_name, svc.name);
    }

    info!("Services populated: {} entries", manifest.services.len());
    Ok(())
}

/// Populate CustomAction and InstallExecuteSequence tables.
fn populate_custom_actions(
    package: &mut VelocityMsi,
    manifest: &VelocityManifest,
) -> Result<()> {
    // Standard sequence actions — (action, condition, sequence)
    // condition = None means always run (null in DB)
    let standard_actions: Vec<(&str, Option<&str>, i32)> = vec![
        ("AppSearch", None, 100),
        ("LaunchConditions", Some("NOT Installed"), 105),
        ("ValidateProductID", None, 110),
        ("CostInitialize", None, 120),
        ("FileCost", None, 130),
        ("CostFinalize", None, 140),
        ("InstallValidate", None, 150),
        ("InstallInitialize", None, 160),
        ("ProcessComponents", None, 170),
        ("InstallFiles", None, 200),
        ("InstallShortcuts", None, 210),
        ("WriteRegistryValues", None, 220),
        ("WriteEnvironmentStrings", None, 230),
        ("InstallServices", None, 240),
        ("StartServices", None, 250),
        ("RegisterProduct", None, 300),
        ("PublishProduct", None, 310),
        ("InstallFinalize", None, 400),
    ];

    for (action, condition, seq) in standard_actions {
        let cond_val = match condition {
            Some(c) => Value::from(c),
            None => Value::Null,
        };
        insert_row(package, "InstallExecuteSequence", vec![
            Value::from(action),
            cond_val,
            Value::Int(seq),
        ])?;
    }

    // Pre-install script custom actions
    for (i, cmd) in manifest.scripts.pre_install.iter().enumerate() {
        let action_name = format!("PreInstallCmd_{}", i);
        // Type 34 = exe command line, Source = null, Target = command
        insert_row(package, "CustomAction", vec![
            Value::from(action_name.as_str()),
            Value::Int(34),
            Value::Null,
            Value::from(format!("cmd.exe /c {}", cmd).as_str()),
        ])?;

        // Schedule before InstallInitialize
        insert_row(package, "InstallExecuteSequence", vec![
            Value::from(action_name.as_str()),
            Value::from("NOT Installed"),
            Value::Int(155 + i as i32),
        ])?;

        debug!("Pre-install action {}: {}", i, cmd);
    }

    // Post-install script custom actions
    for (i, cmd) in manifest.scripts.post_install.iter().enumerate() {
        let action_name = format!("PostInstallCmd_{}", i);
        insert_row(package, "CustomAction", vec![
            Value::from(action_name.as_str()),
            Value::Int(34),
            Value::Null,
            Value::from(format!("cmd.exe /c {}", cmd).as_str()),
        ])?;

        // Schedule after InstallFinalize
        insert_row(package, "InstallExecuteSequence", vec![
            Value::from(action_name.as_str()),
            Value::from("NOT Installed"),
            Value::Int(401 + i as i32),
        ])?;

        debug!("Post-install action {}: {}", i, cmd);
    }

    // Launch application after install (if configured)
    if let Some(ref run_after) = manifest.install.run_after_install {
        insert_row(package, "CustomAction", vec![
            Value::from("LaunchApplication"),
            Value::Int(34),
            Value::Null,
            Value::from(format!("[INSTALLDIR]{}", run_after).as_str()),
        ])?;

        insert_row(package, "InstallExecuteSequence", vec![
            Value::from("LaunchApplication"),
            Value::from("NOT Installed"),
            Value::Int(450),
        ])?;
    }

    info!("Custom actions populated");
    Ok(())
}

/// Build a cabinet (.cab) file containing all application files and embed it in the MSI.
///
/// Creates a proper MSI cabinet that Windows Installer can extract during installation.
/// This is the standard approach for MSI packaging, compatible with Group Policy and SCCM.
fn build_and_embed_cabinet(
    package: &mut VelocityMsi,
    files: &[(PathBuf, String)],
) -> Result<()> {
    // Nothing to embed — skip cabinet creation
    if files.is_empty() {
        info!("No files to cabinet — skipping");
        return Ok(());
    }

    // Build cabinet in memory using a Cursor (which implements Write + Seek)
    let mut cab_data = Cursor::new(Vec::new());
    {
        let mut cab_builder = cab::CabinetBuilder::new();

        // Add all files in a single MSZIP-compressed folder
        let folder = cab_builder.add_folder(cab::CompressionType::MsZip);
        for (_file_path, rel_path) in files {
            folder.add_file(rel_path);
        }

        // Build the cabinet
        let mut cab_writer = cab_builder
            .build(&mut cab_data)
            .map_err(|e| CompilerError::Other(format!("Cabinet build: {}", e)))?;

        // Write file data one at a time (in the same order as add_file)
        for (file_path, rel_path) in files {
            let mut writer = cab_writer
                .next_file()
                .map_err(|e| CompilerError::Other(format!("Cabinet next_file: {}", e)))?
                .ok_or_else(|| {
                    CompilerError::Other(format!(
                        "Cabinet writer ended early, expected file: {}",
                        rel_path
                    ))
                })?;

            let mut reader = std::fs::File::open(file_path)?;
            std::io::copy(&mut reader, &mut writer)
                .map_err(|e| CompilerError::Other(format!("Cabinet copy {}: {}", rel_path, e)))?;
        }

        cab_writer
            .finish()
            .map_err(|e| CompilerError::Other(format!("Cabinet finish: {}", e)))?;
    }

    // Get the raw bytes
    let cab_bytes = cab_data.into_inner();
    let cab_size = cab_bytes.len();

    // Embed the cabinet as a stream in the MSI (standard MSI pattern)
    package.add_stream("Velocity.cab".to_string(), cab_bytes);

    info!(
        "Cabinet built and embedded: {} files, {} bytes",
        files.len(),
        cab_size
    );
    Ok(())
}

/// Options for MSI code signing.
#[derive(Debug, Clone)]
pub struct MsiSignOptions {
    /// Path to the certificate file (.pfx / .p12)
    pub cert_path: PathBuf,
    /// Password for the certificate (if required)
    pub cert_password: Option<String>,
    /// Path to signtool.exe (defaults to auto-detect from Windows SDK)
    pub signtool_path: Option<PathBuf>,
    /// Timestamp server URL (e.g., "http://timestamp.digicert.com")
    pub timestamp_url: Option<String>,
    /// Description shown in the signing dialog
    pub description: Option<String>,
}

/// Sign an MSI package with a digital certificate.
///
/// Uses Windows `signtool.exe` to apply an Authenticode signature.
/// This is required for enterprise deployment via Group Policy in many organizations.
///
/// # Arguments
/// * `msi_path` - Path to the .msi file to sign
/// * `options` - Signing options (certificate, timestamp server, etc.)
///
/// # Returns
/// Result indicating success or failure
#[cfg(target_os = "windows")]
pub fn sign_msi(msi_path: &Path, options: &MsiSignOptions) -> Result<()> {
    use std::process::Command;

    info!("Signing MSI: {}", msi_path.display());

    // Find signtool.exe
    let signtool = if let Some(ref path) = options.signtool_path {
        path.clone()
    } else {
        find_signtool().ok_or_else(|| {
            CompilerError::Other(
                "signtool.exe not found. Install Windows SDK or provide signtool_path.".to_string(),
            )
        })?
    };

    let mut cmd = Command::new(&signtool);
    cmd.arg("sign");

    // Certificate
    cmd.arg("/f").arg(&options.cert_path);

    // Password
    if let Some(ref password) = options.cert_password {
        cmd.arg("/p").arg(password);
    }

    // Description
    if let Some(ref desc) = options.description {
        cmd.arg("/d").arg(desc);
    }

    // Timestamp
    if let Some(ref url) = options.timestamp_url {
        cmd.arg("/tr").arg(url);
        cmd.arg("/td").arg("sha256");
    }

    // Use SHA256
    cmd.arg("/fd").arg("sha256");

    // The MSI file
    cmd.arg(msi_path);

    debug!("Running: {:?}", cmd);

    let output = cmd
        .output()
        .map_err(|e| CompilerError::Other(format!("Failed to run signtool: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(CompilerError::Other(format!(
            "signtool failed (exit {}): {} {}",
            output.status, stdout, stderr
        )));
    }

    info!("MSI signed successfully: {}", msi_path.display());
    Ok(())
}

/// Sign an MSI package (stub for non-Windows platforms).
#[cfg(not(target_os = "windows"))]
pub fn sign_msi(_msi_path: &Path, _options: &MsiSignOptions) -> Result<()> {
    Err(CompilerError::Other(
        "MSI signing is only supported on Windows".to_string(),
    ))
}

/// Find signtool.exe from common Windows SDK installation paths.
#[cfg(target_os = "windows")]
fn find_signtool() -> Option<PathBuf> {
    use std::env;

    // Check PATH first
    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            let candidate = dir.join("signtool.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // Check common Windows SDK locations
    let program_files = env::var("ProgramFiles(x86)")
        .or_else(|_| env::var("ProgramFiles"))
        .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());

    let sdk_base = Path::new(&program_files)
        .join("Windows Kits")
        .join("10")
        .join("bin");
    if sdk_base.exists() {
        // Search for latest version
        if let Ok(entries) = std::fs::read_dir(&sdk_base) {
            let mut versions: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .collect();
            versions.sort_by_key(|e| std::cmp::Reverse(e.file_name()));

            for version_dir in versions {
                let signtool = version_dir.path().join("x64").join("signtool.exe");
                if signtool.exists() {
                    return Some(signtool);
                }
            }
        }
    }

    warn!("signtool.exe not found in PATH or Windows SDK");
    None
}

/// Validate that an MSI file has basic structural integrity.
///
/// This parses the OLE compound file structure to verify:
/// - Valid OLE2 header
/// - Presence of SummaryInformation stream
/// - Presence of string pool streams
/// - Lists table streams found in the database
pub fn validate_msi(msi_path: &Path) -> Result<MsiValidationResult> {
    let data = std::fs::read(msi_path)?;
    let msi_size = data.len() as u64;

    let info = velocity_msi::validate_ole(&data).map_err(|e| {
        CompilerError::Other(format!("OLE validation error: {}", e))
    })?;

    let mut missing_tables = Vec::new();
    if !info.valid_ole {
        missing_tables.push("Invalid OLE2 header".to_string());
    }
    if !info.has_summary {
        missing_tables.push("Missing SummaryInformation stream".to_string());
    }
    if !info.has_string_pool {
        missing_tables.push("Missing string pool streams".to_string());
    }

    // Check for cabinet (any stream that's not a table or system stream)
    let has_cabinet = info.stream_names.iter().any(|name| {
        !name.starts_with('\u{0005}') && // not SummaryInformation
        !name.starts_with('\u{4840}') && // not encoded table/pool stream
        !name.is_empty()
    });

    Ok(MsiValidationResult {
        msi_path: msi_path.to_path_buf(),
        msi_size,
        missing_tables,
        has_cabinet,
        is_valid: info.valid_ole && info.has_summary && info.has_string_pool,
    })
}

/// Result of MSI validation.
#[derive(Debug)]
pub struct MsiValidationResult {
    pub msi_path: PathBuf,
    pub msi_size: u64,
    pub missing_tables: Vec<String>,
    pub has_cabinet: bool,
    pub is_valid: bool,
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
        // Test that we can create an MSI builder and add tables
        let mut builder = VelocityMsi::new();
        create_msi_tables(&mut builder).unwrap();

        // Verify by building the MSI and checking it's valid
        let msi_data = builder.build().unwrap();
        assert!(msi_data.len() > 1000, "MSI should have meaningful content");
        // Verify OLE2 magic bytes
        assert_eq!(&msi_data[0..4], &[0xD0, 0xCF, 0x11, 0xE0]);
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
        let manifest: VelocityManifest = velocity_config::parse_manifest_str(toml_str).unwrap();

        let mut builder = VelocityMsi::new();
        create_msi_tables(&mut builder).unwrap();

        let options = MsiOptions::default();
        let product_code = Uuid::new_v4().to_string();
        let upgrade_code = Uuid::new_v4().to_string();

        populate_properties(
            &mut builder,
            &manifest,
            &product_code,
            &upgrade_code,
            &options,
        )
        .unwrap();

        // Verify by building the MSI
        let msi_data = builder.build().unwrap();
        assert!(msi_data.len() > 1000, "MSI should have meaningful content");
    }

    #[test]
    fn test_msi_validate_roundtrip() {
        // Create a minimal MSI and verify it's written successfully
        let mut builder = VelocityMsi::new();
        create_msi_tables(&mut builder).unwrap();

        // Populate minimal required data
        let toml_str = r#"
[app]
name = "Validate Test"
version = "1.0.0"
publisher = "Test"
description = "Test"
url = "https://example.com"

[install]
default_dir = "{autopf}\\Test"

[files]
source = ["*.txt"]

[shortcuts]
desktop = false
start_menu = false

[ui]
theme = "modern"
"#;
        let manifest: VelocityManifest = velocity_config::parse_manifest_str(toml_str).unwrap();

        let options = MsiOptions::default();
        let product_code = Uuid::new_v4().to_string();
        let upgrade_code = Uuid::new_v4().to_string();
        populate_properties(
            &mut builder,
            &manifest,
            &product_code,
            &upgrade_code,
            &options,
        )
        .unwrap();

        // Build the MSI
        let msi_data = builder.build().unwrap();

        // Verify MSI was generated with reasonable size
        assert!(
            msi_data.len() > 1000,
            "MSI should have meaningful content, got {} bytes",
            msi_data.len()
        );

        // Verify it starts with the OLE2 magic bytes (D0 CF 11 E0)
        assert!(msi_data.len() >= 4);
        assert_eq!(
            &msi_data[0..4],
            &[0xD0, 0xCF, 0x11, 0xE0],
            "MSI should be a valid OLE2 compound document"
        );
    }

    #[test]
    fn test_msi_sign_options_default() {
        let opts = MsiSignOptions {
            cert_path: PathBuf::from("cert.pfx"),
            cert_password: None,
            signtool_path: None,
            timestamp_url: None,
            description: None,
        };
        assert_eq!(opts.cert_path, PathBuf::from("cert.pfx"));
        assert!(opts.cert_password.is_none());
    }
}
