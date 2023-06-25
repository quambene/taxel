//! Validate xml file according to the given taxonomy.

use crate::arg;
use clap::{Arg, ArgMatches};
use taxel::{Eric, PrintConfig, ProcessingFlag};

pub fn validate_args() -> [Arg<'static>; 4] {
    [
        arg::tax_type(),
        arg::tax_version(),
        arg::xml_file(),
        arg::print(),
    ]
}

pub fn validate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, arg::XML_FILE)?;
    let tax_type = arg::get_one(matches, arg::TAX_TYPE)?;
    let tax_version = arg::get_one(matches, arg::TAX_VERSION)?;
    let type_version = format!("{}_{}", tax_type, tax_version);
    let processing_flag: ProcessingFlag;

    let print_config = if matches.contains_id(arg::PRINT) {
        processing_flag = ProcessingFlag::Print;
        let pdf_name = arg::get_one(matches, arg::PRINT)?;
        Some(PrintConfig::new(pdf_name, &processing_flag)?)
    } else {
        processing_flag = ProcessingFlag::Validate;
        None
    };

    let xml = taxel::read(xml_file)?;

    let eric = Eric::new()?;

    let response = eric.validate(xml, type_version, processing_flag, print_config)?;

    eric.log(&response)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app, cmd};

    #[test]
    fn test_validate() {
        let args = vec![
            cmd::BIN,
            cmd::VALIDATE,
            "--xml-file",
            "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = validate(&subcommand_matches);

        println!("res: {:#?}", res);
        assert!(res.is_ok())
    }

    #[test]
    fn test_validate_and_print() {
        let args = vec![
            cmd::BIN,
            cmd::VALIDATE,
            "--xml-file",
            "../test_data/Bilanz_6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
            "--print",
            "ebilanz.pdf",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = validate(&subcommand_matches);

        println!("res: {:#?}", res);
        assert!(res.is_ok())
    }
}
