use minidom::Element;
use crate::config::{CfgError, CfgValueError};
use crate::pattern::device::{MakeModelPattern, CaseNormalization};
use std::str::FromStr;
use std::any::{Any, TypeId};

pub struct SegPart {
    index: i32,
    value: String
}

pub struct MakeModelPatternCfg {
    parts: Vec<SegPart>,
    replace_spaces: bool,
    default_make: String,
    default_model: String,
    separator: char,
    case_normalization: CaseNormalization,
    fallback: String
}

pub struct ScreenshotPatternCfg {
    value: String
}

pub struct DateTimePatternCfg {
    parts: Vec<SegPart>,
    separator: char,
    default_value: String,
    fallback_fs_timestamp: bool
}

pub struct SimpleFileTypePatternCfg {
    default_video: String,
    default_picture: String,
    default_audio: String,
    default_document: String,
    default_other: String
}

impl SegPart {
    pub fn from(el: &Element) -> Result<SegPart, CfgError> {
        let ind_str = match el.attr("index") {
            None => Err(CfgError::val_err("mandatory attribute \"index\" is missing")),
            Some(s) => Ok(s)
        }?;

        let index = match i32::from_str(ind_str) {
            Err(_) => Err(CfgError::val_err("mandatory attribute \"index\" is missing")),
            Ok(i) => Ok(i)
        }?;

        let value = el.text();

        Ok(SegPart{
            index,
            value
        })
    }
}

impl MakeModelPatternCfg {
    pub fn from(el: &Element) -> Result<MakeModelPatternCfg, CfgError> {
        let mut parts: Vec<SegPart> = Vec::new();
        let mut replace_spaces = MakeModelPattern::def_replace_spaces();
        let mut def_make = MakeModelPattern::def_default_make();
        let mut def_model = MakeModelPattern::def_default_model();
        let mut case_normalization = MakeModelPattern::def_case();
        let mut separator = MakeModelPattern::def_separator();
        let mut fallback = String::new();

        for child in el.children() {
            match child.name() {
                "parts" => { parts = Some(Self::parse_parts(child)?) },
                "replaceSpaces" => {
                    replace_spaces = match bool::from_str(child.text().as_str()) {
                        Ok(b) => Ok(b),
                        Err(e) => { Err(
                            CfgError::IllegalValue(CfgValueError::new(
                                "value \"replaceSpaces\" could not be parsed as boolean"
                            ))
                        )}
                    }?
                }
                "defaultMake" => {
                    if !child.text().is_empty() {
                        def_make = child.text();
                    }
                },
                "defaultModel" => {
                    if !child.text().is_empty() {
                        def_model = child.text();
                    }
                },
                "separator" => {
                    let s = child.text();
                    if !s.is_empty() {
                        separator = match s.len() {
                            1 => s[0],
                            _ => { Err(
                                   CfgError::IllegalValue(CfgValueError::new(
                                       "value \"separator\" must be exactly one character"
                                   ))
                                )
                            }
                        }?
                    }
                },
                "caseNormalization" => {
                    case_normalization = match child.text().to_lowercase().as_str() {
                        "lowercase" => CaseNormalization::Lowercase,
                        "uppercase" => CaseNormalization::Uppercase,
                        "none" => CaseNormalization::None,
                        _ => Err(
                            CfgError::IllegalValue(CfgValueError::new(
                                "value \"caseNormalization\" must be one of [\"lowercase\", \"uppercase\", \"none\"]"
                            ))
                        )
                    }?
                },
                "fallback" => {
                    if !child.text().is_empty() {
                        fallback = child.text();
                    }
                }
                _ => continue
            }
        }

        Ok(MakeModelPatternCfg{
            parts,
            replace_spaces,
            default_make: def_make,
            default_model: def_model,
            separator,
            case_normalization,
            fallback
        })
    }

    fn parse_parts(el: &Element) -> Result<Vec<SegPart>, CfgError> {
        let mut parts: Vec<SegPart> = Vec::new();
        for child in el.children() {
            match child.name() {
                "part" => {
                    parts.push(SegPart::from(child)?);
                },
                _ => continue
            }
        }
        Ok(parts)
    }
}