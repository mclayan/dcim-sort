use std::str::FromStr;

use minidom::Element;

use crate::config::{CfgError, CfgValueError, SegmentConfig};
use crate::pattern::device::{CaseNormalization, DevicePart, MakeModelPattern};
use crate::pattern::fallback::SimpleFileTypePattern;
use crate::pattern::general::{DateTimePart, DateTimePattern, ScreenshotPattern};
use crate::pattern::PatternElement;

pub struct SegPart {
    index: i32,
    value: String,
}

pub struct MakeModelPatternCfg {
    parts: Vec<SegPart>,
    replace_spaces: bool,
    default_make: String,
    default_model: String,
    separator: char,
    case_normalization: CaseNormalization,
    fallback: String,
}

pub struct ScreenshotPatternCfg {
    value: String,
    filename_pattern: Option<(String, bool)>,
}

pub struct DateTimePatternCfg {
    parts: Vec<SegPart>,
    separator: char,
    default_value: String,
    fallback_fs_timestamp: bool,
}

pub struct SimpleFileTypePatternCfg {
    default_video: String,
    default_picture: String,
    default_audio: String,
    default_text: String,
    default_document: String,
    default_other: String,
}

fn parse_single_char(el: &Element) -> Result<Option<char>, CfgError> {
    let s = el.text();
    return if !s.is_empty() {
        match s.len() {
            1 => {
                if s.bytes().len() != 1 {
                    Err(CfgError::val_err("separator is not a single-byte character!"))
                } else {
                    let b = s.bytes().next().unwrap();
                    Ok(Some(char::from(b)))
                }
            }
            _ => {
                Err(
                    CfgError::IllegalValue(CfgValueError::new(
                        "value \"separator\" must be exactly one character"
                    ))
                )
            }
        }
    } else {
        Ok(None)
    };
}

fn parse_boolean(el: &Element) -> Result<Option<bool>, CfgError> {
    let text = el.text();
    if text.is_empty() {
        Ok(None)
    } else {
        match bool::from_str(text.as_str()) {
            Ok(r) => Ok(Some(r)),
            Err(_) => Err(CfgError::val_err(
                format!("value for element \"{}\" could not parsed as boolean", el.name()).as_str()
            ))
        }
    }
}

fn parse_string(el: &Element) -> Option<String> {
    let s = el.text();
    if !s.is_empty() {
        Some(s)
    } else {
        None
    }
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

        Ok(SegPart {
            index,
            value,
        })
    }

    pub fn from_multi(el: &Element) -> Result<Vec<SegPart>, CfgError> {
        let mut parts: Vec<SegPart> = Vec::new();
        for child in el.children() {
            match child.name() {
                "part" => {
                    parts.push(SegPart::from(child)?);
                }
                _ => continue
            }
        }
        Ok(parts)
    }
}


impl MakeModelPatternCfg {
    pub fn from(el: &Element) -> Result<Box<dyn SegmentConfig + Send>, CfgError> {
        let mut parts: Vec<SegPart> = Vec::new();
        let mut replace_spaces = MakeModelPattern::def_replace_spaces();
        let mut def_make = MakeModelPattern::def_default_make();
        let mut def_model = MakeModelPattern::def_default_model();
        let mut case_normalization = MakeModelPattern::def_case();
        let mut separator = MakeModelPattern::def_separator();
        let mut fallback = String::new();

        for child in el.children() {
            match child.name() {
                "parts" => { parts = SegPart::from_multi(child)? }
                "replaceSpaces" => {
                    if let Some(b) = parse_boolean(child)? {
                        replace_spaces = b;
                    }
                }
                "defaultMake" => {
                    if let Some(s) = parse_string(child) {
                        def_make = s;
                    }
                }
                "defaultModel" => {
                    if let Some(s) = parse_string(child) {
                        def_model = s;
                    }
                }
                "separator" => {
                    if let Some(sep) = parse_single_char(child)? {
                        separator = sep;
                    }
                }
                "caseNormalization" => {
                    case_normalization = match child.text().to_lowercase().as_str() {
                        "lowercase" => Ok(CaseNormalization::Lowercase),
                        "uppercase" => Ok(CaseNormalization::Uppercase),
                        "none" => Ok(CaseNormalization::None),
                        _ => Err(
                            CfgError::IllegalValue(CfgValueError::new(
                                "value \"caseNormalization\" must be one of [\"lowercase\", \"uppercase\", \"none\"]"
                            ))
                        )
                    }?
                }
                "fallback" => {
                    if !child.text().is_empty() {
                        fallback = child.text();
                    }
                }
                _ => continue
            }
        }

        Ok(
            Box::new(MakeModelPatternCfg {
                parts,
                replace_spaces,
                default_make: def_make,
                default_model: def_model,
                separator,
                case_normalization,
                fallback,
            })
        )
    }
}

impl SegmentConfig for MakeModelPatternCfg {
    fn generate(&self) -> Result<Box<dyn PatternElement + Send>, CfgError> {
        let mut builder = MakeModelPattern::new()
            .separator(self.separator)
            .case_normalization(self.case_normalization.clone())
            .replace_spaces(self.replace_spaces)
            .default_make(self.default_make.clone())
            .default_model(self.default_model.clone())
            .fallback(self.fallback.clone());

        for part in &self.parts {
            if let Some(p) = DevicePart::parse(part.value.as_str()) {
                builder.push_part(p);
            } else {
                return Err(CfgError::val_err(
                    format!("Illegal value for DevicePart: \"{}\"", part.value).as_str()
                ));
            }
        }

        Ok(builder.build())
    }
}


