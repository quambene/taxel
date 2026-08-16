pub mod arg;
pub mod cmd;

use clap::{crate_version, App, SubCommand};

pub fn app() -> App<'static> {
    App::new(cmd::BIN)
        .version(crate_version!())
        .arg(
            clap::Arg::with_name(arg::VERBOSE)
                .long(arg::VERBOSE)
                .takes_value(false)
                .help("Shows what is going on"),
        )
        .subcommand(
            SubCommand::with_name(cmd::NEW)
                .args(cmd::new_args())
                .about("Build a new, taxonomy-valid eBilanz xml file from scratch"),
        )
        .subcommand(
            SubCommand::with_name(cmd::IMPORT)
                .args(cmd::import_args())
                .about("Import fact values from one xml file into another"),
        )
        .subcommand(
            SubCommand::with_name(cmd::EXPORT)
                .args(cmd::export_args())
                .about("Export fact values from an xml file to a csv file"),
        )
        .subcommand(
            SubCommand::with_name(cmd::DOWNLOAD)
                .args(cmd::download_args())
                .about("Download and cache an eBilanz taxonomy"),
        )
        .subcommand(
            SubCommand::with_name(cmd::VALIDATE)
                .args(cmd::validate_args())
                .about("Validate xml file"),
        )
        .subcommand(
            SubCommand::with_name(cmd::SEND)
                .args(cmd::send_args())
                .about("Validate and send xml file"),
        )
}
