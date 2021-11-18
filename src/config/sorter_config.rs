use crate::config::seg_config::{MakeModelPatternCfg, ScreenshotPatternCfg, DateTimePatternCfg, SimpleFileTypePatternCfg};
use minidom::Element;
use crate::config::{CfgError, CfgValueError, SegmentConfig};
use std::str::FromStr;
use crate::sorting::{DuplicateResolution, Comparison, Sorter, SorterBuilder};
use std::path::PathBuf;
use std::rc::Rc;

pub struct SorterCfg {
    supported: Vec<SegmentCfg>,
    fallback: Vec<SegmentCfg>,
    dup_handling: DuplicateResolution
}

pub struct SegmentCfg {
    seg_type: String,
    index: i32,
    cfg: Box<dyn SegmentConfig + Send>
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

        // get 'type' attribute
        let cfg = match el.attr("type") {
            Some(tp) => {
                match tp {
                    "MakeModelPattern" => {
                        MakeModelPatternCfg::from(el)
                    },
                    "ScreenshotPattern" => {
                        ScreenshotPatternCfg::from(el)
                    },
                    "DateTimePattern" => {
                        DateTimePatternCfg::from(el)
                    },
                    "SimpleFileTypePattern" => {
                        SimpleFileTypePatternCfg::from(el)
                    }
                    _ => {
                        println!("[WARN] found unsupported segment type: {}", tp);
                        Err(CfgError::unsupported_segment("unsupported segment type"))
                    }
                }
            },
            None => Err(CfgError::IllegalValue(CfgValueError::new("missing mandatory attribute \"type\"")))
        }?;

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


        Ok(
            SegmentCfg{
                seg_type: seg_tp,
                index,
                cfg
            }
        )
    }

    pub fn from_multiple(el: &Element) -> Result<Vec<SegmentCfg>, CfgError> {
        let mut segments: Vec<SegmentCfg> = Vec::new();

        for child in el.children() {
            let mut i = 0;
            if child.name() == "segment" {
                if let Some(seg) = match Self::from(child) {
                    Ok(s) => Ok(Some(s)),
                    Err(e) => match e {
                        CfgError::XmlParseFailure(_) | CfgError::IllegalValue(_) | CfgError::IoError(_) => Err(e),
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
        let mut dup_handling = Sorter::def_duplicate_handling();

        for child in el.children() {
            match child.name() {
                "supported" => {
                    if let Some(segs) = child.get_child("segments", "") {
                        supported = SegmentCfg::from_multiple(segs)?;
                    }
                },
                "fallback" => {
                    if let Some(segs) = child.get_child("segments", "") {
                        fallback = SegmentCfg::from_multiple(segs)?;
                    }
                },
                "duplicateResolution" => {
                    dup_handling = Self::parse_duplicate_resolution(child)?;
                },
                _ => continue
            }
        }

        Ok(SorterCfg{
            supported,
            fallback,
            dup_handling
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

    pub fn generate_builder(&self, target_dir: PathBuf) -> Result<SorterBuilder, CfgError> {
        let mut builder = Sorter::new(target_dir)
            .duplicate_handling(self.dup_handling);

        for seg in &self.supported {
            builder.push_segment_supported(seg.cfg.generate()?);
        }

        for seg in &self.fallback {
            builder.push_segment_fallback(seg.cfg.generate()?);
        }
        Ok(builder)
    }

    /*
    pub fn generate(&self, target_dir: PathBuf, mpsc::) -> Result<Sorter, CfgError> {
        let mut builder = self.generate_builder(target_dir);

        Ok(builder.build())
    }
     */
}