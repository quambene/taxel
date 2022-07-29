use crate::{config::CertificateConfig, ErrorCode};
use anyhow::anyhow;
use std::{ffi::CString, path::Path, ptr};
use taxel_bindings::{EricCloseHandleToCertificate, EricGetHandleToCertificate};
use taxel_util::ToCString;

pub struct Certificate {
    pub handle: u32,
    pub password: CString,
}

impl Certificate {
    pub fn new(config: CertificateConfig) -> Result<Self, anyhow::Error> {
        println!("Preparing certificate '{}'", &config.path);

        let mut handle = 0;
        let pin_support = ptr::null::<u32>() as *mut u32;
        let path = Path::new(&config.path).try_to_cstring()?;
        let password = config.password.try_to_cstring()?;

        let error_code =
            unsafe { EricGetHandleToCertificate(&mut handle, pin_support, path.as_ptr()) };

        match error_code {
            x if x == ErrorCode::ERIC_OK as i32 => Ok(Certificate { handle, password }),
            error_code => Err(anyhow!("Can't create certificate: {}", error_code)),
        }
    }

    // TODO: check validity of certificate
    // unsafe { EricHoleZertifikatseigenschaften() }
}

impl Drop for Certificate {
    fn drop(&mut self) {
        println!("Cleaning up certificate");

        let error_code = unsafe { EricCloseHandleToCertificate(self.handle) };

        match error_code {
            x if x == ErrorCode::ERIC_OK as i32 => (),
            error_code => panic!("Can't drop certificate handle: {}", error_code),
        }
    }
}
