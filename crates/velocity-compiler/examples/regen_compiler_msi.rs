/// Regenerate compiler MSI and dump its _Tables/_Columns to diagnose duplicates
use std::io::Read;
use velocity_msi::{MsiBuilder, Column, Value};

fn main() {
    let mut builder = MsiBuilder::new();
    builder.set_title("Sample App Installer");
    builder.set_author("Velocity Team");
    builder.set_subject("Sample App v1.0.0");
    builder.set_comments("Sample app installer package");
    builder.set_template("x64", 1033);

    // Create same tables as the compiler
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
    // Empty tables that should be skipped
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

    // Populate with data (matching what the compiler does)
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
        vec![Value::from("INSTALLDIR"), Value::from("ProgramFilesFolder"), Value::from("Sample App")],
    ]).unwrap();

    builder.insert_rows("Component", vec![
        vec![Value::from("comp0"), Value::Null, Value::from("INSTALLDIR"), Value::Int(0), Value::Null, Value::from("file_0")],
    ]).unwrap();

    builder.insert_rows("File", vec![
        vec![Value::from("file_0"), Value::from("comp0"), Value::from("sample-app.exe"), Value::Int(1024), Value::Null, Value::Null, Value::Null, Value::Int(1)],
    ]).unwrap();

    builder.insert_rows("Media", vec![
        vec![Value::Int(1), Value::Int(1), Value::Null, Value::Null, Value::from("#cab0.cab"), Value::Null],
    ]).unwrap();

    builder.insert_rows("Feature", vec![
        vec![Value::from("MainFeature"), Value::Null, Value::from("Complete"), Value::from("Full installation"), Value::Int(1), Value::Int(1), Value::Null, Value::Null],
    ]).unwrap();

    builder.insert_rows("FeatureComponents", vec![
        vec![Value::from("MainFeature"), Value::from("comp0")],
    ]).unwrap();

    builder.insert_rows("InstallExecuteSequence", vec![
        vec![Value::from("AppSearch"), Value::Null, Value::Int(100)],
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(800)],
        vec![Value::from("CostFinalize"), Value::Null, Value::Int(900)],
        vec![Value::from("InstallValidate"), Value::Null, Value::Int(1400)],
        vec![Value::from("InstallInitialize"), Value::Null, Value::Int(1500)],
        vec![Value::from("ProcessComponents"), Value::Null, Value::Int(1600)],
        vec![Value::from("InstallFiles"), Value::Null, Value::Int(4000)],
        vec![Value::from("InstallFinalize"), Value::Null, Value::Int(6600)],
    ]).unwrap();

    builder.insert_rows("InstallUISequence", vec![
        vec![Value::from("AppSearch"), Value::Null, Value::Int(100)],
        vec![Value::from("CostInitialize"), Value::Null, Value::Int(800)],
        vec![Value::from("CostFinalize"), Value::Null, Value::Int(900)],
    ]).unwrap();

    builder.insert_rows("Upgrade", vec![
        vec![Value::from("{11111111-2222-3333-4444-555555555555}"), Value::from("1.0.0"), Value::from("1.0.0"), Value::Null, Value::Int(256)],
    ]).unwrap();

    // CustomAction and LaunchCondition are empty (0 rows) → should be skipped

    // Build
    let msi_data = builder.build().unwrap();
    let out_path = "examples/sample-app/output/diag_test.msi";
    std::fs::write(out_path, &msi_data).unwrap();
    eprintln!("Wrote {} bytes to {}", msi_data.len(), out_path);

    // Now open and inspect
    let cursor = std::io::Cursor::new(&msi_data);
    let mut comp = cfb::CompoundFile::open(cursor).unwrap();

    let streams: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    // Decode string pool
    let pool = decode_string_pool(&mut comp, &streams);

    // Dump _Tables
    let tables_target = velocity_msi::encode_stream_name("_Tables", true);
    for (path, name) in &streams {
        if name == &tables_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            let row_count = buf.len() / 2; // 1 string column = 2 bytes per row
            eprintln!("\n_Tables: {} bytes, {} rows", buf.len(), row_count);
            for i in 0..row_count {
                let id = u16::from_le_bytes([buf[i*2], buf[i*2+1]]);
                let tname = if (id as usize) < pool.len() { &pool[id as usize] } else { "?" };
                eprintln!("  [{}] {}", i, tname);
            }
            break;
        }
    }

    // Dump _Columns
    let cols_target = velocity_msi::encode_stream_name("_Columns", true);
    for (path, name) in &streams {
        if name == &cols_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            let row_count = buf.len() / 10;
            eprintln!("\n_Columns: {} bytes, {} rows", buf.len(), row_count);

            // Column-major decode
            let mut off = 0;
            let mut table_ids = Vec::new();
            for _ in 0..row_count { table_ids.push(u16::from_le_bytes([buf[off], buf[off+1]])); off += 2; }
            let mut numbers = Vec::new();
            for _ in 0..row_count { let r = u16::from_le_bytes([buf[off], buf[off+1]]); numbers.push((r ^ 0x8000) as i16 as i32); off += 2; }
            let mut name_ids = Vec::new();
            for _ in 0..row_count { name_ids.push(u16::from_le_bytes([buf[off], buf[off+1]])); off += 2; }
            let mut types = Vec::new();
            for _ in 0..row_count { let r = u16::from_le_bytes([buf[off], buf[off+1]]); types.push((r ^ 0x8000) as i16 as i32); off += 2; }

            // Check for duplicates
            let mut seen = std::collections::HashMap::new();
            let mut current_table = String::new();
            for i in 0..row_count {
                let tname = if (table_ids[i] as usize) < pool.len() { pool[table_ids[i] as usize].as_str() } else { "?" };
                let cname = if (name_ids[i] as usize) < pool.len() { pool[name_ids[i] as usize].as_str() } else { "?" };
                if tname != current_table.as_str() {
                    current_table = tname.to_string();
                    eprintln!("\n  Table: {}", tname);
                }
                let key = format!("({}, {})", tname, numbers[i]);
                let count = seen.entry(key.clone()).or_insert(0);
                *count += 1;
                let dup = if *count > 1 { " *** DUPLICATE ***" } else { "" };
                eprintln!("    Col {}: {} type=0x{:04X}{}", numbers[i], cname, types[i] as u16, dup);
            }

            eprintln!("\n=== Duplicates ===");
            let mut has_dups = false;
            for (key, count) in &seen {
                if *count > 1 {
                    eprintln!("  {} appears {} times", key, count);
                    has_dups = true;
                }
            }
            if !has_dups {
                eprintln!("  No duplicates found!");
            }
            return;
        }
    }
    eprintln!("_Columns stream not found!");
}

