mod certificate;
mod config;
mod eric;
mod error_code;
mod processing;
mod response_buffer;
mod xml;

pub use config::{CertificateConfig, EricConfig, PrintConfig};
pub use eric::{close, configure, init, read, Eric};
pub use error_code::ErrorCode;

#[derive(Debug)]
pub enum ProcessingFlag {
    Validate = 2,
    Send = 4,
    Print = 32,
    CheckHints = 128,
}

pub enum Preview {
    Yes = 1,
    No = 0,
}

#[derive(Debug)]
pub struct EricResponse {
    pub error_code: i32,
    pub validation_response: String,
    pub server_response: String,
}

impl EricResponse {
    pub fn new(error_code: i32, validation_response: String, server_response: String) -> Self {
        Self {
            error_code,
            validation_response,
            server_response,
        }
    }
}
