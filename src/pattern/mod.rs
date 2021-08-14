use std::fmt::Formatter;
use crate::media::ImgInfo;

pub mod general;
pub mod device;
pub mod fallback;

pub trait PatternElement {
    fn is_optional(&self) -> bool;
    fn translate(&self, info: &ImgInfo) -> Option<String>;
    fn display(&self) -> String;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct PatternInitError {
    msg: String
}
impl PatternInitError {
    pub fn new(reason: &str) -> PatternInitError {
        PatternInitError {
            msg: String::from(reason)
        }
    }
}
impl std::fmt::Display for PatternInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to initialise pattern: {}", self.msg)
    }
}

#[derive(Debug, Clone)]
pub struct SortingError {
    msg: String
}
impl SortingError {
    pub fn new(reason: &str) -> SortingError {
        SortingError {
            msg: String::from(reason)
        }
    }
}
impl std::fmt::Display for SortingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to initialise pattern: {}", self.msg)
    }
}