use crate::config::seg_config::{MakeModelPatternCfg, ScreenshotPatternCfg, DateTimePatternCfg, SimpleFileTypePatternCfg};
use minidom::Element;
use crate::config::{CfgError, CfgValueError};
use std::str::FromStr;
use crate::sorting::{DuplicateResolution, Comparison};

pub struct SorterCfg {
    supported: Vec<SegmentCfg>,
    fallback: Vec<SegmentCfg>,
}

pub struct SegmentCfg {
    seg_type: String,
    index: i32,
    cfg: SegmentType
}

pub enum SegmentType {
    None,
    MakeModelPattern(MakeModelPatternCfg),
    ScreenshotPattern(ScreenshotPatternCfg),
    DateTimePattern(DateTimePatternCfg),
    SimpleFileTypePattern(SimpleFileTypePatternCfg)
}

impl SegmentCfg {
    pub fn from(el: &Element) -> Result<SegmentCfg, CfgError> {
        let mut seg_tp = String::new();
        let mut index = 0;
        let mut cfg = SegmentType::None;

        // get 'type' attribute
        if let Some(tp) = el.attr("type") {
            cfg = match tp {
                "MakeModelPattern" => {
                    SegmentType::MakeModelPattern(MakeModelPatternCfg::from(el)?)
                },
                "ScreenshotPattern" => {
                    SegmentType::ScreenshotPattern(ScreenshotPatternCfg::from(el)?)
                },
                "DateTimePattern" => {
                    SegmentType::DateTimePattern(DateTimePatternCfg::from(el)?)
                },
                "SimpleFileTypePattern" => {
                    SegmentType::SimpleFileTypePattern(SimpleFileTypePatternCfg::from(el)?)
                }
                _ => {
                    println!("[WARN] found unsupported segment type: {}", tp);
                    return Err(CfgError::unsupported_segment("unsupported segment type"))
                }
            }
        }
        else {
            return Err(CfgError::IllegalValue(CfgValueError::new("missing mandatory attribute \"type\"")));
        }

        // get index attribute
        if let Some(i_str) = el.attr("index") {
            index = match i32::from_str(i_str) {
                Ok(ind) => ind,
                Err(e) => Err(
                    CfgError::IllegalValue(CfgValueError::new(
                        format!("Could not read index attribute of segment: {}", e).as_str()
                    ))
                )?
            };
        }
        else {
            return Err(
                CfgError::val_err("missing mandatory attribute \"index\"")
            )
        }


        Ok(SegmentCfg{
            seg_type: seg_tp,
            index,
            cfg
        })
    }

    pub fn from_multiple(el: &Element) -> Result<Vec<SegmentCfg>, CfgError> {
        let mut segments: Vec<SegmentCfg> = Vec::new();

        for child in el.children() {
            let mut i = 0;
            if child.name() == "segment" {
                if let Some(seg) = match Self::from(child) {
                    Ok(s) => Ok(Some(s)),
                    Err(e) => match e {
                        CfgError::XmlParseFailure(_) | CfgError::IllegalValue(_) => Err(e),
                        CfgError::UnsupportedSegment(x) => {
                            println!("[WARN] ignoring segment at index={}", i);
                            Ok(None)
                        }
                    }
                }? {
                    // if negative, insert at beginning
                    if seg.index < 0 {
                        segments.insert(0, seg);
                    }
                    // if between 0 and last item, insert at index
                    else if seg.index < segments.len() as i32 {
                        segments.insert(seg.index as usize, seg);
                    }
                    // append at end (most cases if XML was ordered)
                    else {
                        segments.push(seg);
                    }
                }
                i += 1;
            }
        }

        Ok(segments)
    }
}

impl SorterCfg {

    pub fn from(el: &Element) -> Result<SorterCfg, CfgError> {
        let mut fallback: Vec<SegmentCfg> = Vec::new();
        let mut supported: Vec<SegmentCfg> = Vec::new();

        for child in el.children() {
            match child.name() {
                "supported" => {
                    supported = SegmentCfg::from_multiple(child)?;
                },
                "fallback" => {
                    fallback = SegmentCfg::from_multiple(child)?;
                }
                _ => continue
            }
        }

        Ok(SorterCfg{
            supported,
            fallback
        })
    }

    pub fn parse_duplicate_resolution(el: &Element) -> Result<DuplicateResolution, CfgError> {
        if let Some(s) = el.attr("strategy") {
            let result = match s {
                "ignore" => Ok(DuplicateResolution::Ignore),
                "overwrite" => Ok(DuplicateResolution::Overwrite),
                "compare" => {
                    match el.text().as_str() {
                        "rename" => Ok(DuplicateResolution::Compare(Comparison::Rename)),
                        "favor_target" => Ok(DuplicateResolution::Compare(Comparison::FavorTarget)),
                        "favor_source" => Ok(DuplicateResolution::Compare(Comparison::FavorSource)),
                        c => Err(CfgError::val_err(
                            format!("Illegal value for duplicateResolution strategy=\"{}\": \"{}\"",
                                s, c).as_str()
                        ))
                    }
                },
                _ => Err(CfgError::val_err(
                    format!("Illegal value for duplicateResolution strategy: \"{}\"", s).as_str()
                ))
            }?;
            Ok(result)
        }
        else {
            Err(CfgError::val_err("missing attribute \"strategy\" on duplicateResolution"))
        }
    }
}