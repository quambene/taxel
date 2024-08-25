use anyhow::anyhow;
use taxel_cli::{app, arg, cmd};

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
        Some((cmd::EXTRACT, matches)) => cmd::extract(matches),
        Some((cmd::GENERATE, matches)) => cmd::generate(matches),
        Some((cmd::VALIDATE, matches)) => cmd::validate(matches),
        Some((cmd::SEND, matches)) => cmd::send(matches),
        _ => Err(anyhow!("Subcommand not found")),
    }
}
