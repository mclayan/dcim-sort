use crate::media::{FileMetaProcessor, ImgInfo, MetaType, ImgMeta};
use chrono::{DateTime, Local};

pub struct MetaProcessor {
    processors: Vec<Box<dyn FileMetaProcessor>>,
}
pub struct MetaProcessorBuilder {
    processors: Vec<Box<dyn FileMetaProcessor>>,
}

impl MetaProcessorBuilder {
    pub fn processor(mut self, p: Box<dyn FileMetaProcessor>) -> MetaProcessorBuilder {
        self.processors.push(p);
        self
    }

    pub fn build(mut self) -> MetaProcessor {
        MetaProcessor {
            processors: self.processors
        }
    }
}

impl MetaProcessor {
    pub fn new() -> MetaProcessorBuilder {
        MetaProcessorBuilder {
            processors: Vec::new()
        }
    }

    pub fn process_all(&self, mut files: Vec<ImgInfo>) -> Vec<ImgInfo> {
        for info in &mut files {
            self.process(info);
        }
        files
    }

    pub fn process(&self, img: &mut ImgInfo) {
        let meta_types = MetaType::from_filetype(img.file_type());
        let mut meta = img.metadata().clone();
        let mut changed = false;

        // loop through metadata types and process each supported one once with the first
        // supporting processor.
        for meta_type in meta_types {
            for processor in &self.processors {
                if processor.supports(&meta_type) {
                    if let Some(m) = processor.read_metadata(img.path()) {
                        meta.merge_in(&m);
                        changed = true;
                    }
                    break;
                }
            }
        }
        if changed {
            img.set_metadata(meta);
        }
    }
}
