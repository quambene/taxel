use crate::arg;
use clap::{Arg, ArgMatches};
use taxel::PrintSettings;

pub fn validate_args() -> [Arg<'static>; 4] {
    [
        arg::xml_file(),
        arg::tax_type(),
        arg::tax_version(),
        arg::print(),
    ]
}

pub fn validate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = matches
        .get_one::<String>(arg::XML_FILE)
        .expect("Missing value for argument");
    let tax_type = matches
        .get_one::<String>(arg::TAX_TYPE)
        .expect("Missing value for argument");
    let tax_version = matches
        .get_one::<String>(arg::TAX_VERSION)
        .expect("Missing value for argument");
    let type_version = format!("{}_{}", tax_type, tax_version);
    let print_settings = if matches.contains_id(arg::PRINT) {
        Some(PrintSettings::default())
    } else {
        None
    };

    let config = taxel::configure()?;

    let xml = taxel::read(xml_file)?;

    taxel::init(config.plugin_path, config.log_path)?;

    let res = match print_settings {
        Some(_) => taxel::validate_and_print(xml, type_version, print_settings),
        None => taxel::validate(xml, type_version, print_settings),
    };

    taxel::close()?;

    taxel::log(res?)?;

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
}
