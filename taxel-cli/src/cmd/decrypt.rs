use crate::arg;
use anyhow::anyhow;
use clap::{Arg, ArgMatches};
use taxel::{CertificateConfig, Eric, ErrorCode};

pub fn decrypt_args() -> [Arg<'static>; 3] {
    [
        Arg::new(arg::ENCRYPTED_FILE)
            .long(arg::ENCRYPTED_FILE)
            .required(true)
            .takes_value(true)
            .help("File to be decrypted"),
        Arg::new(arg::CERTIFICATE_FILE)
            .long(arg::CERTIFICATE_FILE)
            .required(true)
            .takes_value(true)
            .help("Certificate for decrypting data"),
        Arg::new(arg::PASSWORD)
            .long(arg::PASSWORD)
            .required(true)
            .takes_value(true)
            .help("Password for certificate"),
    ]
}

pub fn decrypt(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let encrypted_file = arg::get_one(matches, arg::ENCRYPTED_FILE)?;
    let certificate_file = arg::get_one(matches, arg::CERTIFICATE_FILE)?;
    let password = arg::get_one(matches, arg::PASSWORD)?;

    let config = CertificateConfig::new(certificate_file.to_string(), password.to_string());

    let eric = Eric::new()?;

    let error_code = eric.decrypt(encrypted_file, config)?;

    match error_code {
        x if x == ErrorCode::ERIC_OK as i32 => Ok(()),
        others => Err(anyhow!("Can't decrypt file: {}", others)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_decrypt() {
        todo!()
    }
}
