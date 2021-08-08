use chrono::{DateTime, Local};
use std::path::{PathBuf, Path};
use std::io::{Error, ErrorKind};

//mod image;
pub mod kadamak_exif;
pub mod metadata_processor;
pub mod rexiv_proc;

#[derive(Debug)]
pub enum FileType {
    JPEG,
    PNG,
    HEIC,
    Other
}
pub enum MetaType {
    Exif,
    XMP,
    None
}
pub struct FileMetaType {
    file: FileType,
    meta: MetaType
}

pub trait FileMetaProcessor {
    fn supports(&self, mt: &MetaType, ft: &FileType) -> bool;
    fn read_metadata(&self, file: &Path) -> Option<ImgMeta>;
}



#[derive(Debug)]
pub struct ImgInfo {
    fp: PathBuf,
    size: usize,
    file_type: FileType,
    meta: ImgMeta,
    changed_at: DateTime<Local>
}

#[derive(Debug,Clone,PartialEq)]
pub struct ImgMeta {
    created_at: Option<DateTime<Local>>,
    make: String,
    model: String,
    user_comment: String,
    is_screenshot: bool
}
#[derive(Debug, Clone)]
struct TagParseError {
    msg: String
}

impl FileType {
    fn from(extension: &str) -> FileType {
        match extension.to_lowercase().as_str() {
            "jpeg" => FileType::JPEG,
            "jpg" => FileType::JPEG,
            "png" => FileType::PNG,
            "heic" => FileType::HEIC,
            _ => FileType::Other
        }
    }
}

impl MetaType {
    pub fn from_filetype(e: &FileType) -> Vec<MetaType> {
        match e {
            FileType::HEIC => vec![MetaType::Exif, MetaType::XMP],
            FileType::JPEG => vec![MetaType::Exif, MetaType::XMP],
            FileType::PNG => vec![MetaType::Exif, MetaType::XMP],
            _ => vec![MetaType::None]
        }
    }
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

impl ImgInfo {
    pub fn new(file: PathBuf) -> Result<ImgInfo, std::io::Error> {
        //let file = PathBuf::from(&file_path);

        if !file.exists() || !file.is_file() {
            return Err(Error::new(ErrorKind::NotFound, "Could not open path as file!"));
        }
        let metadata : std::fs::Metadata = file.metadata()?;
        let file_type = match file.extension() {
            None => FileType::Other,
            Some(s) => FileType::from(s.to_str().expect("Could not convert extension to str!"))
        };

        Ok(ImgInfo {
            fp: file,
            size: 0,
            file_type,
            meta: ImgMeta::new(),
            changed_at: DateTime::from(metadata.modified()?)
        })
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

    pub fn metadata(&self) -> &ImgMeta {
        &self.meta
    }

    pub fn set_metadata(&mut self, m: ImgMeta) {
        self.meta = m;
    }
}

impl ImgMeta {
    pub fn new() -> ImgMeta {
        ImgMeta{
            created_at: None,
            make: String::new(),
            model: String::new(),
            user_comment: String::new(),
            is_screenshot: false
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

    pub fn merge_in(&mut self, other: &ImgMeta) {
        if self.created_at != other.created_at {
            match self.created_at {
                Some(_) => { },
                None => { self.created_at = other.created_at; }
            }
        }

        if self.make.is_empty() && !other.make.is_empty() {
            self.make = other.make.clone();
        }

        if self.model.is_empty() && !other.model.is_empty() {
            self.model = other.model.clone();
        }

        // false is the default, should always be overridden with true
        if !self.is_screenshot && other.is_screenshot {
            self.is_screenshot = other.is_screenshot;
        }

        if self.user_comment.is_empty() && !other.user_comment.is_empty() {
            self.user_comment = other.user_comment.clone();
        }
    }

    pub fn merge(m1: &ImgMeta, m2: &ImgMeta) -> ImgMeta {
        let mut target = m1.clone();
        target.merge_in(m2);
        target
    }
}

#[cfg(test)]
mod tests {

    mod img_meta_tests {
        use crate::media::ImgMeta;
        use chrono::Local;

        #[test]
        fn merge_implements_all_fields() {
            let mut empty = ImgMeta::new();

            let not_empty = ImgMeta {
                created_at: Some(Local::now()),
                make: String::from("SomeMake"),
                model: String::from("SomeModel"),
                user_comment: String::from("A comment!"),
                is_screenshot: true
            };
            empty.merge_in(&not_empty);
            assert_eq!(not_empty, empty);
        }
    }
}