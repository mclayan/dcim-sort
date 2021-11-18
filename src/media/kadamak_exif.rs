use crate::media::{FileMetaProcessor, MetaType, ImgMeta, TagParseError, FileType};
use std::path::{Path};
use std::fs;
use std::io::BufReader;
use chrono::{DateTime, Local, TimeZone};
use exif::Value;

pub struct KadamakExifProcessor {

}

impl FileMetaProcessor for KadamakExifProcessor {
    fn supports(&self, mt: &MetaType, ft: &FileType) -> bool {
        match ft {
            FileType::JPEG | FileType::PNG | FileType::HEIC => {
                match mt {
                    MetaType::Exif => true,
                    _ => false
                }
            }
            _ => false
        }
    }

    fn read_metadata(&self, file: &Path) -> Option<ImgMeta> {
        Self::read_meta_exif(file)
    }

    fn clone_boxed(&self) -> Box<dyn FileMetaProcessor + Send> {
        KadamakExifProcessor::new()
    }
}

impl KadamakExifProcessor {
    pub fn new() -> Box<dyn FileMetaProcessor + Send> {
        Box::new(KadamakExifProcessor{})
    }

    fn read_meta_exif(path: &Path) -> Option<ImgMeta> {
        match Self::read_exif_data(path) {
            None => None,
            Some(exif) => {
                // first try with DateTime, if not present try DateTimeOriginal
                let datetime_field = match exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
                    None => exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY),
                    Some(f) => Some(f)
                };
                let timestamp: Option<DateTime<Local>> = match datetime_field {
                    None => None,
                    Some(field) => Self::parse_datetime(&field.value)
                };

                let make = match Self::extract_as_string(&exif, exif::Tag::Make) {
                    Some(s) => s,
                    None => String::new()
                };
                let model = match Self::extract_as_string(&exif, exif::Tag::Model) {
                    Some(s) => s,
                    None => String::new()
                };
                let user_comment = match Self::extract_as_string(&exif, exif::Tag::UserComment) {
                    Some(s) => s,
                    None => String::new()
                };
                let is_screenshot = user_comment == "Screenshot";

                Some(ImgMeta {
                    created_at: timestamp,
                    make,
                    model,
                    user_comment,
                    is_screenshot
                })
            }
        }
    }

    fn read_exif_data(path: &Path) -> Option<exif::Exif> {
        let file = fs::File::open(path).expect("Failed to open path as file!");
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        match exifreader.read_from_container(&mut bufreader) {
            Ok(e) => Some(e),
            Err(err) => {
                println!("Failed to read Exif data for file: {}", err);
                None
            }
        }
    }

    fn parse_datetime(val: &exif::Value) -> Option<DateTime<Local>> {
        match val {
            Value::Ascii(values ) => {
                if let Some(bytes) = values.first() {
                    if let Ok(dt) = exif::DateTime::from_ascii(bytes.as_slice()) {
                        Some(Local.ymd(
                            dt.year as i32,
                            dt.month as u32,
                            dt.day as u32
                        ).and_hms(
                            dt.hour as u32,
                            dt.minute as u32,
                            dt.second as u32
                        ))
                    } else {
                        None
                    }
                }
                else {
                    None
                }
            },
            _ => None
        }
    }

    fn extract_user_comment(bytes: &Vec<u8>) -> Result<String, TagParseError> {
        if bytes.len() <= 8 {
            let e = TagParseError::new("minimum size violated!");
            return Err(e)
        }
        let trimmed = &bytes[8..];
        match std::str::from_utf8(trimmed) {
            Err(_) => return Err(TagParseError::new("UserComment is not UTF-8 encodable!")),
            Ok(s) => Ok(String::from(s))
        }
    }

    fn extract_as_string(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
        let field = exif.get_field(tag, exif::In::PRIMARY);
        let val = match field {
            Some(t) => &t.value,
            None => return None
        };

        match val {
            Value::Ascii(values) => {
                let vals = match values.first() {
                    Some(v) => v,
                    None => return None
                };
                let mut buf = String::new();
                for b in vals {
                    buf.push(char::from(*b));
                }
                Some(buf)
            },
            Value::Undefined(vec, pos) => {
                if tag == exif::Tag::UserComment {
                    match Self::extract_user_comment(vec) {
                        Err(e) => None,
                        Ok(v) => Some(v)
                    }
                }
                else {
                    None
                }
            }
            _ => return None
        }
    }
}




