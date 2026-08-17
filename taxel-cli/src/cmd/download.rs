//! Download and cache an eBilanz taxonomy for offline use by `new`/`import`.

use crate::arg::{self, TAXONOMY_TYPE, TAXONOMY_VERSION};
use clap::{Arg, ArgMatches};
use log::debug;

pub fn download_args() -> [Arg<'static>; 3] {
    [
        arg::taxonomy_version(),
        arg::taxonomy_type(),
        arg::taxonomy_path(),
    ]
}

/// Download and cache an eBilanz taxonomy for offline use by `new`/`import` subcommands.
pub fn download(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let version = arg::get_one(matches, TAXONOMY_VERSION)?;
    let taxonomy_type = arg::parse_taxonomy_type(arg::get_one(matches, TAXONOMY_TYPE)?)?;
    let taxonomy_dir = arg::resolve_taxonomy_dir(matches)?;

    debug!(
        "Run `taxel download` with configuration:\n{TAXONOMY_VERSION}={version}\n\
         {TAXONOMY_TYPE}={}\ntaxonomy-dir={}",
        arg::taxonomy_type_flag_value(&taxonomy_type),
        taxonomy_dir.display()
    );

    taxel::download_taxonomy(&taxonomy_type, version, &taxonomy_dir)?;

    println!(
        "Downloaded taxonomy v{version} ({}) to {}",
        arg::taxonomy_type_flag_value(&taxonomy_type),
        taxonomy_dir.display()
    );

    Ok(())
}
