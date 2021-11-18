mod sorter_config;
mod seg_config;

use minidom;
use std::error::Error;
use std::fmt::{Display, Formatter};
use crate::pattern::PatternElement;
use crate::config::sorter_config::SorterCfg;
use minidom::Element;
use std::fs::File;
use std::io::Read;
use crate::sorting::{Sorter, SorterBuilder};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug)]
pub enum CfgError {
    XmlParseFailure(minidom::Error),
    IllegalValue(CfgValueError),
    UnsupportedSegment(CfgValueError),
    IoError(std::io::Error)
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
impl Error for CfgValueError { }
impl CfgValueError {
    pub fn new(msg: &str) -> CfgValueError {
        CfgValueError{
            msg: String::from(msg)
        }
    }
}

pub trait SegmentConfig {
    fn generate(&self) -> Result<Box<dyn PatternElement + Send>, CfgError>;
}

pub struct RootCfg {
    sorter: SorterCfg
}

impl RootCfg {
    pub fn from(el: &Element) -> Result<RootCfg, CfgError> {
        let mut sorter: Option<SorterCfg> = None;

        for child in el.children() {
            match child.name() {
                "sorter" => {
                    sorter = Some(SorterCfg::from(child)?);
                },
                _ => continue
            }
        }

        if let Some(s) = sorter {
            Ok(RootCfg{
                sorter: s
            })
        }
        else {
            Err(CfgError::val_err("mandatory child element \"sorter\" not found"))
        }
    }

    pub fn read_file(file: &mut File) -> Result<RootCfg, CfgError> {
        let data = &mut String::new();
        match file.read_to_string(data) {
            Err(e) => Err(CfgError::IoError(e)),
            Ok(sz) => {
                println!("[INFO] successfully read {} bytes of config", sz);

                let root_el: Element = match data.parse() {
                    Ok(e) => Ok(e),
                    Err(e) => Err(CfgError::XmlParseFailure(e))
                }?;
                if let Some(config_el) = root_el.get_child("config", "") {
                    Self::from(config_el)
                }
                else {
                    Err(CfgError::val_err("missing root element \"config\""))
                }
            }
        }
    }

    pub fn generate_sorter_builder(&self, outdir: PathBuf) -> Result<SorterBuilder, CfgError> {
        self.sorter.generate_builder(outdir)
    }

    /*
    pub fn generate_sorter(&self, outdir: PathBuf) -> Result<Sorter, CfgError> {
        self.sorter.generate(outdir)
    }
     */
}