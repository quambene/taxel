use anyhow::Context;
use std::{ffi::CString, path::PathBuf};

pub trait ToCString {
    fn try_to_cstring(self) -> Result<CString, anyhow::Error>;
}

impl ToCString for String {
    fn try_to_cstring(self) -> Result<CString, anyhow::Error> {
        CString::new(self).context("Can't convert to CString")
    }
}

impl ToCString for &str {
    fn try_to_cstring(self) -> Result<CString, anyhow::Error> {
        CString::new(self).context("Can't convert to CString")
    }
}

impl ToCString for PathBuf {
    fn try_to_cstring(self) -> Result<CString, anyhow::Error> {
        self.to_str()
            .context("Can't convert path to string")?
            .try_to_cstring()
    }
}
