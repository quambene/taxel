use anyhow::{anyhow, Context};
use std::{
    env::{self, current_dir},
    ffi::CStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    ptr,
};
use taxel_bindings::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang, EricBeende,
    EricInitialisiere, EricRueckgabepufferErzeugen, EricRueckgabepufferFreigeben,
    EricRueckgabepufferInhalt,
};
use taxel_util::ToCString;
use crate::ErrorCode;

#[derive(Debug)]
pub enum ProcessingFlag {
    Validate = 2,
    Send = 4,
    Print = 32,
    CheckHints = 128,
}

#[derive(Debug)]
pub struct Config {
    pub plugin_path: PathBuf,
    pub log_path: PathBuf,
}

impl Config {
    pub fn new(plugin_path: PathBuf, log_path: PathBuf) -> Self {
        Self {
            plugin_path,
            log_path,
        }
    }
}

#[derive(Debug)]
pub struct EricResponse {
    pub error_code: i32,
    pub validation_response: String,
    pub server_response: String,
}

impl EricResponse {
    pub fn new(error_code: i32, validation_response: String, server_response: String) -> Self {
        Self {
            error_code,
            validation_response,
            server_response,
        }
    }
}

pub fn configure() -> Result<Config, anyhow::Error> {
    println!("Configuring eric");

    let plugin_path =
        env::var("PLUGIN_PATH").context("Missing environment variable 'PLUGIN_PATH'")?;
    let plugin_path = Path::new(&plugin_path);

    let log_path = env::current_dir().context("Can't get current directory")?;

    println!("Setting plugin path '{}'", plugin_path.display());
    println!("Setting log path '{}'", log_path.display());
    println!("Logging to '{}'", log_path.join("eric.log").display());

    let config = Config::new(plugin_path.into(), log_path);

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

// TODO: implement drop for Eric
pub fn close() -> Result<(), anyhow::Error> {
    println!("Closing eric");

    let error_code = unsafe { EricBeende() };

    match error_code {
        x if x == ErrorCode::ERIC_OK as i32 => Ok(()),
        error_code => Err(anyhow!("Can't close eric: {}", error_code)),
    }
}

pub fn log(response: EricResponse) -> Result<(), anyhow::Error> {
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

    let mut validation_response_file = File::create(validation_response_path).unwrap();
    let mut server_response_file = File::create(server_response_path).unwrap();

    validation_response_file.write_all(response.validation_response.as_bytes())?;
    server_response_file.write_all(response.server_response.as_bytes())?;

    Ok(())
}

pub fn read(xml_file: &str) -> Result<String, anyhow::Error> {
    let xml_path = Path::new(xml_file);
    println!("Reading xml file '{}'", xml_path.display());
    let xml = fs::read_to_string(Path::new(xml_path))?;
    Ok(xml)
}

// TODO: Eric has to be initiated
pub fn process(
    xml: String,
    type_version: String,
    processing_flag: ProcessingFlag,
    transfer_code: Option<u32>,
) -> Result<EricResponse, anyhow::Error> {
    println!("Processing xml file");

    match processing_flag {
        ProcessingFlag::Validate => println!("Validating xml file"),
        ProcessingFlag::Send => println!("Sending xml file"),
        ProcessingFlag::Print => println!("Print"),
        ProcessingFlag::CheckHints => println!("Check hints"),
    }

    let xml = xml.try_to_cstring()?;
    let type_version = type_version.try_to_cstring()?;

    // transfer_code should be NULL except for data retrieval; if transfer_code is not NULL in the other cases, it will be ignored
    let transfer_code = match transfer_code {
        Some(mut code) => &mut code,
        None => ptr::null::<u32>() as *mut u32,
    };

    // TODO: set parameters
    // let print_settings = eric_druck_parameter_t {
    //     version: 2,
    //     vorschau: todo!(),
    //     ersteSeite: todo!(),
    //     duplexDruck: todo!(),
    //     pdfName: todo!(),
    //     fussText: todo!(),
    // };

    // TODO: set parameters
    // let crypto_settings = eric_verschluesselungs_parameter_t {
    //     abrufCode: ptr::null(),
    //     pin: todo!(),
    //     version: 2,
    //     zertifikatHandle: todo!(),
    // };

    let validation_response_buffer = unsafe { EricRueckgabepufferErzeugen() };
    let server_response_buffer = unsafe { EricRueckgabepufferErzeugen() };

    let error_code = unsafe {
        EricBearbeiteVorgang(
            xml.as_ptr(),
            type_version.as_ptr(),
            processing_flag as u32,
            // TODO: pass ptr::null() or &print_settings
            ptr::null(),
            // TODO: pass ptr::null() or &crypto_settings,
            ptr::null(),
            // TODO: pass ptr::null() or transfer_code
            transfer_code,
            validation_response_buffer,
            server_response_buffer,
        )
    };

    let transfer_code = unsafe { transfer_code.as_ref() };

    match transfer_code {
        Some(code) => println!("Transfer code: {}", code),
        None => (),
    }

    let validation_response = unsafe {
        let c_buffer = EricRueckgabepufferInhalt(validation_response_buffer);
        CStr::from_ptr(c_buffer)
    }
    .to_str()?
    .to_string();

    let server_response = unsafe {
        let c_buffer = EricRueckgabepufferInhalt(server_response_buffer);
        CStr::from_ptr(c_buffer)
    }
    .to_str()?
    .to_string();

    unsafe {
        EricRueckgabepufferFreigeben(validation_response_buffer);
        EricRueckgabepufferFreigeben(server_response_buffer);
    }

    let response = EricResponse::new(error_code, validation_response, server_response);

    Ok(response)
}

pub fn validate(xml: String, type_version: String) -> Result<EricResponse, anyhow::Error> {
    process(xml, type_version, ProcessingFlag::Validate, None)
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
    fn test_init() {
        let config = configure().unwrap();

        let res = init(config.plugin_path, config.log_path);

        close().unwrap();

        println!("{:#?}", res);
        assert!(res.is_ok());
    }

    #[test]
    fn test_validate() {
        let current_dir = current_dir().unwrap();
        println!("current dir: {:#?}", current_dir);

        let config = configure().unwrap();
        let xml_path = "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml";

        println!("Reading xml file");
        let xml = fs::read_to_string(xml_path).unwrap();

        let version = "Bilanz_6.5".to_string();

        init(config.plugin_path, config.log_path).unwrap();

        let res = validate(xml, version);

        close().unwrap();

        println!("{:#?}", res);
        assert!(res.is_ok());

        let response = res.unwrap();

        let current_dir = env::current_dir()
            .context("Can't get current directory")
            .unwrap();

        let validation_response_path = current_dir.join("validation_response.xml");
        let server_response_path = current_dir.join("server_response.xml");

        let mut validation_response_file = File::create(validation_response_path).unwrap();
        let mut server_response_file = File::create(server_response_path).unwrap();

        validation_response_file
            .write_all(response.validation_response.as_bytes())
            .unwrap();
        server_response_file
            .write_all(response.server_response.as_bytes())
            .unwrap();

        assert_eq!(response.error_code, ErrorCode::ERIC_OK as i32);

        let doc = Document::parse(&response.validation_response).unwrap();
        println!("Doc: {:#?}", doc);
        let node = doc.descendants().find(|node| node.has_tag_name("Erfolg"));
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.tag_name().name(), "Erfolg");

        assert!(response.server_response.is_empty());
    }
}
