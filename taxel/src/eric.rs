use crate::{
    certificate::Certificate,
    config::{CertificateConfig, EricConfig, PrintConfig},
    processing::process,
    response_buffer::ResponseBuffer,
    EricResponse, ErrorCode, ProcessingFlag,
};
use anyhow::{anyhow, Context};
use std::{
    env::{self, current_dir},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use taxel_bindings::{EricBeende, EricDekodiereDaten, EricInitialisiere};
use taxel_util::ToCString;

#[derive(Debug)]
pub struct Eric;

impl Eric {
    pub fn new() -> Result<Self, anyhow::Error> {
        println!("Initializing eric");

        let plugin_path =
            env::var("PLUGIN_PATH").context("Missing environment variable 'PLUGIN_PATH'")?;
        let plugin_path = Path::new(&plugin_path);
        let log_path = env::current_dir().context("Can't get current directory")?;

        println!("Setting plugin path '{}'", plugin_path.display());
        println!("Setting log path '{}'", log_path.display());
        println!("Logging to '{}'", log_path.join("eric.log").display());

        let plugin_path = plugin_path.try_to_cstring()?;
        let log_path = log_path.try_to_cstring()?;

        let error_code = unsafe { EricInitialisiere(plugin_path.as_ptr(), log_path.as_ptr()) };

        match error_code {
            x if x == ErrorCode::ERIC_OK as i32 => Ok(Eric),
            error_code => Err(anyhow!("Can't init eric: {}", error_code)),
        }
    }

    pub fn validate(
        &self,
        xml: String,
        type_version: String,
    ) -> Result<EricResponse, anyhow::Error> {
        process(
            xml,
            type_version,
            ProcessingFlag::Validate,
            None,
            None,
            None,
        )
    }

    pub fn validate_and_print(
        &self,
        xml: String,
        type_version: String,
        print_config: PrintConfig,
    ) -> Result<EricResponse, anyhow::Error> {
        process(
            xml,
            type_version,
            ProcessingFlag::Print,
            Some(print_config),
            None,
            None,
        )
    }

    pub fn send(
        &self,
        xml: String,
        type_version: String,
        config: CertificateConfig,
    ) -> Result<EricResponse, anyhow::Error> {
        process(
            xml,
            type_version,
            ProcessingFlag::Send,
            None,
            Some(config),
            None,
        )
    }

    pub fn send_and_print(
        &self,
        xml: String,
        type_version: String,
        certificate_config: CertificateConfig,
        print_config: Option<PrintConfig>,
    ) -> Result<EricResponse, anyhow::Error> {
        process(
            xml,
            type_version,
            ProcessingFlag::Send,
            print_config,
            Some(certificate_config),
            None,
        )
    }

    pub fn log(&self, response: &EricResponse) -> Result<(), anyhow::Error> {
        println!("Response code: {}", response.error_code);

        let current_dir = current_dir()?;

        let validation_response_path = current_dir.join("validation_response.xml");
        let server_response_path = current_dir.join("server_response.xml");

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

    pub fn decrypt(
        &self,
        encrypted_file: &str,
        config: CertificateConfig,
    ) -> Result<i32, anyhow::Error> {
        let encrypted_data = encrypted_file.try_to_cstring()?;
        let response_buffer = ResponseBuffer::new()?;
        let certificate = Certificate::new(config)?;

        let error_code = unsafe {
            EricDekodiereDaten(
                certificate.as_value(),
                certificate.password.as_ptr(),
                encrypted_data.as_ptr(),
                response_buffer.as_ptr(),
            )
        };

        Ok(error_code)
    }
}

impl Drop for Eric {
    fn drop(&mut self) {
        println!("Closing eric");

        // TODO: drop ResponseBuffer and Certificate before dropping Eric

        let error_code = unsafe { EricBeende() };

        match error_code {
            x if x == ErrorCode::ERIC_OK as i32 => (),
            error_code => panic!("Can't close eric: {}", error_code),
        }
    }
}

pub fn configure() -> Result<EricConfig, anyhow::Error> {
    println!("Configuring eric");

    let plugin_path =
        env::var("PLUGIN_PATH").context("Missing environment variable 'PLUGIN_PATH'")?;
    let plugin_path = Path::new(&plugin_path);

    let log_path = env::current_dir().context("Can't get current directory")?;

    println!("Setting plugin path '{}'", plugin_path.display());
    println!("Setting log path '{}'", log_path.display());
    println!("Logging to '{}'", log_path.join("eric.log").display());

    let config = EricConfig::new(plugin_path.into(), log_path);

    Ok(config)
}

pub fn init(plugin_path: PathBuf, log_path: PathBuf) -> Result<(), anyhow::Error> {
    println!("Initializing eric");

    let plugin_path = plugin_path.try_to_cstring()?;
    let log_path = log_path.try_to_cstring()?;

    let error_code = unsafe { EricInitialisiere(plugin_path.as_ptr(), log_path.as_ptr()) };

    match error_code {
        x if x == ErrorCode::ERIC_OK as i32 => Ok(()),
        error_code => Err(anyhow!("Can't init eric: {}", error_code)),
    }
}

// TODO: implement EricEntladePlugins
pub fn close() -> Result<(), anyhow::Error> {
    println!("Closing eric");

    let error_code = unsafe { EricBeende() };

    match error_code {
        x if x == ErrorCode::ERIC_OK as i32 => Ok(()),
        error_code => Err(anyhow!("Can't close eric: {}", error_code)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;
    use std::{
        env::current_dir,
        fs::{self, File},
        io::Write,
    };

    #[test]
    fn test_new() {
        let res = Eric::new();

        println!("{:#?}", res);
        assert!(res.is_ok());

        // eric goes out of scope here and is closed automatically (via implementation of Drop)
    }

    #[test]
    fn test_validate() {
        let current_dir = current_dir().unwrap();
        println!("current dir: {:#?}", current_dir);

        let xml_path = "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml";
        println!("Reading xml file '{}'", xml_path);
        let xml = fs::read_to_string(xml_path).unwrap();

        let version = "Bilanz_6.5".to_string();

        let eric = Eric::new().unwrap();

        let res = eric.validate(xml, version);

        println!("{:#?}", res);
        assert!(res.is_ok());

        let response = res.unwrap();

        eric.log(&response).unwrap();

        assert_eq!(response.error_code, ErrorCode::ERIC_OK as i32);

        let doc = Document::parse(&response.validation_response).unwrap();
        println!("Doc: {:#?}", doc);
        let node = doc.descendants().find(|node| node.has_tag_name("Erfolg"));
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.tag_name().name(), "Erfolg");

        assert!(response.server_response.is_empty());
    }

    #[test]
    fn test_validate_and_print() {
        todo!()
    }

    #[test]
    fn test_send() {
        todo!()
    }

    #[test]
    fn test_send_and_print() {
        todo!()
    }
}
