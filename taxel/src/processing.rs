use crate::{
    certificate::Certificate,
    config::{CertificateConfig, PrintConfig},
    response_buffer::ResponseBuffer,
    EricResponse, Preview, ProcessingFlag,
};
use std::{ptr, mem::MaybeUninit};
use taxel_bindings::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang,
};
use taxel_util::{ToCString, ToOsString};

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
        ProcessingFlag::Send => println!("Sending xml file"),
        ProcessingFlag::Print => println!("Validating xml file"),
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
            "Printing transmission confirmation to '{}'",
            print_config.pdf_name
        ),
        None => (),
    }

    // allocate pdf_name as CString
    let pdf_name = print_config
        .map(|el| el.pdf_name.to_osstring().try_to_cstring())
        .transpose()?;

    let mut print_params = MaybeUninit::<eric_druck_parameter_t>::uninit();

    // SAFETY: match reference of pdf_name; otherwise pdf_name is moved, and pdf_name.as_ptr() would be dangling
    if let Some(pdf_name) = &pdf_name {
        // allocate eric_druck_parameter_t
        print_params.write(eric_druck_parameter_t {
            version: 2,
            vorschau: match processing_flag {
                ProcessingFlag::Validate => Preview::Yes as u32,
                ProcessingFlag::Print => Preview::Yes as u32,
                _ => Preview::No as u32,
            },
            ersteSeite: 0,
            duplexDruck: 0,
            pdfName: pdf_name.as_ptr(),
            fussText: ptr::null(),
        });
    }

    let certificate = match certificate_config {
        Some(config) => Some(Certificate::new(config)),
        None => None,
    }
    .transpose()?;

    // SAFETY: match a reference of certificate; otherwise certificate is moved, and certificate.password.as_ptr() would be dangling
    let crypto_params = match &certificate {
        // allocate eric_verschluesselungs_parameter_t
        Some(certificate) => Some(eric_verschluesselungs_parameter_t {
            abrufCode: ptr::null(),
            pin: certificate.password.as_ptr(),
            version: 2,
            zertifikatHandle: certificate.handle,
        }),
        None => None,
    };

    let validation_response_buffer = ResponseBuffer::new()?;
    let server_response_buffer = ResponseBuffer::new()?;

    let error_code = unsafe {
        EricBearbeiteVorgang(
            xml.as_ptr(),
            type_version.as_ptr(),
            processing_flag as u32,
            match pdf_name {
                Some(_) => print_params.as_ptr(),
                None => ptr::null(),
            },
            match crypto_params {
                Some(el) => &el,
                None => ptr::null(),
            },
            transfer_code,
            validation_response_buffer.as_ptr(),
            server_response_buffer.as_ptr(),
        )
    };

    drop(print_params);

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
