use std::path::{Path, PathBuf};
use crate::media::{FileType, ImgInfo};
use crate::pattern::PatternElement;

pub struct Translator {
    segments_supported: Vec<Box<dyn PatternElement + Send>>,
    segments_fallback: Vec<Box<dyn PatternElement + Send>>
}

impl Translator {
    pub fn new(segs_sup: Vec<Box<dyn PatternElement + Send>>, segs_fb: Vec<Box<dyn PatternElement + Send>>) -> Translator {
        Translator{
            segments_supported: segs_sup,
            segments_fallback: segs_fb
        }
    }

    pub fn get_seg_count(&self) -> (usize, usize) {
        (self.segments_supported.len(), self.segments_fallback.len())
    }

    pub fn translate(&self, file: &ImgInfo, target_root: &Path) -> PathBuf {
        let mut destination = target_root.to_path_buf();
        let segments = match file.file_type() {
            FileType::Other => &self.segments_fallback,
            _               => &self.segments_supported
        };

        for pattern in segments {
            if let Some(s) = pattern.translate(file) {
                destination.push(s);
            }
        }

        destination
    }
}