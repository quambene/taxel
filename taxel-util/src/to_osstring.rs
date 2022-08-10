use std::ffi::OsStr;

pub trait ToOsString<'a> {
    fn to_osstring(self) -> &'a OsStr;
}

impl<'a> ToOsString<'a> for &'a str {
    fn to_osstring(self) -> &'a OsStr {
        OsStr::new(self)
    }
}
