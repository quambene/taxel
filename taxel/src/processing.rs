use crate::{
    config::{CertificateConfig, PrintConfig},
    response_buffer::ResponseBuffer,
    EricResponse, ProcessingFlag,
};
use std::ptr;
use taxel_bindings::EricBearbeiteVorgang;
use taxel_util::ToCString;

pub fn process(
    xml: String,
    type_version: String,
    processing_flag: ProcessingFlag,
    print_config: Option<PrintConfig>,
    certificate_config: Option<CertificateConfig>,
    transfer_code: Option<u32>,
) -> Result<EricResponse, anyhow::Error> {
    println!("Processing xml file");

    match processing_flag {
        ProcessingFlag::Validate => println!("Validating xml file"),
        ProcessingFlag::Print => println!("Validating xml file"),
        ProcessingFlag::Send => println!("Sending xml file"),
        ProcessingFlag::SendAndPrint => println!("Send and print"),
        ProcessingFlag::CheckHints => println!("Check hints"),
    }

    let xml = xml.try_to_cstring()?;
    let type_version = type_version.try_to_cstring()?;

    // Transfer_code should be NULL except for data retrieval; if transfer_code is not NULL in the other cases, it will be ignored
    let transfer_code = match transfer_code {
        Some(mut code) => &mut code,
        None => ptr::null::<u32>() as *mut u32,
    };

    match &print_config {
        Some(print_config) => println!(
            "Printing confirmation to file '{}'",
            print_config.pdf_name.to_str()?
        ),
        None => (),
    }

    let validation_response_buffer = ResponseBuffer::new()?;
    let server_response_buffer = ResponseBuffer::new()?;

    let error_code = unsafe {
        EricBearbeiteVorgang(
            xml.as_ptr(),
            type_version.as_ptr(),
            processing_flag.into_u32(),
            // SAFETY: match a reference of print_config; otherwise print_config is moved, and print_parameter.as_ptr() would be dangling
            match &print_config {
                Some(el) => el.print_parameter.as_ptr(),
                None => ptr::null(),
            },
            // SAFETY: match a reference of certificate_config; otherwise certificate_config is moved, and certificate_parameter.as_ptr() would be dangling
            match &certificate_config {
                Some(el) => el.certificate_parameter.as_ptr(),
                None => ptr::null(),
            },
            transfer_code,
            validation_response_buffer.as_ptr(),
            server_response_buffer.as_ptr(),
        )
    };

    // TODO: EricHoleFehlerText() for error code

    let transfer_code = unsafe { transfer_code.as_ref() };

    match transfer_code {
        Some(code) => println!("Transfer code: {}", code),
        None => (),
    }

    let validation_response = validation_response_buffer.read()?;
    // TODO: parse server response via EricGetErrormessagesFromXMLAnswer()
    let server_response = server_response_buffer.read()?;
    let response = EricResponse::new(
        error_code,
        validation_response.to_string(),
        server_response.to_string(),
    );

    Ok(response)
}
