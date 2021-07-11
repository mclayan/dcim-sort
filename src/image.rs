use chrono::{DateTime, Local, Date, TimeZone};
use std::path::Path;
use std::io::{Error, ErrorKind, BufReader};
use std::borrow::Borrow;
use std::fs;
use exif;
use exif::Value;
use std::ffi::OsStr;
use std::fs::read;


fn match_file_type(extension: &str) -> FileType {
    match extension.to_lowercase().as_str() {
        "jpeg" => FileType::JPEG,
        "jpg" => FileType::JPEG,
        "heic" => FileType::HEIC,
        _ => FileType::Other
    }
}

#[derive(Debug)]
pub enum FileType {
    JPEG,
    HEIC,
    Other
}

#[derive(Debug)]
pub struct ImgInfo {
    fp: String,
    size: usize,
    file_type: FileType,
    meta: Option<ImgMeta>,
    changed_at: DateTime<Local>
}

#[derive(Debug)]
pub struct ImgMeta {
    created_at: Option<DateTime<Local>>,
    make: String,
    model: String,
    is_screenshot: bool
}

impl ImgInfo {
    pub fn new(file_path: String) -> Result<ImgInfo, std::io::Error> {
        let file = Path::new(&file_path);

        if !file.exists() || !file.is_file() {
            return Err(Error::new(ErrorKind::NotFound, "Could not open path as file!"));
        }
        let metadata : std::fs::Metadata = file.metadata()?;
        let file_type = match file.extension() {
            None => FileType::Other,
            Some(s) => match_file_type(s.to_str().expect("Could not convert extension to str!"))
        };

        let exif = ImgInfo::read_exif_data(file);
        let meta = match exif {
            Some(e) => Some(ImgMeta::from_exif(&e)),
            None => None
        };
        Ok(ImgInfo {
            fp: file_path,
            size: 0,
            file_type,
            meta,
            changed_at: DateTime::from(metadata.modified()?)
        })
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

    pub fn path(&self) -> &String {
        &self.fp
    }

    pub fn size(&self) -> &usize {
        &self.size
    }

    pub fn file_type(&self) -> &FileType {
        &self.file_type
    }

    pub fn changed_at(&self) -> &DateTime<Local> {
        &self.changed_at
    }

    pub fn matadata(&self) -> Option<&ImgMeta> {
         match &self.meta {
            Some(m) => Some(m),
            None => None
        }
    }
}

impl ImgMeta {
    pub fn from_exif(exif: &exif::Exif) -> ImgMeta {
        let mut timestamp: Option<DateTime<Local>> = match exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
            None => None,
            Some(field) => ImgMeta::parse_datetime(&field.value)
        };

        let make = match extract_as_string(&exif, exif::Tag::Make) {
            Some(s) => s,
            None => String::new()
        };
        let model = match extract_as_string(&exif, exif::Tag::Model) {
            Some(s) => s,
            None => String::new()
        };

        ImgMeta {
            created_at: timestamp,
            make,
            model,
            is_screenshot: false
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
        _ => return None
    }
}