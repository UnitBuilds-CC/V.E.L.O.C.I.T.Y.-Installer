/// Identify each stream by encoding and decode _Tables against string pool
use std::io::{Cursor, Read};

fn main() {
    let msi_path = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\test_minimal\output\installer.msi";
    let data = std::fs::read(msi_path).expect("read MSI");
    let cursor = Cursor::new(&data);
    let mut comp = cfb::CompoundFile::open(cursor).expect("open CFB");

    let stream_info: Vec<_> = comp.walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_path_buf(), e.name().to_string()))
        .collect();

    // Known encoded names
    let tables_enc = velocity_msi::encode_stream_name("_Tables", true);
    let cols_enc = velocity_msi::encode_stream_name("_Columns", true);
    let val_enc = velocity_msi::encode_stream_name("_Validation", true);
    let pool_enc = velocity_msi::encode_stream_name("_StringPool", true);
    let data_enc = velocity_msi::encode_stream_name("_StringData", true);
    let prop_enc = velocity_msi::encode_stream_name("Property", true);
    let dir_enc = velocity_msi::encode_stream_name("Directory", true);
    let comp_enc = velocity_msi::encode_stream_name("Component", true);
    let file_enc = velocity_msi::encode_stream_name("File", true);
    let media_enc = velocity_msi::encode_stream_name("Media", true);
    let feat_enc = velocity_msi::encode_stream_name("Feature", true);
    let fc_enc = velocity_msi::encode_stream_name("FeatureComponents", true);
    let ies_enc = velocity_msi::encode_stream_name("InstallExecuteSequence", true);
    let ius_enc = velocity_msi::encode_stream_name("InstallUISequence", true);

    eprintln!("Encoded names:");
    eprintln!("  _Tables = {}", hex_encode(&tables_enc));
    eprintln!("  _Columns = {}", hex_encode(&cols_enc));
    eprintln!("  Property = {}", hex_encode(&prop_enc));
    eprintln!();

    // Read all streams with identification
    for (path, name) in &stream_info {
        let mut s = comp.open_stream(path).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();

        let identity = match name.as_str() {
            n if n == tables_enc => "_Tables",
            n if n == cols_enc => "_Columns",
            n if n == val_enc => "_Validation",
            n if n == pool_enc => "_StringPool",
            n if n == data_enc => "_StringData",
            n if n == prop_enc => "Property",
            n if n == dir_enc => "Directory",
            n if n == comp_enc => "Component",
            n if n == file_enc => "File",
            n if n == media_enc => "Media",
            n if n == feat_enc => "Feature",
            n if n == fc_enc => "FeatureComponents",
            n if n == ies_enc => "InstallExecuteSequence",
            n if n == ius_enc => "InstallUISequence",
            _ => "???",
        };

        eprintln!("Stream: {} ({} bytes) = {}", hex_encode(name), buf.len(), identity);

        if identity == "_Tables" {
            eprintln!("  _Tables hex: {:02x?}", &buf);
        }
        if identity == "Property" {
            eprintln!("  Property hex: {:02x?}", &buf);
        }
    }
}

fn hex_encode(name: &str) -> String {
    name.encode_utf16()
        .map(|c| format!("{:04x}", c))
        .collect::<Vec<_>>()
        .join(" ")
}
