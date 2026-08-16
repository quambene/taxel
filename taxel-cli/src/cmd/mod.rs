mod download;
mod export;
mod extract;
mod generate;
mod import;
mod new;
mod send;
mod utils;
mod validate;

pub use download::{download, download_args};
pub use export::{export, export_args};
pub use extract::{extract, extract_args};
pub use generate::{generate, generate_args};
pub use import::{import, import_args};
pub use new::{new, new_args};
pub use send::{send, send_args};
pub use validate::{validate, validate_args};

/// The binary name for taxel-cli.
pub const BIN: &str = "taxel";

/// The available subcommands for taxel-cli.
pub const EXTRACT: &str = "extract";
pub const GENERATE: &str = "generate";
pub const NEW: &str = "new";
pub const IMPORT: &str = "import";
pub const EXPORT: &str = "export";
pub const DOWNLOAD: &str = "download";
pub const VALIDATE: &str = "validate";
pub const SEND: &str = "send";
