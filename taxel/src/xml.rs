use std::{fs, path::Path};

pub fn read(xml_file: &str) -> Result<String, anyhow::Error> {
    let xml_path = Path::new(xml_file);
    println!("Reading xml file '{}'", xml_path.display());
    let xml = fs::read_to_string(xml_path)?;
    Ok(xml)
}
