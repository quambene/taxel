use crate::arg;
use clap::{Arg, ArgMatches};
use taxel::Certificate;

pub fn send_args() -> [Arg<'static>; 5] {
    [
        arg::xml_file(),
        arg::tax_type(),
        arg::tax_version(),
        arg::certificate_file(),
        arg::password(),
    ]
}

pub fn send(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = matches
        .get_one::<String>(arg::XML_FILE)
        .expect("Missing value for argument");
    let tax_type = matches
        .get_one::<String>(arg::TAX_TYPE)
        .expect("Missing value for argument");
    let tax_version = matches
        .get_one::<String>(arg::TAX_VERSION)
        .expect("Missing value for argument");
    let certificate_file = matches
        .get_one::<String>(arg::CERTIFICATE_FILE)
        .expect("Missing value for argument");
    let password = matches
        .get_one::<String>(arg::PASSWORD)
        .expect("Missing value for argument");
    let type_version = format!("{}_{}", tax_type, tax_version);

    let certificate = Certificate::new(certificate_file.to_string(), password.to_string());

    let config = taxel::configure()?;

    let xml = taxel::read(xml_file)?;

    taxel::init(config.plugin_path, config.log_path)?;

    let res = taxel::send(xml, type_version, certificate);

    taxel::close()?;

    taxel::log(res?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app, cmd};

    #[test]
    fn test_send() {
        let args = vec![
            cmd::BIN,
            cmd::SEND,
            "--xml-file",
            "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::SEND).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = send(&subcommand_matches);
        println!("res: {:#?}", res);

        assert!(res.is_ok())
    }
}
