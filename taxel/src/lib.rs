mod eric;
mod error_code;

pub use eric::{close, configure, init, log, read, send, validate, Config};
pub use error_code::ErrorCode;
