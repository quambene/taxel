use crate::errors::ErrorCode;
use anyhow::{anyhow, Context};
use std::{
    env,
    path::{Path, PathBuf},
};
use taxel_bindings::ericapi::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang, EricBeende,
    EricInitialisiere, EricRueckgabepufferErzeugen,
};
use taxel_util::ToCString;

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

pub fn configure() -> Result<Config, anyhow::Error> {
    println!("Configuring eric");

    let cargo_manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("Missing environment variable 'CARGO_MANIFEST_DIR'");
    let cargo_manifest_dir = Path::new(&cargo_manifest_dir).join("..");

    let plugin_path =
        env::var("PLUGIN_PATH").context("Missing environment variable 'PLUGIN_PATH'")?;
    let plugin_path = Path::new(&cargo_manifest_dir).join(plugin_path);

    let log_path = env::current_dir().context("Can't get current directory")?;

    println!("Setting plugin path '{}'", plugin_path.display());
    println!("Setting log path '{}'", log_path.display());

    let config = Config::new(plugin_path, log_path);

    Ok(config)
}

pub fn init(plugin_path: PathBuf, log_path: PathBuf) -> Result<(), anyhow::Error> {
    println!("Initializing eric");

    let plugin_path = plugin_path.try_to_cstring()?;
    let log_path = log_path.try_to_cstring()?;

    let error_code = unsafe { EricInitialisiere(plugin_path.as_ptr(), log_path.as_ptr()) };

    match error_code {
        x if x == ErrorCode::Ok as i32 => Ok(()),
        // TODO: handle all error codes
        error_code => Err(anyhow!("Can't init eric: {}", error_code)),
    }
}

// TODO: implement drop for Eric
pub fn close() -> Result<(), anyhow::Error> {
    println!("Closing eric");

    let error_code = unsafe { EricBeende() };

    match error_code {
        x if x == ErrorCode::Ok as i32 => Ok(()),
        // TODO: handle all error codes
        error_code => Err(anyhow!("Can't init eric: {}", error_code)),
    }
}

// TODO: Eric has to be initiated
pub fn process(
    xml: String,
    version: String,
    flags: u32,
    mut transfer_code: u32,
) -> Result<(), anyhow::Error> {
    let xml = xml.try_to_cstring()?.as_ptr();
    let version = version.try_to_cstring()?.as_ptr();
    let transfer_code = &mut transfer_code;

    // TODO: set correct parameters
    let print_settings = eric_druck_parameter_t {
        version: 2,
        vorschau: 1,
        ersteSeite: 1,
        duplexDruck: 1,
        pdfName: "todo".to_string().try_to_cstring()?.as_ptr(),
        fussText: "todo".to_string().try_to_cstring()?.as_ptr(),
    };

    // TODO: set correct parameters
    let crypto_settings = eric_verschluesselungs_parameter_t {
        abrufCode: "todo".to_string().try_to_cstring()?.as_ptr(),
        pin: "todo".to_string().try_to_cstring()?.as_ptr(),
        version: 1,
        zertifikatHandle: 1,
    };

    let error_code = unsafe {
        let eric_response_buffer = EricRueckgabepufferErzeugen();
        let server_response_buffer = EricRueckgabepufferErzeugen();

        EricBearbeiteVorgang(
            xml,
            version,
            flags,
            &print_settings,
            &crypto_settings,
            transfer_code,
            eric_response_buffer,
            server_response_buffer,
        )
    };

    match error_code {
        0 => Ok(()),
        // TODO: handle all error codes
        _ => Err(anyhow!("Can't close eric")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let config = configure().unwrap();

        let res = init(config.plugin_path, config.log_path);

        close().unwrap();

        println!("{:#?}", res);
        assert!(res.is_ok());
    }
}
