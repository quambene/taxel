use crate::{arg, cmd};
use clap::{Arg, ArgMatches};

pub fn send_args() -> [Arg<'static>; 3] {
    cmd::validate_args()
}

pub fn send(matches: &ArgMatches) -> Result<(), anyhow::Error> {
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

    let res = taxel::send(xml, type_version);

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
