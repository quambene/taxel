mod ericapi;
mod error_code;

pub use ericapi::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang, EricBeende,
    EricInitialisiere, EricRueckgabepufferErzeugen, EricRueckgabepufferFreigeben,
    EricRueckgabepufferInhalt,
};
pub use error_code::ErrorCode;
