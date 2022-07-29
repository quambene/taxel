mod certificate;
mod config;
mod eric;
mod error_code;
mod processing;
mod response_buffer;
mod xml;

pub use config::{CertificateConfig, PrintConfig};
pub use eric::Eric;
pub use error_code::ErrorCode;
pub use xml::read;

/*
    ProcessingFlag
    Validate = 2
    Send = 4
    Print = 32
    CheckHints = 128

    binary operation: 2 | 4 = 6
    ValidateAndSend = 6

    binary operation: 2 | 32 = 34
    ValidateAndPrint = 34

    binary operation: 4 | 32 = 36
    SendAndPrint = 36

    binary operation: 2 | 4 | 32 = 38
    ValidateAndSendAndPrint = 38

    Send and Print are always validated, i.e.
        Send = ValidateAndSend
        Print = ValidateAndPrint
        SendAndPrint = ValidateAndSendAndPrint
*/

#[derive(Debug)]
pub enum ProcessingFlag {
    Validate = 2,
    Send = 4,
    Print = 32,
    SendAndPrint = 36,
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
