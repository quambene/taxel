use crate::ErrorCode;
use std::ffi::CStr;
use taxel_bindings::{
    EricReturnBufferApi, EricRueckgabepufferErzeugen, EricRueckgabepufferFreigeben,
    EricRueckgabepufferInhalt,
};

pub struct ResponseBuffer {
    ctx: *mut EricReturnBufferApi,
}

impl ResponseBuffer {
    pub fn new() -> Result<Self, anyhow::Error> {
        let response_buffer = unsafe { EricRueckgabepufferErzeugen() };

        Ok(ResponseBuffer {
            ctx: response_buffer,
        })
    }

    pub fn as_ptr(&self) -> *mut EricReturnBufferApi {
        self.ctx
    }

    pub fn read(&self) -> Result<&str, anyhow::Error> {
        let buffer = unsafe {
            let ptr = EricRueckgabepufferInhalt(self.ctx);
            CStr::from_ptr(ptr)
        };

        Ok(buffer.to_str()?)
    }
}

impl Drop for ResponseBuffer {
    fn drop(&mut self) {
        println!("Cleaning up response buffer");

        let error_code = unsafe { EricRueckgabepufferFreigeben(self.ctx) };

        match error_code {
            x if x == ErrorCode::ERIC_OK as i32 => (),
            error_code => panic!("Can't drop reponse buffer: {}", error_code),
        }
    }
}
