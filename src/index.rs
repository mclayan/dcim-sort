use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use crate::media::{FileType, ImgInfo};
use crate::pipeline::{PipelineController};

pub enum PathBox {
    Directory(PathBuf),
    File(PathBuf)
}

impl PathBox {
    pub fn from(p: PathBuf) -> PathBox {
        return if p.is_dir() {
            PathBox::Directory(p)
        } else {
            PathBox::File(p)
        }
    }
}

pub struct Scanner {
    entry_point: PathBuf,
    max_depth: u8,
    depth: u8,
    debug: bool,
    ignore_unknown_types: bool
}

impl Scanner {
    pub fn new(root_path: &Path) -> Result<Scanner, Error> {
        if !root_path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "root must be a directory!"));
        }
        else {
            Ok(Scanner{
                entry_point: root_path.to_path_buf(),
                max_depth: 10,
                depth: 0,
                debug: false,
                ignore_unknown_types: false
            })
        }
    }

    pub fn debug(&mut self, b: bool) {
        self.debug = b;
    }

    pub fn ignore_unknown_types(&mut self, b: bool) {
        self.ignore_unknown_types = b;
    }

    pub fn set_max_depth(&mut self, max: u8) {
        self.max_depth = max;
    }

    pub fn get_max_depth(&self) -> u8 {
        self.max_depth
    }

    pub fn scan(&mut self) -> Vec<ImgInfo> {
        let mut index : Vec<ImgInfo> =  Vec::new();
        self.depth = 0;
        let root = self.entry_point.clone();
        self.scan_path(PathBox::from(root), &mut index);
        index
    }

    pub fn scan_pipeline(&mut self, controller: &mut PipelineController) {
        if self.debug {
            println!("starting with root={}", self.entry_point.to_str().unwrap_or("<INVALID_UTF-8>"));
        }
        let root =self.entry_point.clone();
        self.scan_path_ch(PathBox::from(root), controller);
    }

    fn scan_path(&mut self, d: PathBox, index: &mut Vec<ImgInfo>) {
        if self.debug {
            let tmp = match &d{
                PathBox::Directory(d) => ("d", String::from(d.to_str().unwrap_or("?"))),
                PathBox::File(d) => ("f", String::from(d.to_str().unwrap_or("?")))
            };
            println!("depth={:03} type={} p={}", self.depth, tmp.0, tmp.1);
        }
        match d {
            PathBox::File(f) => {
                match ImgInfo::new(f) {
                    Ok(i) => {
                        if self.ignore_unknown_types {
                            match i.file_type() {
                                FileType::Other => {},
                                _ => { index.push(i); },
                            }
                        }
                        else {
                            index.push(i);
                        }
                    },
                    Err(e) => println!("Error processing file: {}", e)
                }
            },
            PathBox::Directory(d) => {
                if self.depth < self.max_depth {
                    self.depth += 1;
                    for child in d.read_dir().expect("Error reading path a directory!") {
                        let child_path = child.expect("Error reading child!").path();
                        self.scan_path(PathBox::from(child_path), index);
                    }
                    self.depth -= 1;
                }
            }
        }

    }

    fn scan_path_ch(&mut self, d: PathBox, controller: &mut PipelineController) {
        if self.debug {
            let tmp = match &d{
                PathBox::Directory(d) => ("d", String::from(d.to_str().unwrap_or("?"))),
                PathBox::File(d) => ("f", String::from(d.to_str().unwrap_or("?")))
            };
            println!("depth={:03} type={} p={}", self.depth, tmp.0, tmp.1);
        }
        match d {
            PathBox::File(f) => {
                match ImgInfo::new(f) {
                    Ok(i) => {
                        if self.ignore_unknown_types {
                            match i.file_type() {
                                FileType::Other => {},
                                _ => { controller.process(i); },
                            }
                        }
                        else {
                            controller.process(i);
                        }
                    },
                    Err(e) => println!("Error processing file: {}", e)
                }
            },
            PathBox::Directory(d) => {
                if self.depth < self.max_depth {
                    self.depth += 1;
                    for child in d.read_dir().expect("Error reading path a directory!") {
                        let child_path = child.expect("Error reading child!").path();
                        self.scan_path_ch(PathBox::from(child_path), controller);
                    }
                    self.depth -= 1;
                }
            }
        }
    }
}
