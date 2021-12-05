use std::fs::File;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::logging::LogMsg;
use crate::LogReq;
use crate::media::{FileType, ImgInfo};
use crate::pattern::PatternElement;
use crate::pipeline::Report;
use crate::sorting::fs_support::DirCreationRequest;

pub mod fs_support;

#[derive(Clone, Copy)]
pub enum Strategy {
    Copy,
    Move,
    Print
}

#[derive(Clone, Copy)]
pub enum Comparison {
    Rename,
    FavorTarget,
    FavorSource,
}
#[derive(Clone, Copy)]
pub enum DuplicateResolution {
    Ignore,
    Overwrite,
    Compare(Comparison),
}

pub struct Sorter {
    segments: Vec<Box<dyn PatternElement + Send>>,
    fallback_segments: Vec<Box<dyn PatternElement + Send>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    duplicate_counter: u64,
    skipped_files: u64,
    sorted_files: u64,
    created_dirs: u64,
    dirs_to_create: Vec<String>,
    rx_callback: mpsc::Receiver<bool>,
    tx_callback: mpsc::Sender<bool>,
    tx_dir_creation: mpsc::Sender<DirCreationRequest>,
    log: Option<mpsc::Sender<LogReq>>,
    log_id: String,
}

pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement + Send>>,
    fallback_segments: Vec<Box<dyn PatternElement + Send>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    log: Option<mpsc::Sender<LogReq>>,
    build_count: u32
}

impl SorterBuilder {
    fn generate_log_id(&mut self) -> String {
        let id = format!("sorter@{:02}", self.build_count);
        self.build_count += 1;
        id
    }

    /// Add a segment pattern to the internal vec of segments for sorting
    /// files with supported metadata.
    pub fn segment(mut self, s: Box<dyn PatternElement + Send>) -> SorterBuilder {
        self.push_segment_supported(s);
        self
    }

    pub fn log(mut self, log: mpsc::Sender<LogReq>) -> SorterBuilder {
        self.log = Some(log);
        self
    }

    /// Add a segment pattern to the internal vec of segments for sorting
    /// files without supported metadata.
    pub fn fallback(mut self, s: Box<dyn PatternElement + Send>) -> SorterBuilder {
        self.push_segment_fallback(s);
        self
    }

    /// set how files already existing in the target directory with the
    /// same name should be handled.
    pub fn duplicate_handling(mut self, a: DuplicateResolution) -> SorterBuilder {
        self.dup_handling = a;
        self
    }

    pub fn push_segment_supported(&mut self, s: Box<dyn PatternElement + Send>) {
        self.segments.push(s);
    }

    pub fn push_segment_fallback(&mut self, s: Box<dyn PatternElement + Send>) {
        self.fallback_segments.push(s);
    }

    pub fn build(mut self, dir_creation_tx: mpsc::Sender<DirCreationRequest>) -> Sorter {
        let (tx, rx) = mpsc::channel::<bool>();
        let segs = self.clone_segs();
        let log_id = self.generate_log_id();
        Sorter {
            segments: segs.0,
            fallback_segments: segs.1,
            dup_handling: self.dup_handling,
            target_root: self.target_root,
            duplicate_counter: 0,
            skipped_files: 0,
            sorted_files: 0,
            created_dirs: 0,
            dirs_to_create: Vec::new(),
            rx_callback: rx,
            tx_callback: tx,
            tx_dir_creation: dir_creation_tx,
            log: self.log,
            log_id: log_id
        }
    }

    pub fn build_clone(&mut self, dir_creation_tx: mpsc::Sender<DirCreationRequest>) -> Sorter {
        let (tx, rx) = mpsc::channel::<bool>();
        let segs = self.clone_segs();
        Sorter {
            segments: segs.0,
            fallback_segments: segs.1,
            dup_handling: self.dup_handling,
            target_root: self.target_root.clone(),
            duplicate_counter: 0,
            skipped_files: 0,
            sorted_files: 0,
            created_dirs: 0,
            dirs_to_create: Vec::new(),
            rx_callback: rx,
            tx_callback: tx,
            tx_dir_creation: dir_creation_tx,
            log: self.log.clone(),
            log_id: self.generate_log_id()
        }
    }

    fn clone_segs(&self) -> (Vec<Box<dyn PatternElement + Send>>, Vec<Box<dyn PatternElement + Send>>) {
        let mut segs = Vec::<Box<dyn PatternElement + Send>>::with_capacity(self.segments.len());
        let mut fb_segs = Vec::<Box<dyn PatternElement + Send>>::with_capacity(self.fallback_segments.len());

        for s in &self.segments {
            segs.push(s.clone_boxed());
        }

        for s in &self.fallback_segments {
            fb_segs.push(s.clone_boxed());
        }

        (segs, fb_segs)
    }
}

