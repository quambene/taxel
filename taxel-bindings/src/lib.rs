mod ericapi;

pub use ericapi::{
    eric_druck_parameter_t, eric_verschluesselungs_parameter_t, EricBearbeiteVorgang, EricBeende,
    EricCloseHandleToCertificate, EricDekodiereDaten, EricGetHandleToCertificate,
    EricInitialisiere, EricReturnBufferApi, EricRueckgabepufferErzeugen,
    EricRueckgabepufferFreigeben, EricRueckgabepufferInhalt, EricZertifikatHandle,
};
