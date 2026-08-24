//! Test: does using the `cab` crate for the cabinet cause msiexec 1620?
//! This test replicates test_replicate_exact but uses the `cab` crate (like the compiler).
use velocity_msi::{MsiBuilder, Column, Value, CabinetFile, build_cabinet};
use std::process::Command;
use std::io::Cursor;

fn main() {
    println!("=== Test: cab crate vs velocity_msi cabinet ===\n");

    // Test 1: Using velocity_msi::build_cabinet (known to produce 1603 = valid)
    println!("--- Test 1: velocity_msi cabinet ---");
    let msi1 = build_test_msi(false);
    std::fs::write("test_vel_cab.msi", &msi1).unwrap();
    let code1 = test_msi("test_vel_cab.msi");
    println!("Result: exit code {}\n", code1);

    // Test 2: Using cab crate (like the compiler - may produce 1620)
    println!("--- Test 2: cab crate cabinet ---");
    let msi2 = build_test_msi(true);
    std::fs::write("test_cab_crate.msi", &msi2).unwrap();
    let code2 = test_msi("test_cab_crate.msi");
    println!("Result: exit code {}\n", code2);

    if code1 != 1620 && code2 == 1620 {
        println!("ROOT CAUSE CONFIRMED: cab crate cabinet causes 1620!");
    } else if code1 == 1620 && code2 == 1620 {
        println!("Both fail - issue is NOT the cabinet crate");
    } else {
        println!("Both work - issue is elsewhere in compiler data flow");
    }
}

fn test_msi(path: &str) -> i32 {
    let log = format!("{}.log", path);
    let status = Command::new("msiexec.exe")
        .args(&["/i", path, "/qn", "/l*v", &log])
        .status()
        .unwrap();
    let code = status.code().unwrap_or(-1);
    
    // Check log for product info
    if let Ok(content) = std::fs::read_to_string(&log) {
        for line in content.lines() {
            if line.contains("Product Name") || line.contains("1620") {
                println!("  LOG: {}", line.trim());
            }
        }
    }
    
    // Uninstall if successful
    if code == 0 {
        if let Ok(content) = std::fs::read_to_string(&log) {
            for line in content.lines() {
                if line.contains("ProductCode") {
                    // Extract product code for uninstall
                    break;
                }
            }
        }
    }
    
    code
}

