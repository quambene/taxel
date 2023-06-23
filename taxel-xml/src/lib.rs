pub mod read_target_tags;
pub mod update_target_tags;

#[derive(Debug)]
pub struct Tag {
    name: String,
    value: String,
}

impl Tag {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}
