//! Rust bindings for the ELSTER Rich Client (ERiC)

// TODO: Fix https://github.com/rust-lang/cargo/issues/6313

mod ericapi;

pub use ericapi::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang, EricBeende,
    EricCloseHandleToCertificate, EricDekodiereDaten, EricGetHandleToCertificate,
    EricInitialisiere, EricReturnBufferApi, EricRueckgabepufferErzeugen,
    EricRueckgabepufferFreigeben, EricRueckgabepufferInhalt, EricZertifikatHandle,
};
