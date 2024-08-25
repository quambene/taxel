//! Validate xml file according to the given taxonomy.

use super::utils;
use crate::arg;
use clap::{Arg, ArgMatches};
use eric_sdk::Eric;
use std::{
    env::current_dir,
    fs,
    path::{Path, PathBuf},
};

pub fn validate_args() -> [Arg<'static>; 5] {
    [
        arg::tax_type(),
        arg::tax_version(),
        arg::xml_file(),
        arg::print(),
        arg::log_dir(),
    ]
}

pub fn validate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, arg::XML_FILE)?;
    let tax_type = arg::get_one(matches, arg::TAX_TYPE)?;
    let tax_version = arg::get_one(matches, arg::TAX_VERSION)?;
    let log_dir = arg::get_maybe_one(matches, arg::LOG_DIR);
    let log_path = match log_dir {
        Some(log_dir) => PathBuf::from(log_dir),
        None => current_dir()?,
    };
    let print_config = if matches.contains_id(arg::PRINT) {
        let pdf_name = arg::get_one(matches, arg::PRINT)?;
        Some(pdf_name)
    } else {
        None
    };
    let xml_path = Path::new(xml_file);
    let xml = fs::read_to_string(xml_path)?;

    let eric = Eric::new(&log_path)?;

    let response = eric.validate(xml, tax_type, tax_version, print_config)?;

    utils::log_response(&log_path, &response)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app, cmd};

    #[test]
    #[cfg_attr(not(feature = "integration-test"), ignore)]
    fn test_validate() {
        let args = vec![
            cmd::BIN,
            cmd::VALIDATE,
            "--xml-file",
            "../test_data/taxonomy/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = validate(subcommand_matches);

        println!("res: {:#?}", res);
        assert!(res.is_ok())
    }

    #[test]
    #[cfg_attr(not(feature = "integration-test"), ignore)]
    fn test_validate_and_print() {
        let args = vec![
            cmd::BIN,
            cmd::VALIDATE,
            "--xml-file",
            "../test_data/taxonomy/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
            "--print",
            "ebilanz.pdf",
        ];

        let app = app();
        let matches = app.get_matches_from(args);
        let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
        println!("subcommand matches: {:#?}", subcommand_matches);

        let res = validate(subcommand_matches);

        println!("res: {:#?}", res);
        assert!(res.is_ok())
    }
}
