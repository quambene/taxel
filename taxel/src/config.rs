use crate::{certificate::Certificate, Preview, ProcessingFlag};
use std::{
    ffi::{CStr, CString},
    path::Path,
    ptr,
};
use taxel_bindings::{eric_druck_parameter_t, eric_verschluesselungs_parameter_t};
use taxel_util::ToCString;

pub struct CertificateConfig {
    // allocate path as CString
    pub path: CString,
    // allocate password as CString
    pub password: CString,
    pub certificate: Certificate,
    pub certificate_parameter: CertificateParameter,
}

impl CertificateConfig {
    pub fn new(path: &str, password: &str) -> Result<Self, anyhow::Error> {
        let path = Path::new(path).try_to_cstring()?;
        let password = password.try_to_cstring()?;
        let certificate = Certificate::new(&path)?;
        let certificate_parameter = CertificateParameter::new(&certificate, &password);

        Ok(Self {
            path,
            password,
            certificate,
            certificate_parameter,
        })
    }
}

// CertificateParameter is a wrapper type to keep C memory alignment for eric_verschluesselungs_parameter_t
#[derive(Debug)]
#[repr(C)]
pub struct CertificateParameter {
    inner: eric_verschluesselungs_parameter_t,
}

impl CertificateParameter {
    pub fn new(certificate: &Certificate, password: &CStr) -> Self {
        let certificate_parameter = eric_verschluesselungs_parameter_t {
            abrufCode: ptr::null(),
            // SAFETY: password.as_ptr() is not dangling as password is allocated in struct CertificateConfig and password is not moved as a reference to the CString is given
            pin: password.as_ptr(),
            version: 2,
            zertifikatHandle: certificate.handle,
        };

        Self {
            inner: certificate_parameter,
        }
    }

    pub fn as_ptr(&self) -> *const eric_verschluesselungs_parameter_t {
        &self.inner
    }
}

#[derive(Debug)]
pub struct PrintConfig {
    // allocate pdf_name as CString
    pub pdf_name: CString,
    pub print_parameter: PrintParameter,
}

impl PrintConfig {
    pub fn new(pdf_name: &str, processing_flag: &ProcessingFlag) -> Result<Self, anyhow::Error> {
        let pdf_path = Path::new(pdf_name).try_to_cstring()?;
        let print_parameter = PrintParameter::new(&pdf_path, processing_flag);

        Ok(Self {
            pdf_name: pdf_path,
            print_parameter,
        })
    }
}

// PrintParameter is a wrapper type to keep C memory alignment for eric_druck_parameter_t
#[derive(Debug)]
#[repr(C)]
pub struct PrintParameter {
    inner: eric_druck_parameter_t,
}

impl PrintParameter {
    pub fn new(pdf_name: &CStr, processing_flag: &ProcessingFlag) -> Self {
        let print_parameter = eric_druck_parameter_t {
            version: 2,
            vorschau: match processing_flag {
                ProcessingFlag::Validate => Preview::Yes as u32,
                ProcessingFlag::Print => Preview::Yes as u32,
                _ => Preview::No as u32,
            },
            ersteSeite: 0,
            duplexDruck: 0,
            // SAFETY: pdf_name.as_ptr() is not dangling as pdf_name is allocated in struct PrintConfig and pdf_name is not moved as a reference to the CString is given
            pdfName: pdf_name.as_ptr(),
            fussText: ptr::null(),
        };

        Self {
            inner: print_parameter,
        }
    }

    pub fn as_ptr(&self) -> *const eric_druck_parameter_t {
        &self.inner
    }
}
