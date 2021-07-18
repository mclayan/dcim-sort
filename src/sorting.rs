use std::fmt::Formatter;
use crate::image::{ImgMeta, ImgInfo};
use crate::pattern::{PatternElement, SortingError};
use std::path::{PathBuf, Path};
use std::fs::{File, read_dir};

pub enum Strategy {
    Copy,
    Move
}
pub enum Comparison {
    Rename,
    FavorTarget,
    FavorSource,
}

pub enum DuplicateResolution {
    Ignore,
    Overwrite,
    Compare(Comparison),
}

pub struct Sorter {
    segments: Vec<Box<dyn PatternElement>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    duplicate_counter: u64,
}

pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
}

impl SorterBuilder {
    pub fn segment(mut self, s: Box<dyn PatternElement>) -> SorterBuilder {
        self.segments.push(s);
        self
    }

    pub fn duplicate_handling(mut self, a: DuplicateResolution) -> SorterBuilder {
        self.dup_handling = a;
        self
    }

    pub fn build(self) -> Sorter {
        Sorter {
            segments: self.segments,
            dup_handling: self.dup_handling,
            target_root: self.target_root,
            duplicate_counter: 0,
        }
    }
}

impl Sorter {
    pub fn new(target_dir: PathBuf) -> SorterBuilder {
        SorterBuilder {
            segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            target_root: target_dir.clone(),
        }
    }

    pub fn sort(&mut self, img: &ImgInfo, strategy: Strategy) {
        let destination = self.translate(img);
        let mut target = destination.clone();
        let filename = img.path()
            .file_name().expect("filename could not be read!")
            .to_str().expect("filename is not a valid UTF-8 str!");
        target.push(filename);

        let do_execute = match target.exists() {
            true => {
                self.duplicate_counter += 1;
                match &self.dup_handling {
                    DuplicateResolution::Ignore => { false }
                    DuplicateResolution::Overwrite => { true }
                    DuplicateResolution::Compare(c) => {
                        if Sorter::check_files_matching(img.path(), target.as_path()) {
                            match c {
                                Comparison::Rename => {
                                    if let Some(t) = Sorter::find_dup_free_name(destination.as_path(), filename) {
                                        target = t;
                                        true
                                    }
                                    else {
                                        false
                                    }
                                }
                                Comparison::FavorTarget => { false }
                                Comparison::FavorSource => { true }
                            }
                        }
                        else {
                            false
                        }
                    }
                }
            }
            false => true
        };

        if do_execute {
            match strategy {
                Strategy::Copy => { Sorter::copy_file(target, img.path()) },
                Strategy::Move => { Sorter::move_file(target, img.path() )}
            }
        }
    }

    fn move_file(dest: PathBuf, source: &Path) {
        todo!()
    }

    fn copy_file(dest: PathBuf, source: &Path) {
        std::fs::copy()
    }

    pub fn translate(&mut self, img: &ImgInfo) -> PathBuf {
        let mut dest = self.target_root.clone();
        for pattern in &self.segments {
            if let Some(seg_str) = pattern.translate(img) {
                dest.push(seg_str);
            }
        }
        dest
    }

    fn check_files_matching(f1: &Path, f2: &Path) -> bool {
        assert!(f1.is_file());
        assert!(f2.is_file());
        let file1 = File::open(f1).expect("Could not open file!");
        let file2 = File::open(f2).expect("Could not open file!");

        let s1 = file1.metadata().expect("Failed to read metadata!").len();
        let s2 = file2.metadata().expect("Failed to read metadata!").len();
        s1 == s2

        // todo: calculate checksum for comparison if sizes match
    }

    fn find_dup_free_name(target_folder: &Path, filename: &str) -> Option<PathBuf> {
        assert!(target_folder.is_dir());

        let mut counter: u16 = 0;
        let mut target = PathBuf::from(target_folder);
        target.set_file_name(filename);
        while target.exists() {
            if counter < 999 {
                counter += 1;
            } else {
                return None;
            }
            let mut name = format!("{}.{:03}", filename, counter);
            target.set_file_name(name);
        }
        Some(target)
    }
}
