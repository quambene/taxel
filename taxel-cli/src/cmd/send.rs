use crate::arg;
use clap::{Arg, ArgMatches};
use taxel::{CertificateConfig, Eric, PrintConfig};

pub fn send_args() -> [Arg<'static>; 6] {
    [
        arg::tax_type(),
        arg::tax_version(),
        arg::xml_file(),
        arg::certificate_file(),
        arg::password(),
        arg::print(),
    ]
}

pub fn send(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, arg::XML_FILE)?;
    let tax_type = arg::get_one(matches, arg::TAX_TYPE)?;
    let tax_version = arg::get_one(matches, arg::TAX_VERSION)?;
    let certificate_file = arg::get_one(matches, arg::CERTIFICATE_FILE)?;
    let password = arg::get_one(matches, arg::PASSWORD)?;
    let type_version = format!("{}_{}", tax_type, tax_version);

    let certificate_config =
        CertificateConfig::new(certificate_file.to_string(), password.to_string());

    let print_config = if matches.contains_id(arg::PRINT) {
        let pdf_name = arg::get_one(matches, arg::PRINT)?;
        Some(PrintConfig::new(pdf_name))
    } else {
        None
    };

    let xml = taxel::read(xml_file)?;

    let eric = Eric::new()?;

    let response = eric.send(xml, type_version, certificate_config, print_config)?;

    eric.log(&response)?;

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
            "--certificate-file",
            "../test_data/test-certificate.pfx",
            "--password",
            "123456",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::SEND).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = send(&subcommand_matches);
        println!("res: {:#?}", res);

        assert!(res.is_ok())
    }

    #[test]
    fn test_send_and_print() {
        let args = vec![
            cmd::BIN,
            cmd::SEND,
            "--xml-file",
            "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
            "--certificate-file",
            "../test_data/test-certificate.pfx",
            "--password",
            "123456",
            "--print",
            "ebilanz.pdf",
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
