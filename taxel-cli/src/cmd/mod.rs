mod extract;
mod generate;
mod send;
mod validate;

pub use extract::{extract, extract_args};
pub use generate::{generate, generate_args};
pub use send::{send, send_args};
pub use validate::{validate, validate_args};

/// The binary name for taxel-cli.
pub const BIN: &str = "taxel";

/// The available subcommands for taxel-cli.
pub const EXTRACT: &str = "extract";
pub const GENERATE: &str = "generate";
pub const VALIDATE: &str = "validate";
pub const SEND: &str = "send";
