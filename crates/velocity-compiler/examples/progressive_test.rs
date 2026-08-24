/// Progressive MSI build test: add tables one at a time, test each with msiexec
use velocity_msi::{Column, MsiBuilder, Value};

fn main() {
    let stages: Vec<(&str, fn() -> Result<Vec<u8>, String>)> = vec![
        ("1_Property", stage_property),
        ("2_Directory", stage_directory),
        ("3_Component", stage_component),
        ("4_File", stage_file),
        ("5_Media", stage_media),
        ("6_Feature", stage_feature),
        ("7_FC", stage_fc),
        ("8_IES", stage_ies),
        ("9_IUS", stage_ius),
        ("10_EmptyTables", stage_empty_tables),
    ];

    for (name, builder_fn) in &stages {
        let data = match builder_fn() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{}: BUILD ERROR: {}", name, e);
                continue;
            }
        };
        let path = format!(r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\prog_{}.msi", name);
        std::fs::write(&path, &data).unwrap();

        let abs = std::fs::canonicalize(&path).unwrap();
        let p = abs.to_str().unwrap().trim_start_matches(r"\\?\");
        let output = std::process::Command::new("msiexec.exe")
            .args(&["/i", p, "/qn", "/norestart"])
            .output();
        let code = output.map(|o| o.status.code().unwrap_or(-1)).unwrap_or(-1);
        let status = match code {
            0 => "SUCCESS",
            1620 => "CANNOT OPEN",
            1613 => "CANNOT INSTALL",
            1603 => "FATAL ERROR",
            1708 => "INSTALL FAILED",
            _ => "OTHER",
        };
        eprintln!("{:20}: exit={:5} ({:15})  {:6} bytes", name, code, status, data.len());
    }
}

fn new_builder() -> MsiBuilder {
    let mut b = MsiBuilder::new();
    b.set_title("Progressive Test");
    b.set_author("Test");
    b.set_template("x64", 1033);
    b
}

fn add_prop(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("Property", vec![
        vec![Value::from("ProductName"), Value::from("Test")],
        vec![Value::from("ProductVersion"), Value::from("1.0")],
        vec![Value::from("Manufacturer"), Value::from("Test")],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_dir(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("Directory", vec![
        Column::build("Directory").string(72).primary_key().build(),
        Column::build("Directory_Parent").string(72).nullable().build(),
        Column::build("DefaultDir").string(255).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("Directory", vec![
        vec![Value::from("TARGETDIR"), Value::Null, Value::from("SourceDir")],
        vec![Value::from("ProgramFiles64Folder"), Value::from("TARGETDIR"), Value::from("PFiles")],
        vec![Value::from("INSTALLDIR"), Value::from("ProgramFiles64Folder"), Value::from("TestApp:TestApp")],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_comp(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("Component", vec![
        Column::build("Component").string(72).primary_key().build(),
        Column::build("ComponentId").string(38).nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("KeyPath").string(72).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("Component", vec![
        vec![Value::from("comp_0"), Value::from("{12345678-1234-1234-1234-123456789012}"),
             Value::from("INSTALLDIR"), Value::Int(0), Value::Null, Value::from("file_0")],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_file(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("File", vec![
        Column::build("File").string(72).primary_key().build(),
        Column::build("Component_").string(72).build(),
        Column::build("FileName").string(255).build(),
        Column::build("FileSize").int32().build(),
        Column::build("Version").string(72).nullable().build(),
        Column::build("Language").int16().nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Sequence").int32().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("File", vec![
        vec![Value::from("file_0"), Value::from("comp_0"), Value::from("test.txt"),
             Value::Int(100), Value::Null, Value::Null, Value::Int(0), Value::Int(1)],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_media(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("Media", vec![
        Column::build("DiskId").int16().primary_key().build(),
        Column::build("LastSequence").int32().build(),
        Column::build("DiskPrompt").string(255).nullable().build(),
        Column::build("VolumeLabel").string(32).nullable().build(),
        Column::build("Cabinet").string(255).nullable().build(),
        Column::build("Source").string(72).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("Media", vec![
        vec![Value::Int(1), Value::Int(1), Value::Null, Value::Null,
             Value::from("#Test.cab"), Value::Null],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_feature(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("Feature", vec![
        Column::build("Feature").string(38).primary_key().build(),
        Column::build("Feature_Parent").string(38).nullable().build(),
        Column::build("Title").string(64).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
        Column::build("Display").int16().nullable().build(),
        Column::build("Level").int16().nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("Feature", vec![
        vec![Value::from("Complete"), Value::Null, Value::from("Test"), Value::from("Test"),
             Value::Int(1), Value::Int(1), Value::from("INSTALLDIR"), Value::Int(0)],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_fc(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("FeatureComponents", vec![
        Column::build("Feature_").string(38).primary_key().build(),
        Column::build("Component_").string(72).primary_key().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("FeatureComponents", vec![
        vec![Value::from("Complete"), Value::from("comp_0")],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_ies(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("InstallExecuteSequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("InstallExecuteSequence", vec![
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(120)],
        vec![Value::from("InstallFinalize"), Value::Null, Value::Int(400)],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

fn add_ius(b: &mut MsiBuilder) -> Result<(), String> {
    b.create_table("InstallUISequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.insert_rows("InstallUISequence", vec![
        vec![Value::from("ShowLog"), Value::Null, Value::Int(-1)],
    ]).map_err(|e| e.to_string())?;
    Ok(())
}

// === Stage functions: each builds from scratch ===

fn stage_property() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_directory() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_component() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_file() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_media() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_feature() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    add_feature(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_fc() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    add_feature(&mut b)?;
    add_fc(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_ies() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    add_feature(&mut b)?;
    add_fc(&mut b)?;
    add_ies(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_ius() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    add_feature(&mut b)?;
    add_fc(&mut b)?;
    add_ies(&mut b)?;
    add_ius(&mut b)?;
    b.build().map_err(|e| e.to_string())
}

fn stage_empty_tables() -> Result<Vec<u8>, String> {
    let mut b = new_builder();
    add_prop(&mut b)?;
    add_dir(&mut b)?;
    add_comp(&mut b)?;
    add_file(&mut b)?;
    add_media(&mut b)?;
    add_feature(&mut b)?;
    add_fc(&mut b)?;
    add_ies(&mut b)?;
    add_ius(&mut b)?;
    // Add empty tables (like the compiler does - they get filtered out by build())
    b.create_table("CustomAction", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Type").int16().nullable().build(),
        Column::build("Source").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.create_table("Registry", vec![
        Column::build("Registry").string(72).primary_key().build(),
        Column::build("Root").int16().nullable().build(),
        Column::build("Key").string(255).nullable().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.create_table("Upgrade", vec![
        Column::build("UpgradeCode").string(38).primary_key().build(),
        Column::build("VersionMin").string(20).nullable().build(),
        Column::build("VersionMax").string(20).nullable().build(),
        Column::build("Language").string(20).nullable().build(),
        Column::build("Attributes").int32().nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.create_table("LaunchCondition", vec![
        Column::build("Condition").string(255).primary_key().build(),
        Column::build("Description").string(255).nullable().build(),
    ]).map_err(|e| e.to_string())?;
    b.build().map_err(|e| e.to_string())
}
