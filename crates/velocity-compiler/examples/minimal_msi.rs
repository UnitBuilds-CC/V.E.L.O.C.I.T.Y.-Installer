use velocity_msi::{Column, MsiBuilder, Value};

fn main() {
    // Create a minimal MSI with one table
    let mut builder = MsiBuilder::new();
    builder.set_title("Test");
    builder.set_author("Test");
    builder.set_template("x64", 1033);

    // Create Property table
    builder
        .create_table(
            "Property",
            vec![
                Column::build("Property").string(72).primary_key().build(),
                Column::build("Value").string(255).nullable().build(),
            ],
        )
        .unwrap();
    builder
        .insert_rows(
            "Property",
            vec![vec![
                Value::from("ProductName"),
                Value::from("Test Product"),
            ]],
        )
        .unwrap();

    let data = builder.build().unwrap();
    std::fs::write("target/test_inspect.msi", &data).unwrap();
    println!("Created MSI: {} bytes", data.len());

    // Verify OLE2 header
    assert_eq!(&data[0..4], &[0xD0, 0xCF, 0x11, 0xE0]);
    println!("OLE2 header valid: D0 CF 11 E0");
}
