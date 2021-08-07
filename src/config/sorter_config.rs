use crate::config::seg_config::{MakeModelPatternCfg, ScreenshotPatternCfg, DateTimePatternCfg, SimpleFileTypePatternCfg};
use minidom::Element;
use std::error::Error;
use crate::config::{CfgError, CfgValueError};
use std::str::FromStr;

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
            seg_tp = String::from(tp);
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
                )
            }?
        }
        else {
            return Err(
                CfgError::IllegalValue(CfgValueError::new("missing mandatory attribute \"index\""))
            )
        }


        Ok(SegmentCfg{
            seg_type: seg_tp,
            index,
            cfg
        })
    }
}

impl SorterCfg {

    pub fn from(el: &Element) -> Result<SorterCfg, CfgError> {


        todo!()
    }
}