fn decode_string_pool(comp: &mut cfb::CompoundFile<std::io::Cursor<&Vec<u8>>>, streams: &[(std::path::PathBuf, String)]) -> Vec<String> {
    let pool_target = velocity_msi::encode_stream_name("_StringPool", true);
    let data_target = velocity_msi::encode_stream_name("_StringData", true);
    let mut pool_data: Option<Vec<u8>> = None;
    let mut string_data: Option<Vec<u8>> = None;
    for (path, name) in streams {
        if name == &pool_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            pool_data = Some(buf);
        }
        if name == &data_target {
            let mut s = comp.open_stream(path).unwrap();
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).unwrap();
            string_data = Some(buf);
        }
    }
    let pool_data = pool_data.unwrap();
    let string_data = string_data.unwrap();
    let mut offset = 4;
    let mut entries = Vec::new();
    while offset + 4 <= pool_data.len() {
        let len = u16::from_le_bytes([pool_data[offset], pool_data[offset+1]]);
        entries.push(len as usize);
        offset += 4;
    }
    let mut strings = vec![String::new()];
    let mut data_off = 0;
    for len in &entries {
        if data_off + len > string_data.len() { break; }
        let s = String::from_utf8_lossy(&string_data[data_off..data_off+*len]).to_string();
        strings.push(s);
        data_off += len;
    }
    strings
}
