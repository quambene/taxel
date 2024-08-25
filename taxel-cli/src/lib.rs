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
            SubCommand::with_name(cmd::EXTRACT)
                .args(cmd::extract_args())
                .about("Extract tag values from xml file"),
        )
        .subcommand(
            SubCommand::with_name(cmd::GENERATE)
                .args(cmd::generate_args())
                .about("Generate xml file"),
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
