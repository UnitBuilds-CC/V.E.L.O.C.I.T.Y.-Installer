/// Test: full compiler-like MSI with all tables, test with msiexec
use velocity_msi::{MsiBuilder, Column, Value};
use std::process::Command;

fn main() {
    let mut builder = MsiBuilder::new();
    builder.set_title("Sample App Installer");
    builder.set_author("Velocity Team");
    builder.set_subject("Sample App v1.0.0");
    builder.set_comments("Sample app installer");
    builder.set_template("x64", 1033);

    // Create ALL tables the compiler creates (18 tables)
    builder.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("Directory", vec![
        Column::build("Directory").string(72).primary_key().build(),
        Column::build("Directory_Parent").string(72).nullable().build(),
        Column::build("DefaultDir").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("Component", vec![
        Column::build("Component").string(72).primary_key().build(),
        Column::build("ComponentId").string(38).nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("KeyPath").string(72).nullable().build(),
    ]).unwrap();
    builder.create_table("File", vec![
        Column::build("File").string(72).primary_key().build(),
        Column::build("Component_").string(72).build(),
        Column::build("FileName").string(255).build(),
        Column::build("FileSize").int32().build(),
        Column::build("Version").string(72).nullable().build(),
        Column::build("Language").int16().nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Sequence").int32().build(),
    ]).unwrap();
    builder.create_table("Media", vec![
        Column::build("DiskId").int16().primary_key().build(),
        Column::build("LastSequence").int32().build(),
        Column::build("DiskPrompt").string(255).nullable().build(),
        Column::build("VolumeLabel").string(32).nullable().build(),
        Column::build("Cabinet").string(255).nullable().build(),
        Column::build("Source").string(72).nullable().build(),
    ]).unwrap();
    builder.create_table("Feature", vec![
        Column::build("Feature").string(38).primary_key().build(),
        Column::build("Feature_Parent").string(38).nullable().build(),
        Column::build("Title").string(64).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
        Column::build("Display").int16().nullable().build(),
        Column::build("Level").int16().nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
    ]).unwrap();
    builder.create_table("FeatureComponents", vec![
        Column::build("Feature_").string(38).primary_key().build(),
        Column::build("Component_").string(72).primary_key().build(),
    ]).unwrap();
    builder.create_table("Registry", vec![
        Column::build("Registry").string(72).primary_key().build(),
        Column::build("Root").int16().nullable().build(),
        Column::build("Key").string(255).nullable().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).unwrap();
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
    ]).unwrap();
    builder.create_table("Icon", vec![
        Column::build("Name").string(72).primary_key().build(),
        Column::build("Data").binary().nullable().build(),
    ]).unwrap();
    builder.create_table("Environment", vec![
        Column::build("Environment").string(72).primary_key().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).unwrap();
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
    ]).unwrap();
    builder.create_table("ServiceControl", vec![
        Column::build("ServiceControl").string(72).primary_key().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Event").int32().nullable().build(),
        Column::build("Arguments").string(255).nullable().build(),
        Column::build("Wait").int16().nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).unwrap();
    builder.create_table("CustomAction", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Type").int16().nullable().build(),
        Column::build("Source").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
    ]).unwrap();
    builder.create_table("InstallExecuteSequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).unwrap();
    builder.create_table("InstallUISequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).unwrap();
    builder.create_table("Upgrade", vec![
        Column::build("UpgradeCode").string(38).primary_key().build(),
        Column::build("VersionMin").string(20).nullable().build(),
        Column::build("VersionMax").string(20).nullable().build(),
        Column::build("Language").string(20).nullable().build(),
        Column::build("Attributes").int32().nullable().build(),
    ]).unwrap();
    builder.create_table("LaunchCondition", vec![
        Column::build("Condition").string(255).primary_key().build(),
        Column::build("Description").string(255).nullable().build(),
    ]).unwrap();

    // Populate with data (matching compiler)
    builder.insert_rows("Property", vec![
        vec![Value::from("ProductCode"), Value::from("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}")],
        vec![Value::from("UpgradeCode"), Value::from("{11111111-2222-3333-4444-555555555555}")],
        vec![Value::from("ProductName"), Value::from("Sample App")],
        vec![Value::from("Manufacturer"), Value::from("Velocity Team")],
        vec![Value::from("ProductVersion"), Value::from("1.0.0")],
        vec![Value::from("ProductLanguage"), Value::from("1033")],
        vec![Value::from("ALLUSERS"), Value::from("1")],
    ]).unwrap();
    builder.insert_rows("Directory", vec![
        vec![Value::from("TARGETDIR"), Value::Null, Value::from("SourceDir")],
        vec![Value::from("ProgramFilesFolder"), Value::from("TARGETDIR"), Value::from("PFiles")],
        vec![Value::from("INSTALLDIR"), Value::from("ProgramFilesFolder"), Value::from("SampleApp")],
    ]).unwrap();
    builder.insert_rows("Component", vec![
        vec![Value::from("comp0"), Value::Null, Value::from("INSTALLDIR"), Value::Int(0), Value::Null, Value::from("file_0")],
    ]).unwrap();
    builder.insert_rows("File", vec![
        vec![Value::from("file_0"), Value::from("comp0"), Value::from("sample.exe"), Value::Int(1024), Value::Null, Value::Null, Value::Null, Value::Int(1)],
    ]).unwrap();
    builder.insert_rows("Media", vec![
        vec![Value::Int(1), Value::Int(1), Value::Null, Value::Null, Value::from("#cab0.cab"), Value::Null],
    ]).unwrap();
    builder.insert_rows("Feature", vec![
        vec![Value::from("MainFeature"), Value::Null, Value::from("Complete"), Value::from("Full install"), Value::Int(1), Value::Int(1), Value::Null, Value::Null],
    ]).unwrap();
    builder.insert_rows("FeatureComponents", vec![
        vec![Value::from("MainFeature"), Value::from("comp0")],
    ]).unwrap();
    builder.insert_rows("InstallExecuteSequence", vec![
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(800)],
        vec![Value::from("CostFinalize"), Value::Null, Value::Int(900)],
        vec![Value::from("InstallValidate"), Value::Null, Value::Int(1400)],
        vec![Value::from("InstallInitialize"), Value::Null, Value::Int(1500)],
        vec![Value::from("InstallFinalize"), Value::Null, Value::Int(6600)],
    ]).unwrap();
    builder.insert_rows("InstallUISequence", vec![
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(800)],
        vec![Value::from("CostFinalize"), Value::Null, Value::Int(900)],
    ]).unwrap();
    builder.insert_rows("Upgrade", vec![
        vec![Value::from("{11111111-2222-3333-4444-555555555555}"), Value::from("1.0.0"), Value::from("1.0.0"), Value::Null, Value::Int(256)],
    ]).unwrap();
    // Registry, Shortcut, Icon, Environment, ServiceInstall, ServiceControl, CustomAction, LaunchCondition: EMPTY

    let msi_data = builder.build().unwrap();
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-app/output");
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = out_dir.join("test_full_tables.msi");
    std::fs::write(&path, &msi_data).unwrap();
    eprintln!("Wrote {} bytes to {}", msi_data.len(), path.display());

    // Test with msi crate
    let cursor = std::io::Cursor::new(&msi_data);
    match msi::Package::open(cursor) {
        Ok(pkg) => {
            eprintln!("msi crate: OK - {} tables", pkg.tables().count());
        }
        Err(e) => eprintln!("msi crate: FAILED: {}", e),
    }

    // Test with msiexec
    let msi_path = std::fs::canonicalize(&path).unwrap();
    let msi_str = msi_path.to_str().unwrap().trim_start_matches(r"\\?\").to_string();
    let status = Command::new("msiexec")
        .args(&["/i", &msi_str, "/qn", "/norestart"])
        .status()
        .expect("Failed to run msiexec");
    eprintln!("msiexec exit code: {}", status.code().unwrap_or(-1));

    // Uninstall
    let status2 = Command::new("msiexec")
        .args(&["/x", "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}", "/qn", "/norestart"])
        .status();
    if let Ok(s) = status2 {
        eprintln!("uninstall exit code: {}", s.code().unwrap_or(-1));
    }
}
