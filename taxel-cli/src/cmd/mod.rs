mod download;
mod export;
mod merge;
mod new;
mod send;
mod utils;
mod validate;

pub use download::{download, download_args};
pub use export::{export, export_args};
pub use merge::{merge, merge_args};
pub use new::{new, new_args};
pub use send::{send, send_args};
pub use validate::{validate, validate_args};

/// The binary name for taxel-cli.
pub const BIN: &str = "taxel";

/// The available subcommands for taxel-cli.
pub const NEW: &str = "new";
pub const MERGE: &str = "merge";
pub const EXPORT: &str = "export";
pub const DOWNLOAD: &str = "download";
pub const VALIDATE: &str = "validate";
pub const SEND: &str = "send";
