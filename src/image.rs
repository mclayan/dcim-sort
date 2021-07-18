use chrono::{DateTime, Local, TimeZone};
use std::path::{Path, PathBuf};
use std::io::{Error, ErrorKind, BufReader};
use std::fs;
use exif;
use exif::Value;
use std::ffi::OsStr;
use std::fs::{read, read_to_string};
use std::process::exit;
use std::fmt::Formatter;


fn match_file_type(extension: &str) -> FileType {
    match extension.to_lowercase().as_str() {
        "jpeg" => FileType::JPEG,
        "jpg" => FileType::JPEG,
        "png" => FileType::PNG,
        "heic" => FileType::HEIC,
        _ => FileType::Other
    }
}

#[derive(Debug)]
pub enum FileType {
    JPEG,
    PNG,
    HEIC,
    Other
}

#[derive(Debug)]
pub struct ImgInfo {
    fp: PathBuf,
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
    user_comment: String,
    is_screenshot: bool
}

impl ImgInfo {
    pub fn new(file: PathBuf) -> Result<ImgInfo, std::io::Error> {
        //let file = PathBuf::from(&file_path);

        if !file.exists() || !file.is_file() {
            return Err(Error::new(ErrorKind::NotFound, "Could not open path as file!"));
        }
        let metadata : std::fs::Metadata = file.metadata()?;
        let file_type = match file.extension() {
            None => FileType::Other,
            Some(s) => match_file_type(s.to_str().expect("Could not convert extension to str!"))
        };

        let exif = ImgInfo::read_exif_data(&file);
        let meta = match exif {
            Some(e) => Some(ImgMeta::from_exif(&e)),
            None => None
        };
        Ok(ImgInfo {
            fp: file,
            size: 0,
            file_type,
            meta,
            changed_at: DateTime::from(metadata.modified()?)
        })
    }

    fn read_exif_data(path: &PathBuf) -> Option<exif::Exif> {
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

    pub fn path(&self) -> &Path {
        self.fp.as_path()
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

    pub fn metadata(&self) -> Option<&ImgMeta> {
         match &self.meta {
            Some(m) => Some(m),
            None => None
        }
    }
}

impl ImgMeta {
    pub fn from_exif(exif: &exif::Exif) -> ImgMeta {
        // first try with DateTime, if not present try DateTimeOriginal
        let datetime_field = match exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
            None => exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY),
            Some(f) => Some(f)
        };
        let mut timestamp: Option<DateTime<Local>> = match datetime_field {
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
        let user_comment = match extract_as_string(&exif, exif::Tag::UserComment) {
            Some(s) => s,
            None => String::new()
        };
        let is_screenshot = user_comment == "Screenshot";

        ImgMeta {
            created_at: timestamp,
            make,
            model,
            user_comment,
            is_screenshot
        }
    }

    pub fn created_at(&self) -> Option<&DateTime<Local>> {
        if let Some(ts) = &self.created_at {
            Some(ts)
        }
        else {
            None
        }
    }
    pub fn make(&self) -> &str {
        &self.make
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn user_comment(&self) -> &str {
        &self.user_comment
    }
    pub fn is_screenshot(&self) -> bool {
        self.is_screenshot
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

#[derive(Debug, Clone)]
struct TagParseError {
    msg: String
}

impl std::fmt::Display for TagParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl TagParseError {
    pub fn new(msg: &str) -> TagParseError {
        TagParseError {
            msg: String::from(msg)
        }
    }
}

fn extract_user_comment(bytes: &Vec<u8>) -> Result<String, TagParseError> {
    if bytes.len() <= 8 {
        let e = TagParseError::new("minimum size violated!");
        return Err(e)
    }
    let trimmed = &bytes[8..];
    match std::str::from_utf8(trimmed) {
        Err(e) => return Err(TagParseError::new("UserComment is not UTF-8 encodable!")),
        Ok(s) => Ok(String::from(s))
    }
}


fn extract_as_string(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = exif.get_field(tag, exif::In::PRIMARY);
    let val = match field {
        Some(t) => &t.value,
        None => return None
    };
    /*
    println!("tag=\"{}\" val_type=\"{}\"", tag, match val {
        Value::Ascii(_) => "ascii",
        Value::Byte(_) => "byte",
        Value::SByte(_) => "sbyte",
        Value::Double(_) => "double",
        Value::Float(_) => "float",
        Value::Long(_) => "long",
        Value::SLong(_) => "slong",
        Value::Rational(_) => "rational",
        Value::SRational(_) => "srational",
        Value::Short(_) => "short",
        Value::SShort(_) => "sshort",
        Value::Undefined(_, _) => "undefined",
        Value::Unknown(_, _, _) => "unknown"
    });
     */
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
                match extract_user_comment(vec) {
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