mod sorter_config;
mod seg_config;

use minidom;
use std::error::Error;
use std::fmt::{Display, Formatter};

pub enum CfgError {
    XmlParseFailure(minidom::Error),
    IllegalValue(CfgValueError),
    UnsupportedSegment(CfgValueError)
}

impl CfgError {
    pub fn val_err(msg: &str) -> CfgError {
        CfgError::IllegalValue(CfgValueError::new(msg))
    }

    pub fn unsupported_segment(msg: &str) -> CfgError {
        CfgError::UnsupportedSegment(CfgValueError::new(msg))
    }
}

#[derive(Debug)]
pub struct CfgValueError {
    msg: String
}

impl Display for CfgValueError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}
impl Error for CfgValueError {

}
impl CfgValueError {
    pub fn new(msg: &str) -> CfgValueError {
        CfgValueError{
            msg: String::from(msg)
        }
    }
}