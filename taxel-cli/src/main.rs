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
        Some((cmd::NEW, matches)) => cmd::new(matches),
        Some((cmd::MERGE, matches)) => cmd::merge(matches),
        Some((cmd::EXPORT, matches)) => cmd::export(matches),
        Some((cmd::IMPORT, matches)) => cmd::import(matches),
        Some((cmd::DOWNLOAD, matches)) => cmd::download(matches),
        Some((cmd::VALIDATE, matches)) => cmd::validate(matches),
        Some((cmd::SEND, matches)) => cmd::send(matches),
        _ => Err(anyhow!("Subcommand not found")),
    }
}
