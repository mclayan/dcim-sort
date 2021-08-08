use crate::pattern::{PatternElement};
use std::path::{PathBuf, Path};
use std::fs::{File};
use crate::media::{ImgInfo, FileType};
use std::io::Error;

#[derive(Clone, Copy)]
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
    fallback_segments: Vec<Box<dyn PatternElement>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    duplicate_counter: u64,
    skipped_files: u64,
    sorted_files: u64,
    created_dirs: u64,
}

pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement>>,
    fallback_segments: Vec<Box<dyn PatternElement>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
}

impl SorterBuilder {
    /// Add a segment pattern to the internal vec of segments for sorting
    /// files with supported metadata.
    pub fn segment(mut self, s: Box<dyn PatternElement>) -> SorterBuilder {
        self.segments.push(s);
        self
    }

    /// Add a segment pattern to the internal vec of segments for sorting
    /// files without supported metadata.
    pub fn fallback(mut self, s: Box<dyn PatternElement>) -> SorterBuilder {
        self.fallback_segments.push(s);
        self
    }

    /// set how files already existing in the target directory with the
    /// same name should be handled.
    pub fn duplicate_handling(mut self, a: DuplicateResolution) -> SorterBuilder {
        self.dup_handling = a;
        self
    }

    pub fn build(self) -> Sorter {
        Sorter {
            segments: self.segments,
            fallback_segments: self.fallback_segments,
            dup_handling: self.dup_handling,
            target_root: self.target_root,
            duplicate_counter: 0,
            skipped_files: 0,
            sorted_files: 0,
            created_dirs: 0
        }
    }
}

impl Sorter {
    pub fn new(target_dir: PathBuf) -> SorterBuilder {
        SorterBuilder {
            segments: Vec::new(),
            fallback_segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            target_root: target_dir.clone(),
        }
    }

    pub fn sort_all(&mut self, index: &Vec<ImgInfo>, strategy: Strategy) {
        for info in index {
            self.sort(info, strategy);
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
            if !destination.exists() {
                println!("mkdir: {}", destination.to_str().unwrap_or("INVALID_UTF-8"));
                match std::fs::create_dir_all(destination) {
                    Err(e) => {
                        println!("Failed to create destination directory: {}", e);
                        return;
                    }
                    Ok(_) => { self.created_dirs += 1 }
                }
            }
            let result: Result<Option<u64>, Error> = match strategy {
                Strategy::Copy => {
                    match Sorter::copy_file(target.as_path(), img.path()) {
                        Ok(i) => Ok(Some(i)),
                        Err(e) => Err(e)
                    }
                },
                Strategy::Move => {
                    match Sorter::move_file(target, img.path()) {
                        Ok(_) => Ok(None),
                        Err(e) => Err(e)
                    }
                }
            };
            match result {
                Ok(_) => { self.sorted_files += 1; },
                Err(e) => {
                    eprintln!("Error sorting file {}: {}", img.path().to_str().unwrap_or("INVALID_UTF-8"), e);
                }
            }

        }
        else {
            self.skipped_files += 1;
        }
    }

    fn move_file(dest: PathBuf, source: &Path) -> std::io::Result<()> {
        let result = std::fs::rename(source, dest);
        result
    }

    fn copy_file(dest: &Path, source: &Path) -> std::io::Result<u64> {
        let result = std::fs::copy(source, dest);
        result
    }

    pub fn translate(&mut self, img: &ImgInfo) -> PathBuf {
        let mut dest = self.target_root.clone();
        match img.file_type() {
            FileType::Other => { self.translate_unsupported(img, dest) },
            _ =>               { self.translate_supported(img, dest)   },
        }
    }

    fn translate_supported(&mut self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
        for pattern in &self.segments {
            if let Some(seg_str) = pattern.translate(img) {
                dest.push(seg_str);
            }
        }
        dest
    }

    fn translate_unsupported(&mut self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
        for pattern in &self.fallback_segments {
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

        let m1 = file1.metadata().expect("Failed to read metadata!");
        let m2 = file2.metadata().expect("Failed to read metadata!");

        let d1 = m1.modified().expect("Failed to read modified time!");
        let d2 = m2.modified().expect("Failed to read modified time!");

        d1 == d2 && m1.len() == m2.len()

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
