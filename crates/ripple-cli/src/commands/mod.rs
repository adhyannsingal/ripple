pub mod inspect;

#[derive(Debug)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}
impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub mod impact;
pub mod index;
mod persistent;
pub mod status;

pub mod diff;
mod diff_render;
