//! Validate xml file according to the given taxonomy and send xml file to the
//! tax authorities.

use crate::arg;
use clap::{Arg, ArgMatches};
use std::{env::current_dir, path::PathBuf};
use taxel::{CertificateConfig, Eric, PrintConfig, ProcessingFlag};

pub fn send_args() -> [Arg<'static>; 7] {
    [
        arg::tax_type(),
        arg::tax_version(),
        arg::xml_file(),
        arg::certificate_file(),
        arg::password(),
        arg::print(),
        arg::log_dir(),
    ]
}

pub fn send(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, arg::XML_FILE)?;
    let tax_type = arg::get_one(matches, arg::TAX_TYPE)?;
    let tax_version = arg::get_one(matches, arg::TAX_VERSION)?;
    let certificate_file = arg::get_one(matches, arg::CERTIFICATE_FILE)?;
    let password = arg::get_one(matches, arg::PASSWORD)?;
    let log_dir = arg::get_maybe_one(matches, arg::LOG_DIR);
    let type_version = format!("{}_{}", tax_type, tax_version);
    let processing_flag: ProcessingFlag;
    let log_path = match log_dir {
        Some(log_dir) => PathBuf::from(log_dir),
        None => current_dir()?,
    };

    let eric = Eric::new(&log_path)?;

    let certificate_config = CertificateConfig::new(certificate_file, password)?;

    let print_config = if matches.contains_id(arg::PRINT) {
        processing_flag = ProcessingFlag::SendAndPrint;
        let pdf_name = arg::get_one(matches, arg::PRINT)?;
        Some(PrintConfig::new(pdf_name, &processing_flag)?)
    } else {
        processing_flag = ProcessingFlag::Send;
        None
    };

    let xml = taxel::read(xml_file)?;

    let response = eric.send(
        xml,
        type_version,
        processing_flag,
        certificate_config,
        print_config,
    )?;

    eric.log(&log_path, &response)?;

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
            "../test_data/ebilanz/taxonomy_v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
            "--certificate-file",
            "../test_data/test-certificate.pfx",
            "--password",
            "123456",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::SEND).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = send(subcommand_matches);
        println!("res: {:#?}", res);

        assert!(res.is_ok())
    }

    #[test]
    fn test_send_and_print() {
        let args = vec![
            cmd::BIN,
            cmd::SEND,
            "--xml-file",
            "../test_data/ebilanz/taxonomy_v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
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

        let res = send(subcommand_matches);
        println!("res: {:#?}", res);

        assert!(res.is_ok())
    }
}
