mod eric;
mod error_code;

pub use eric::{
    close, configure, decrypt, init, log, read, send, validate, validate_and_print, Certificate,
    Config, PrintSettings,
};
pub use error_code::ErrorCode;
