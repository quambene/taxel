mod eric;
mod error_code;

pub use eric::{close, configure, init, log, read, validate, Config};
pub use error_code::ErrorCode;
