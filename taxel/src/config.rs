pub struct CertificateConfig {
    // TODO: use PathBuf instead of String
    pub path: String,
    pub password: String,
}

impl CertificateConfig {
    pub fn new(path: String, password: String) -> Self {
        Self { path, password }
    }
}

pub struct PrintConfig {
    // TODO: use PathBuf instead of String
    pub pdf_name: String,
}

impl PrintConfig {
    pub fn new(pdf_name: &str) -> Self {
        Self {
            pdf_name: pdf_name.to_string(),
        }
    }
}
