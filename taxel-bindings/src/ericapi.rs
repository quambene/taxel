#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::Path};
    use taxel_util::ToCString;

    #[test]
    fn test_ericapi() {
        let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .expect("Missing environment variable 'CARGO_MANIFEST_DIR'");
        let cargo_manifest_dir = Path::new(&cargo_manifest_dir).join("..");

        let plugin_path =
            env::var("PLUGIN_PATH").expect("Missing environment variable 'PLUGIN_PATH'");
        let plugin_path = cargo_manifest_dir
            .join(plugin_path)
            .try_to_cstring()
            .unwrap();

        let log_path = env::current_dir()
            .unwrap()
            .to_str()
            .unwrap()
            .try_to_cstring()
            .unwrap();

        unsafe {
            let error_code = EricInitialisiere(plugin_path.as_ptr(), log_path.as_ptr());
            assert_eq!(error_code, 0);

            let buffer = EricRueckgabepufferErzeugen();
            let error_code = EricVersion(buffer);
            assert_eq!(error_code, 0);

            let error_code = EricBeende();
            assert_eq!(error_code, 0);
        }
    }
}
