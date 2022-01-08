use crate::media::{FileMetaProcessor, ImgInfo, MetaType};

pub struct MetaProcessor {
    processors: Vec<Box<dyn FileMetaProcessor + Send>>,
}
pub struct MetaProcessorBuilder {
    proc_p_high: Vec<Box<dyn FileMetaProcessor + Send>>,
    proc_p_none: Vec<Box<dyn FileMetaProcessor + Send>>,
    proc_p_low: Vec<Box<dyn FileMetaProcessor + Send>>,
}

pub enum Priority {
    Highest,
    Lowest,
    Fixed(usize),
    None
}

impl MetaProcessorBuilder {
    pub fn processor(mut self, p: Box<dyn FileMetaProcessor + Send>, prio: Priority) -> MetaProcessorBuilder {
        match prio {
            Priority::Highest => { self.proc_p_high.push(p); },
            Priority::Lowest => { self.proc_p_low.push(p); },
            Priority::Fixed(i) => {
                if i > self.proc_p_none.len() {
                    self.proc_p_none.push(p);
                }
                else {
                    self.proc_p_none.insert(i, p);
                }
            }
            Priority::None => { self.proc_p_none.push(p); }
        }
        self
    }

    pub fn build_clone(&self) -> MetaProcessor {
        let processors = self.clone_procs();

        MetaProcessor {
            processors
        }
    }

    fn clone_procs(&self) -> Vec<Box<dyn FileMetaProcessor + Send>> {
        let mut procs = Vec::<Box<dyn FileMetaProcessor + Send>>::with_capacity(self.proc_p_high.len() + self.proc_p_high.len() + self.proc_p_low.len());

        for proc in &self.proc_p_high {
            procs.push(proc.clone_boxed());
        }
        for proc in &self.proc_p_none {
            procs.push(proc.clone_boxed());
        }
        for proc in &self.proc_p_low {
            procs.push(proc.clone_boxed());
        }

        procs
    }
}

impl MetaProcessor {
    pub fn new() -> MetaProcessorBuilder {
        MetaProcessorBuilder {
            proc_p_high: Vec::new(),
            proc_p_none: Vec::new(),
            proc_p_low: Vec::new()
        }
    }

    pub fn process_all(&self, mut files: Vec<ImgInfo>) -> Vec<ImgInfo> {
        let mut count = 0;
        for info in &mut files {
            self.process(info);
            count += 1;
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
                if processor.supports(&meta_type, img.file_type()) {
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
