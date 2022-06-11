#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, ffi::CString};

    #[test]
    fn test_eric() {
        let plugin_path =
            env::var("PLUGIN_PATH").expect("Missing environment variable 'PLUGIN_PATH'");
        let plugin_path = CString::new(plugin_path).unwrap();
        let plugin_path = plugin_path.as_ptr();

        let log_path = env::current_dir().unwrap();
        let log_path = CString::new(log_path.to_str().unwrap()).unwrap();
        let log_path = log_path.as_ptr();

        unsafe {
            let error_code = EricInitialisiere(plugin_path, log_path);
            assert_eq!(error_code, 0);

            let buffer = EricRueckgabepufferErzeugen();
            let error_code = EricVersion(buffer);
            assert_eq!(error_code, 0);

            let error_code = EricBeende();
            assert_eq!(error_code, 0);
        }
    }
}