fn build_test_msi(use_cab_crate: bool) -> Vec<u8> {
    let mut b = MsiBuilder::new();
    b.set_title("Test App Installer");
    b.set_author("Velocity Team");
    b.set_subject("Test App v1.0.0");
    b.set_comments("Test installer package");
    b.set_template("x64", 1033);

    // Create ALL tables (same as compiler)
    b.create_table("Property", vec![
        Column::build("Property").string(72).primary_key().build(),
        Column::build("Value").string(255).nullable().build(),
    ]).unwrap();
    b.create_table("Directory", vec![
        Column::build("Directory").string(72).primary_key().build(),
        Column::build("Directory_Parent").string(72).nullable().build(),
        Column::build("DefaultDir").string(255).nullable().build(),
    ]).unwrap();
    b.create_table("Component", vec![
        Column::build("Component").string(72).primary_key().build(),
        Column::build("ComponentId").string(38).nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("KeyPath").string(72).nullable().build(),
    ]).unwrap();
    b.create_table("File", vec![
        Column::build("File").string(72).primary_key().build(),
        Column::build("Component_").string(72).build(),
        Column::build("FileName").string(255).build(),
        Column::build("FileSize").int32().build(),
        Column::build("Version").string(72).nullable().build(),
        Column::build("Language").int16().nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
        Column::build("Sequence").int32().build(),
    ]).unwrap();
    b.create_table("Media", vec![
        Column::build("DiskId").int16().primary_key().build(),
        Column::build("LastSequence").int32().build(),
        Column::build("DiskPrompt").string(255).nullable().build(),
        Column::build("VolumeLabel").string(32).nullable().build(),
        Column::build("Cabinet").string(255).nullable().build(),
        Column::build("Source").string(72).nullable().build(),
    ]).unwrap();
    b.create_table("Feature", vec![
        Column::build("Feature").string(38).primary_key().build(),
        Column::build("Feature_Parent").string(38).nullable().build(),
        Column::build("Title").string(64).nullable().build(),
        Column::build("Description").string(255).nullable().build(),
        Column::build("Display").int16().nullable().build(),
        Column::build("Level").int16().nullable().build(),
        Column::build("Directory_").string(72).nullable().build(),
        Column::build("Attributes").int16().nullable().build(),
    ]).unwrap();
    b.create_table("FeatureComponents", vec![
        Column::build("Feature_").string(38).primary_key().build(),
        Column::build("Component_").string(72).primary_key().build(),
    ]).unwrap();
    b.create_table("Registry", vec![
        Column::build("Registry").string(72).primary_key().build(),
        Column::build("Root").int16().nullable().build(),
        Column::build("Key").string(255).nullable().build(),
        Column::build("Name").string(255).nullable().build(),
        Column::build("Value").string(255).nullable().build(),
        Column::build("Component_").string(72).nullable().build(),
    ]).unwrap();
    b.create_table("CustomAction", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Type").int16().nullable().build(),
        Column::build("Source").string(72).nullable().build(),
        Column::build("Target").string(255).nullable().build(),
    ]).unwrap();
    b.create_table("InstallExecuteSequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).unwrap();
    b.create_table("InstallUISequence", vec![
        Column::build("Action").string(72).primary_key().build(),
        Column::build("Condition").string(255).nullable().build(),
        Column::build("Sequence").int16().nullable().build(),
    ]).unwrap();

    // Populate data
    let product_code = "{D5E0EEC4-CF5C-5D68-BAC7-AA26667F6C80}";
    let upgrade_code = "{7B44FAB1-58DD-5368-9B0C-338B5E7519DD}";

    // Properties
    let props: Vec<(&str, &str)> = vec![
        ("ProductCode", product_code),
        ("UpgradeCode", upgrade_code),
        ("ProductName", "Test App"),
        ("Manufacturer", "Velocity Team"),
        ("ProductVersion", "1.0.0"),
        ("ProductLanguage", "1033"),
        ("ALLUSERS", "1"),
    ];
    for (name, value) in props {
        b.insert_rows("Property", vec![vec![Value::from(name), Value::from(value)]]).unwrap();
    }

    // Directories
    b.insert_rows("Directory", vec![vec![
        Value::from("TARGETDIR"), Value::Null, Value::from("SourceDir"),
    ]]).unwrap();
    b.insert_rows("Directory", vec![vec![
        Value::from("ProgramFiles64Folder"), Value::from("TARGETDIR"), Value::from("PFiles"),
    ]]).unwrap();
    b.insert_rows("Directory", vec![vec![
        Value::from("INSTALLDIR"), Value::from("ProgramFiles64Folder"), Value::from("TestApp"),
    ]]).unwrap();

    // Files + Components (3 files for simplicity)
    let file_names = vec!["core.dll", "test-app.exe", "readme.txt"];
    let file_sizes = vec![1024, 2048, 512];
    
    for (i, fname) in file_names.iter().enumerate() {
        let file_id = format!("file_{}", i);
        let comp_id = format!("comp_{}", i);
        let guid = format!("{{{:08X}-0000-0000-0000-{:012X}}}", i + 0x10000000, i);
        
        b.insert_rows("Component", vec![vec![
            Value::from(comp_id.as_str()),
            Value::from(guid.as_str()),
            Value::from("INSTALLDIR"),
            Value::Int(0),
            Value::Null,
            Value::Null,
        ]]).unwrap();
        
        b.insert_rows("File", vec![vec![
            Value::from(file_id.as_str()),
            Value::from(comp_id.as_str()),
            Value::from(*fname),
            Value::Int(file_sizes[i] as i32),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Int(i as i32 + 1),
        ]]).unwrap();
        
        b.insert_rows("FeatureComponents", vec![vec![
            Value::from("Complete"),
            Value::from(comp_id.as_str()),
        ]]).unwrap();
    }

    // Feature
    b.insert_rows("Feature", vec![vec![
        Value::from("Complete"),
        Value::Null,
        Value::from("Test App Setup"),
        Value::from("Complete installation"),
        Value::Int(1),
        Value::Int(1),
        Value::from("INSTALLDIR"),
        Value::Int(0),
    ]]).unwrap();

    // Media
    b.insert_rows("Media", vec![vec![
        Value::Int(1),
        Value::Int(file_names.len() as i32),
        Value::Null,
        Value::Null,
        Value::from("Velocity.cab"),
        Value::Null,
    ]]).unwrap();

    // Registry (1 entry)
    b.insert_rows("Component", vec![vec![
        Value::from("comp_reg_0"),
        Value::from("{20000000-0000-0000-0000-000000000000}"),
        Value::from("INSTALLDIR"),
        Value::Int(0),
        Value::Null,
        Value::Null,
    ]]).unwrap();
    b.insert_rows("FeatureComponents", vec![vec![
        Value::from("Complete"),
        Value::from("comp_reg_0"),
    ]]).unwrap();
    b.insert_rows("Registry", vec![vec![
        Value::from("reg_0"),
        Value::Int(2),
        Value::from("Software\\TestApp"),
        Value::from("InstallPath"),
        Value::from("[INSTALLDIR]"),
        Value::from("comp_reg_0"),
    ]]).unwrap();

    // Install sequences
    let exec_actions: Vec<(&str, Option<&str>, i32)> = vec![
        ("ActionText", None, 20),
        ("ExecuteAction", None, 1300),
    ];
    for (action, cond, seq) in exec_actions {
        let c: Value = match cond { Some(v) => Value::from(v), None => Value::Null };
        b.insert_rows("InstallExecuteSequence", vec![vec![
            Value::from(action), c, Value::Int(seq),
        ]]).unwrap();
    }
    let ui_actions: Vec<(&str, Option<&str>, i32)> = vec![
        ("ActionText", None, 20),
        ("ExecuteAction", None, 1300),
    ];
    for (action, cond, seq) in ui_actions {
        let c: Value = match cond { Some(v) => Value::from(v), None => Value::Null };
        b.insert_rows("InstallUISequence", vec![vec![
            Value::from(action), c, Value::Int(seq),
        ]]).unwrap();
    }

    // Cabinet
    if use_cab_crate {
        println!("  Using cab crate for cabinet");
        let cab_bytes = build_cab_crate(&file_names, &file_sizes);
        println!("  Cabinet size: {} bytes", cab_bytes.len());
        b.add_stream("Velocity.cab".to_string(), cab_bytes);
    } else {
        println!("  Using velocity_msi::build_cabinet");
        let mut cab_files = Vec::new();
        for (i, _fname) in file_names.iter().enumerate() {
            cab_files.push(CabinetFile {
                name: format!("file_{}", i),
                data: vec![0u8; file_sizes[i]],
            });
        }
        let cabinet = build_cabinet(&cab_files);
        println!("  Cabinet size: {} bytes", cabinet.len());
        b.add_stream("Velocity.cab".to_string(), cabinet);
    }

    b.build().unwrap()
}

/// Build cabinet using the `cab` crate (matching compiler's approach)
fn build_cab_crate(file_names: &[&str], file_sizes: &[usize]) -> Vec<u8> {
    let mut cab_data = Cursor::new(Vec::new());
    {
        let mut cab_builder = cab::CabinetBuilder::new();
        let folder = cab_builder.add_folder(cab::CompressionType::MsZip);
        for name in file_names {
            folder.add_file(name.to_string());
        }
        let mut cab_writer = cab_builder.build(&mut cab_data).unwrap();
        for (i, _name) in file_names.iter().enumerate() {
            let mut writer = cab_writer.next_file().unwrap().unwrap();
            // Write dummy data (same size as file_sizes[i])
            let data = vec![0u8; file_sizes[i]];
            std::io::Write::write_all(&mut writer, &data).unwrap();
        }
        cab_writer.finish().unwrap();
    }
    cab_data.into_inner()
}
