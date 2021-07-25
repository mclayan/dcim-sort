use crate::media::{FileMetaProcessor, MetaType, ImgMeta, FileType};
use std::path::Path;
use rexiv2::Metadata;
use chrono::{DateTime, Local, TimeZone, NaiveDateTime};

const EXIF_DATETIME_RX: &str = "^\\d{4}:\\d{2}:\\d{2} \\d{2}:\\d{2}:\\d{2}$";
const EXIF_DATETIME_FMT: &str = "%Y:%m:%d %T";

const EXIF_T_DATETIME: (u64,&str) = (0x0132, "Exif.Image.DateTime");
const EXIF_T_DATETIME_ORIGINAL: (u64,&str) = (0x9003, "Exif.Image.DateTimeOriginal");
const EXIF_T_MAKE: (u64,&str) = (0x010f, "Exif.Image.Make");
const EXIF_T_MODEL: (u64,&str) = (0x0110, "Exif.Image.Model");
const EXIF_T_USER_COMMENT: (u64,&str) = (0x9286, "Exif.Photo.UserComment");

const XMP_T_CREATE_DATE: &str = "Xmp.photoshop.DateCreated";
const XMP_T_USER_COMMENT: &str = "Xmp.exif.UserComment";

pub struct Rexiv2Processor { }

impl FileMetaProcessor for Rexiv2Processor {

    fn supports(&self, mt: &MetaType, ft: &FileType) -> bool {
        // HEIF formats are not supported yet
        match ft {
            FileType::JPEG | FileType::PNG => {
                match mt {
                    MetaType::Exif | MetaType::XMP => true,
                    _ => false
                }
            },
            _ => false
        }
    }

    fn read_metadata(&self, file: &Path) -> Option<ImgMeta> {
        let mut meta = ImgMeta::new();
        let mut found_meta = false;
        if let Ok(rmeta) = rexiv2::Metadata::new_from_path(file) {
            if rmeta.has_exif() {
                meta.merge_in(&Self::read_exif(&rmeta));
                found_meta = true;
            }
            if rmeta.has_xmp() {
                meta.merge_in(&Self::read_xmp(&rmeta));
                found_meta = true;
            }

        }
        if found_meta {
            Some(meta)
        }
        else {
            None
        }
    }
}

impl Rexiv2Processor {
    pub fn new() -> Box<dyn FileMetaProcessor> {
        Box::new(Rexiv2Processor{})
    }

    fn read_exif(rmeta: &Metadata) -> ImgMeta {
        let created_at = Self::exif_read_datetime(rmeta);
        let make = rmeta.get_tag_string(EXIF_T_MAKE.1).unwrap_or(String::new());
        let model = rmeta.get_tag_string(EXIF_T_MODEL.1).unwrap_or(String::new());
        let user_comment = rmeta.get_tag_string(EXIF_T_USER_COMMENT.1).unwrap_or(String::new());
        let is_screenshot = user_comment == "Screenshot";

        ImgMeta {
            created_at,
            make,
            model,
            user_comment,
            is_screenshot
        }
    }

    fn exif_read_datetime(rmeta: &Metadata) -> Option<DateTime<Local>> {
        if let Ok(tag) = rmeta.get_tag_string(EXIF_T_DATETIME_ORIGINAL.1) {
            Self::exif_parse_datetime(&tag)
        }
        else if let Ok(tag) = rmeta.get_tag_string(EXIF_T_DATETIME.1) {
            Self::exif_parse_datetime(&tag)
        }
        else {
            None
        }
    }

    fn exif_parse_datetime(inp: &str) -> Option<DateTime<Local>> {
        if let Ok(result) = NaiveDateTime::parse_from_str(inp, EXIF_DATETIME_FMT) {
            Some(Local.from_local_datetime(&result).unwrap())
        }
        else {
            None
        }
    }

    fn read_xmp(rmeta: &Metadata) -> ImgMeta {
        let created_at = Self::xmp_read_datetime(rmeta);
        let user_comment = rmeta.get_tag_string(XMP_T_USER_COMMENT).unwrap_or(String::new());
        let is_screenshot = user_comment == "lang=\"x-default\" Screenshot";

        ImgMeta{
            created_at,
            make: String::new(),
            model: String::new(),
            user_comment,
            is_screenshot
        }
    }

    fn xmp_read_datetime(rmeta: &Metadata) -> Option<DateTime<Local>> {
        if let Ok(ts) = rmeta.get_tag_string(XMP_T_CREATE_DATE) {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&ts, "%FT%T") {
                Some(Local.from_local_datetime(&dt).unwrap())
            }
            else {
                None
            }
        }
        else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    mod supports {
        use crate::media::rexiv_proc::Rexiv2Processor;
        use crate::media::{MetaType, FileType};

        #[test]
        fn decline_heif() {
            let flag = Rexiv2Processor::new().supports(&MetaType::Exif, &FileType::HEIC);
            assert!(!flag);
        }
    }
}