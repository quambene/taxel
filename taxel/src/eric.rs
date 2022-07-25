use anyhow::{anyhow, Context};
use std::{
    env,
    ffi::CStr,
    path::{Path, PathBuf},
    ptr,
};
use taxel_bindings::{
    ericapi::{
        eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang,
        EricBeende, EricInitialisiere, EricRueckgabepufferErzeugen, EricRueckgabepufferFreigeben,
        EricRueckgabepufferInhalt,
    },
    ErrorCode,
};
use taxel_util::ToCString;

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
    error_code: i32,
    validation_response: String,
    server_response: String,
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

// TODO: Eric has to be initiated
pub fn process(
    xml: String,
    version: String,
    processing_flag: ProcessingFlag,
    mut transfer_code: u32,
) -> Result<EricResponse, anyhow::Error> {
    println!("Processing xml file");

    match processing_flag {
        ProcessingFlag::Validate => println!("Validating xml file"),
        ProcessingFlag::Send => println!("Sending xml file"),
        ProcessingFlag::Print => println!("Print"),
        ProcessingFlag::CheckHints => println!("Check hints"),
    }

    let xml = xml.try_to_cstring()?;
    let version = version.try_to_cstring()?;
    let transfer_code = &mut transfer_code;

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
            version.as_ptr(),
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

    // TODO: handle transfer code
    println!("Transfer code: {}", transfer_code);

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

pub fn validate(
    xml: String,
    version: String,
    transfer_code: u32,
) -> Result<EricResponse, anyhow::Error> {
    process(xml, version, ProcessingFlag::Validate, transfer_code)
}

#[cfg(test)]
mod tests {
    use roxmltree::Document;

    use super::*;
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
        // TODO: Bei allen Verfahren außer der Datenabholung sollte das Transferhandle NULL sein. Wird bei solchen Verfahren ein Handle übergeben, so wird dieses ignoriert.
        let transfer_code = 0;

        init(config.plugin_path, config.log_path).unwrap();

        let res = validate(xml, version, transfer_code);

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