impl ScreenshotPatternCfg {
    pub fn from(el: &Element) -> Result<Box<dyn SegmentConfig + Send>, CfgError> {
        let mut value = ScreenshotPattern::def_value();
        let mut filename_pattern: Option<String> = None;
        let mut case_insensitive = false;
        for child in el.children() {
            match child.name() {
                "value" => {
                    if !child.text().is_empty() {
                        value = child.text();
                    }
                }
                "filenamePattern" => {
                    if !child.text().is_empty() {
                        filename_pattern = Some(child.text());
                        if let Some(case_str) = child.attr("caseInsensitive") {
                            case_insensitive = match bool::from_str(case_str) {
                                Ok(b) => b,
                                Err(_) => {
                                    return Err(CfgError::val_err("invalid value for attribute \"caseInsensitive\": must be a boolean"));
                                }
                            };
                        }
                    }
                }
                _ => continue
            }
        }
        Ok(Box::new(match filename_pattern {
            None => ScreenshotPatternCfg {
                value,
                filename_pattern: None,
            },
            Some(p) => ScreenshotPatternCfg {
                value,
                filename_pattern: Some((p, case_insensitive)),
            }
        }))
    }
}

impl SegmentConfig for ScreenshotPatternCfg {
    fn generate(&self) -> Result<Box<dyn PatternElement + Send>, CfgError> {
        match &self.filename_pattern {
            None => Ok(ScreenshotPattern::new(self.value.clone())),
            Some(p) => match ScreenshotPattern::with_fname_matching(self.value.clone(),
                                                                    p.0.as_str(),
                                                                    p.1) {
                Ok(r) => Ok(r),
                Err(e) => Err(CfgError::val_err(format!("failed to load screenshot file pattern: {}", e).as_str()))
            }
        }
    }
}


impl DateTimePatternCfg {
    pub fn from(el: &Element) -> Result<Box<dyn SegmentConfig + Send>, CfgError> {
        let mut parts: Vec<SegPart> = Vec::new();
        let mut separator = DateTimePattern::def_separator();
        let mut def_val = DateTimePattern::def_default();
        let mut fallback = DateTimePattern::def_fs_timestamp_fallback();

        for child in el.children() {
            match child.name() {
                "parts" => parts = SegPart::from_multi(child)?,
                "separator" => {
                    if let Some(sep) = parse_single_char(child)? {
                        separator = sep;
                    }
                }
                "defaultValue" => {
                    if let Some(s) = parse_string(child) {
                        def_val = s;
                    }
                }
                "fallbackFsTimestamp" => {
                    if let Some(b) = parse_boolean(child)? {
                        fallback = b;
                    }
                }
                _ => continue
            }
        }

        Ok(
            Box::new(DateTimePatternCfg {
                parts,
                separator,
                default_value: def_val,
                fallback_fs_timestamp: fallback,
            })
        )
    }
}

impl SegmentConfig for DateTimePatternCfg {
    fn generate(&self) -> Result<Box<dyn PatternElement + Send>, CfgError> {
        let mut builder = DateTimePattern::new()
            .separator(self.separator)
            .default(self.default_value.clone())
            .fs_timestamp_fallback(self.fallback_fs_timestamp);

        for part in &self.parts {
            if let Some(p) = DateTimePart::parse(part.value.as_str()) {
                builder.push_part(p);
            } else {
                return Err(CfgError::val_err(
                    format!("Illegal value for DateTimePart: \"{}\"", part.value).as_str()
                ));
            }
        }

        Ok(builder.build())
    }
}


impl SimpleFileTypePatternCfg {
    pub fn from(el: &Element) -> Result<Box<dyn SegmentConfig + Send>, CfgError> {
        let mut video = SimpleFileTypePattern::def_video();
        let mut pic = SimpleFileTypePattern::def_picture();
        let mut audio = SimpleFileTypePattern::def_audio();
        let mut text = SimpleFileTypePattern::def_text();
        let mut doc = SimpleFileTypePattern::def_document();
        let mut other = SimpleFileTypePattern::def_other();

        for child in el.children() {
            match child.name() {
                "defaultVideo" => {
                    if let Some(s) = parse_string(child) {
                        video = s;
                    }
                }
                "defaultPicture" => {
                    if let Some(s) = parse_string(child) {
                        pic = s;
                    }
                }
                "defaultAudio" => {
                    if let Some(s) = parse_string(child) {
                        audio = s;
                    }
                }
                "defaultText" => {
                    if let Some(s) = parse_string(child) {
                        text = s;
                    }
                }
                "defaultDocument" => {
                    if let Some(s) = parse_string(child) {
                        doc = s;
                    }
                }
                "defaultOther" => {
                    if let Some(s) = parse_string(child) {
                        other = s;
                    }
                }
                _ => continue
            }
        }

        Ok(
            Box::new(SimpleFileTypePatternCfg {
                default_video: video,
                default_picture: pic,
                default_audio: audio,
                default_text: text,
                default_document: doc,
                default_other: other,
            })
        )
    }
}

impl SegmentConfig for SimpleFileTypePatternCfg {
    fn generate(&self) -> Result<Box<dyn PatternElement + Send>, CfgError> {
        Ok(SimpleFileTypePattern::new()
            .video(self.default_video.clone())
            .picture(self.default_picture.clone())
            .audio(self.default_audio.clone())
            .text(self.default_text.clone())
            .document(self.default_document.clone())
            .other(self.default_other.clone())
            .build()
        )
    }
}