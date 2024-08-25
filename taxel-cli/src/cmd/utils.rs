use eric_sdk::EricResponse;
use std::{fs::File, io::Write, path::Path};

pub fn log_response(log_path: &Path, response: &EricResponse) -> Result<(), anyhow::Error> {
    println!("Response code: {}", response.error_code);

    let validation_response_path = log_path.join("validation_response.xml");
    let server_response_path = log_path.join("server_response.xml");

    if !response.validation_response.is_empty() {
        println!(
            "Logging validation result to '{}'",
            validation_response_path.display()
        );
    }

    if !response.server_response.is_empty() {
        println!(
            "Logging server reponse to '{}'",
            server_response_path.display()
        );
    }

    let mut validation_response_file = File::create(validation_response_path)?;
    let mut server_response_file = File::create(server_response_path)?;

    validation_response_file.write_all(response.validation_response.as_bytes())?;
    server_response_file.write_all(response.server_response.as_bytes())?;

    Ok(())
}
