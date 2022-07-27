use crate::arg;
use clap::{Arg, ArgMatches};

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
    let encrypted_file = matches
        .get_one::<String>(arg::ENCRYPTED_FILE)
        .expect("Missing value for argument");
    let certificate_file = matches
        .get_one::<String>(arg::CERTIFICATE_FILE)
        .expect("Missing value for argument");
    let password = matches
        .get_one::<String>(arg::PASSWORD)
        .expect("Missing value for argument");

    let config = taxel::configure()?;

    taxel::init(config.plugin_path, config.log_path)?;

    let res = taxel::decrypt(encrypted_file, certificate_file, password);

    taxel::close()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode() {
        todo!()
    }
}
