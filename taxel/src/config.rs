pub struct CertificateConfig {
    pub path: String,
    pub password: String,
}

impl CertificateConfig {
    pub fn new(path: String, password: String) -> Self {
        Self { path, password }
    }
}

pub struct PrintConfig {
    pub pdf_name: String,
}

impl PrintConfig {
    pub fn new(pdf_name: String) -> Self {
        Self { pdf_name }
    }
}

impl Default for PrintConfig {
    fn default() -> Self {
        let pdf_name = "ebilanz.pdf".to_string();
        Self::new(pdf_name)
    }
}
