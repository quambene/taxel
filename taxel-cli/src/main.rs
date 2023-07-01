mod arg;
mod cmd;

use anyhow::anyhow;
use clap::{crate_version, App, SubCommand};

#[macro_use]
extern crate log;

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let app = app();
    let matches = app.get_matches();

    if matches.is_present(arg::VERBOSE) {
        info!("matches: {:#?}", matches);
    }

    match matches.subcommand() {
        Some((cmd::GENERATE, matches)) => cmd::generate(matches),
        Some((cmd::VALIDATE, matches)) => cmd::validate(matches),
        Some((cmd::SEND, matches)) => cmd::send(matches),
        _ => Err(anyhow!("Subcommand not found")),
    }
}

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