impl Sorter {
    pub fn def_duplicate_handling() -> DuplicateResolution {
        DuplicateResolution::Ignore
    }

    pub fn new(target_dir: PathBuf) -> SorterBuilder {
        SorterBuilder {
            segments: Vec::new(),
            fallback_segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            target_root: target_dir.clone(),
            log: None,
            build_count: 0
        }
    }

    pub fn sort_all(&mut self, index: &Vec<ImgInfo>, strategy: Strategy) {
        for info in index {
            self.sort(info, strategy);
        }
        match strategy {
            Strategy::Print => {
                for dir in &self.dirs_to_create {
                    println!("mkdir: {}", dir);
                }
                println!("=======[ Simulation Report]=========");
                println!("total: {}\nfiles_sorted: {}\nfiles_skipped: {}\nduplicates: {}\ndirs_created: {}\n",
                         index.len(),
                         self.sorted_files,
                         self.skipped_files,
                         self.duplicate_counter,
                         self.dirs_to_create.len()
                );
            },
            _ => ()
        };
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
                let req = DirCreationRequest::new(&destination, self.tx_callback.clone());
                self.tx_dir_creation.send(req);
                match self.rx_callback.recv() {
                    Ok(b) => {
                        match b {
                            true => (),
                            false => eprintln!("failed to create destination dir: {}", destination.to_str().unwrap_or("<INVALID_UTF-8>"))
                        }
                    }
                    Err(e) => {
                        panic!("Could not receive callback from DirManager: {}", e);
                    }
                }
                if matches!(strategy, Strategy::Print) {
                    self.dirs_to_create.push(String::from(destination.to_str().unwrap_or("<INVALID_UTF-8>")));
                }
                /*
                match strategy {
                    Strategy::Copy | Strategy::Move => {
                        match std::fs::create_dir_all(destination) {
                            Err(e) => {
                                println!("Failed to create destination directory: {}", e);
                                return;
                            }
                            Ok(_) => { self.created_dirs += 1 }
                        }
                    }
                    _ => {
                        let path_str = String::from(destination.to_str().unwrap_or("INVALID_UTF-8"));
                        if !self.dirs_to_create.contains(&path_str) {
                            self.dirs_to_create.push(path_str);
                        }
                    }
                }
                 */
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
                },
                Strategy::Print => {
                    println!("{} -> {}",
                             img.path().to_str().unwrap_or("INVALID_UTF-8"),
                             target.to_str().unwrap_or("INVALID_UTF-8")
                    );
                    Ok(None)
                }
            };
            match result {
                Ok(_) => { self.sorted_files += 1; },
                Err(e) => {
                    eprintln!("[Sorter] Error sorting file {}: {}", img.path().to_str().unwrap_or("INVALID_UTF-8"), e);
                }
            }

        }
        else {
            self.skipped_files += 1;
            if let Some(log) = &self.log {
                let log_msg = format!("{} -> {}: file exists",
                                      img.path().to_str().unwrap_or("<INVALID UTF-8>"),
                                      target.to_str().unwrap_or("<INVALID UTF-8>")
                );
                log.send(LogReq::Msg(LogMsg::new(self.log_id.clone(), log_msg))).expect("failed to send log message!");
            }
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

    pub fn translate(&self, img: &ImgInfo) -> PathBuf {
        let dest = self.target_root.clone();
        match img.file_type() {
            FileType::Other => { self.translate_unsupported(img, dest) },
            _ =>               { self.translate_supported(img, dest)   },
        }
    }

    fn translate_supported(&self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
        for pattern in &self.segments {
            if let Some(seg_str) = pattern.translate(img) {
                dest.push(seg_str);
            }
        }
        dest
    }

    fn translate_unsupported(&self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
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

        // todo: make this optional
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
            let name = format!("{}.{:03}", filename, counter);
            target.set_file_name(name);
        }
        Some(target)
    }

    pub fn get_seg_count(&self) -> (usize,usize) {
        (self.segments.len(), self.fallback_segments.len())
    }

    pub fn get_segments_supported(&self) -> &[Box<dyn PatternElement + Send>] {
        &self.segments[..]
    }

    pub fn get_report(&self) -> Report {
        Report {
            count_success: self.sorted_files,
            count_skipped: self.skipped_files,
            count_duplicate: self.duplicate_counter
        }
    }
}
