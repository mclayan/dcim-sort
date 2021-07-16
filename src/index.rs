use crate::image::{ImgInfo};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::panic::panic_any;
use crate::index::PathBox::Directory;
use chrono::Local;

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
    entry_point: String,
    max_depth: u8,
    depth: u8,
    debug: bool
}

impl Scanner {
    pub fn new(root_path: String) -> Result<Scanner, Error> {
        let path = PathBuf::from(&root_path);
        if !path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "root must be a directory!"));
        }
        else {
            Ok(Scanner{
                entry_point: root_path,
                max_depth: 10,
                depth: 0,
                debug: false
            })
        }
    }

    pub fn debug(&mut self, b: bool) {
        self.debug = b;
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
        let root = PathBuf::from(&self.entry_point);
        self.scan_path(PathBox::from(root), &mut index);
        index
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
                    Ok(i) => index.push(i),
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
}
