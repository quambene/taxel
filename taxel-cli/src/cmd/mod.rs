mod decrypt;
mod send;
mod validate;

pub use decrypt::{decrypt, decrypt_args};
pub use send::{send, send_args};
pub use validate::{validate, validate_args};

// Binary name
pub const BIN: &str = "taxel";

// Available subcommands
pub const VALIDATE: &str = "validate";
pub const SEND: &str = "send";
pub const DECRYPT: &str = "decrypt";
