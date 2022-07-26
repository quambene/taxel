use crate::arg;
use clap::{Arg, ArgMatches};

pub fn validate_args() -> [Arg<'static>; 3] {
    [
        Arg::new(arg::XML_FILE)
            .long(arg::XML_FILE)
            .required(true)
            .takes_value(true)
            .help("XML file to be validated"),
        Arg::new(arg::TAX_TYPE)
            .long(arg::TAX_TYPE)
            .required(false)
            .takes_value(true)
            .default_value("Bilanz")
            .possible_values(["Bilanz"])
            .help("Tax type of the xml file"),
        Arg::new(arg::TAX_VERSION)
            .long(arg::TAX_VERSION)
            .required(false)
            .takes_value(true)
            .default_value("6.5")
            .possible_values([
                "5.0", "5.1", "5.2", "5.3", "5.4", "6.0", "6.1", "6.2", "6.3", "6.4", "6.5",
            ])
            .help("Tax version of the xml file"),
    ]
}

pub fn validate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let config = taxel::configure()?;

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

    let xml = taxel::read(xml_file)?;

    taxel::init(config.plugin_path, config.log_path)?;

    let res = taxel::validate(xml, type_version);

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
