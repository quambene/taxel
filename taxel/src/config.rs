use crate::{Preview, ProcessingFlag};
use std::{ffi::CString, path::Path, ptr};
use taxel_bindings::eric_druck_parameter_t;
use taxel_util::ToCString;

pub struct CertificateConfig {
    // TODO: use PathBuf instead of String
    pub path: String,
    pub password: String,
}

impl CertificateConfig {
    pub fn new(path: String, password: String) -> Self {
        Self { path, password }
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
    pub fn new(pdf_name: &CString, processing_flag: &ProcessingFlag) -> Self {
        let print_parameter = eric_druck_parameter_t {
            version: 2,
            vorschau: match processing_flag {
                ProcessingFlag::Validate => Preview::Yes as u32,
                ProcessingFlag::Print => Preview::Yes as u32,
                _ => Preview::No as u32,
            },
            ersteSeite: 0,
            duplexDruck: 0,
            // SAFETY: pdf_name.as_ptr() is not dangling as pdf_name was allocated in struct PrintConfig and pdf_name is not moved as a reference to the CString is given
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
