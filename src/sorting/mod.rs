use std::io::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::logging::LogMsg;
use crate::{HashAlgorithm, LogReq};
use crate::media::{FileType, ImgInfo};
use crate::pattern::PatternElement;
use crate::pipeline::Report;
use crate::sorting::comparison::{Cause, ComparisonErr, FileComparer};
use crate::sorting::fs_support::DirCreationRequest;

pub mod fs_support;
pub mod comparison;

/// Sorting Operation to perform on files sorted.
///
/// # Variants
///
///  - Copy: copy the file only, leave original in the source folder
///  - Move: move the source file to the target folder
///  - Print: only print what the target file would be after pattern evaluation without doing anything
#[derive(Clone, Copy)]
pub enum Operation {
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
    comparer: FileComparer
}

pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement + Send>>,
    fallback_segments: Vec<Box<dyn PatternElement + Send>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    log: Option<mpsc::Sender<LogReq>>,
    build_count: u32,
    hash_algo: HashAlgorithm
}

enum PreCheckResult {
    Execute,
    Skip,
    RenameTarget,
    Error(String)
}
impl PreCheckResult {
    pub fn to_str(&self) -> &'static str {
        match self {
            PreCheckResult::Execute => "Execute",
            PreCheckResult::Skip => "Skip",
            PreCheckResult::RenameTarget => "Rename",
            PreCheckResult::Error(_) => "Error"
        }
    }
}

impl SorterBuilder {

    /// Generates a unique ID for each sorter built for logging
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

    /// add a channel connected to a logger
    pub fn log(mut self, log: mpsc::Sender<LogReq>) -> SorterBuilder {
        self.log = Some(log);
        self
    }

    /// set the hash algorithm for comparing
    pub fn hash_algorithm(mut self, algo: HashAlgorithm) -> SorterBuilder {
        self.hash_algo = algo;
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

    /// add a supported path segment to the end of the list
    pub fn push_segment_supported(&mut self, s: Box<dyn PatternElement + Send>) {
        self.segments.push(s);
    }

    /// add a fallback path segment to the end of the list
    pub fn push_segment_fallback(&mut self, s: Box<dyn PatternElement + Send>) {
        self.fallback_segments.push(s);
    }

    /// consume the current builder and produce a single instance of a sorter
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
            log_id: log_id,
            comparer: FileComparer::default()
        }
    }

    /// build a single sorter instance without consuming the builder
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
            log_id: self.generate_log_id(),
            comparer: FileComparer::new(false, self.hash_algo)
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
    /// default duplicate handling strategy
    pub fn def_duplicate_handling() -> DuplicateResolution {
        DuplicateResolution::Ignore
    }

    /// create a new builder configured with `target_dir` as the target root folder
    pub fn new(target_dir: PathBuf) -> SorterBuilder {
        SorterBuilder {
            segments: Vec::new(),
            fallback_segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            target_root: target_dir.clone(),
            log: None,
            build_count: 0,
            hash_algo: HashAlgorithm::None
        }
    }

    /// sort all items in `index` according to `operation`
    pub fn sort_all(&mut self, index: &Vec<ImgInfo>, operation: Operation) {
        for info in index {
            self.sort(info, operation);
        }
        match operation {
            Operation::Print => {
                for dir in &self.dirs_to_create {
                    println!("mkdir: {}", dir);
                }
                println!("=======[ Simulation Report ]=========");
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

    /// sort a single source file
    pub fn sort(&mut self, img: &ImgInfo, operation: Operation) {
        let destination = self.translate(img);
        let mut target = destination.clone();
        let filename = img.path()
            .file_name().expect("filename could not be read!")
            .to_str().expect("filename is not a valid UTF-8 str!");
        target.push(filename);

        let precheck_result = self.evaluate_execution(img.path(), target.as_path());

        match precheck_result {
            PreCheckResult::Execute => {
                match self.execute(img, destination, target, operation) {
                    Ok(_) => (),
                    Err(e) => self.log_msg(format!("Failed to execute: {}", e))
                }
            },
            PreCheckResult::Skip => {
                self.skipped_files += 1;
                let log_msg = format!("{} -> {}: file exists, skipping",
                                      img.path().to_str().unwrap_or("<INVALID UTF-8>"),
                                      target.to_str().unwrap_or("<INVALID UTF-8>")
                );
                self.log_msg(log_msg);
            }
            PreCheckResult::RenameTarget => {
                if let Some(t) = Sorter::find_dup_free_name(destination.as_path(), filename) {
                    target = t;
                    match self.execute(img, destination, target, operation) {
                        Ok(_) => (),
                        Err(e) => self.log_msg(format!("Failed to execute: {}", e))
                    }
                }
            }
            PreCheckResult::Error(e) => {
                let msg = format!("Error comparing files: {}", e);
                self.log_msg(msg);
            }
        }
    }

    fn log_msg(&self, msg: String) {
        if let Some(log) = &self.log {
            match log.send(LogReq::Msg(LogMsg::new(self.log_id.clone(), msg))) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to log message: {}", e);
                }
            }
        }
    }

    /// execute `operation` on the input file `img`, which may be either copying or moving to
    /// `target`, which needs to consist of target folder and target filename. Will always overwrite
    /// `target` if it exists.
    fn execute(&mut self, img: &ImgInfo, destination: PathBuf, target: PathBuf, operation: Operation) -> Result<(), String> {
        if !destination.exists() {
            if self.create_dir(&destination) == false {
                eprintln!("[WARN] received negative creation confirmation for target_dir=\"{}\"",
                          destination.to_str().unwrap_or("<INVALID UTF-8>")
                );
            }
            if matches!(operation, Operation::Print) {
                self.dirs_to_create.push(String::from(destination.to_str().unwrap_or("<INVALID_UTF-8>")));
            }
        }
        let result: Result<Option<u64>, Error> = match operation {
            Operation::Copy => {
                match Sorter::copy_file(target.as_path(), img.path()) {
                    Ok(i) => Ok(Some(i)),
                    Err(e) => Err(e)
                }
            },
            Operation::Move => {
                match Sorter::move_file(target, img.path()) {
                    Ok(_) => Ok(None),
                    Err(e) => Err(e)
                }
            },
            Operation::Print => {
                println!("{} -> {}",
                         img.path().to_str().unwrap_or("INVALID_UTF-8"),
                         target.to_str().unwrap_or("INVALID_UTF-8")
                );
                Ok(None)
            }
        };
        match result {
            Ok(_) => {
                self.sorted_files += 1;
                Ok(())
            },
            Err(e) => {
                Err(format!("Failed to sort file from src=\"{}\": {}",
                    img.path().to_str().unwrap_or("INVALID_UTF-8"),
                    e
                ))
            }
        }
    }

    /// perform a pre-check on the operation to determine if it should be executed according to the
    /// policy of handling duplicates (if the target exists).
    fn evaluate_execution(&mut self, src: &Path, target: &Path) -> PreCheckResult {
        if !src.is_file() {
            return PreCheckResult::Error(
                format!("source file does not exist: {}",
                        src.to_str().unwrap_or("<INVALID_UTF-8>")
                )
            );
        }
        if !target.exists() {
            return PreCheckResult::Execute;
        }
        else {
            self.duplicate_counter += 1;
        }

        // both src and target exist, evaluate strategy
        match &self.dup_handling {
            // duplicate files are ignored and remain in the source dir
            DuplicateResolution::Ignore => PreCheckResult::Skip,
            // duplicate files are overwritten without comparing
            DuplicateResolution::Overwrite => PreCheckResult::Execute,
            // duplicate files are compared and handled according to Comparison policy
            DuplicateResolution::Compare(c) => {
                match self.comparer.check_files_matching(src, target) {
                    Ok(b) => match b {
                        // files match, no need to do anything
                        true => PreCheckResult::Skip,
                        // files differ, check policy
                        false => match c {
                            // rename target to keep both files
                            Comparison::Rename => PreCheckResult::RenameTarget,
                            // favour target, skip
                            Comparison::FavorTarget => PreCheckResult::Skip,
                            // overwrite target with source
                            Comparison::FavorSource => PreCheckResult::Execute
                        }
                    },
                    Err(e) => PreCheckResult::Error(Self::create_cmp_err_msg(e, src, target))
                }
            }
        }
    }

    /// request DirManager to create the destination directory, waits for confirmation
    fn create_dir(&self, destination: &PathBuf) -> bool {
        let req = DirCreationRequest::new(destination, self.tx_callback.clone());
        self.tx_dir_creation.send(req).expect("could not send dir creation request!");
        match self.rx_callback.recv() {
            Ok(b) => {
                match b {
                    true => true,
                    false => {
                        eprintln!("failed to create destination dir: {}", destination.to_str().unwrap_or("<INVALID_UTF-8>"));
                        false
                    }
                }
            }
            Err(e) => {
                panic!("Could not receive callback from DirManager: {}", e);
            }
        }
    }

    /// move a file from `source` to `dest`, overwriting existing files
    fn move_file(dest: PathBuf, source: &Path) -> std::io::Result<()> {
        assert!(!dest.is_dir());
        assert!(source.is_file());

        let result = std::fs::rename(source, dest);
        result
    }

    /// copy `source` to `dest`, overwriting existing files
    fn copy_file(dest: &Path, source: &Path) -> std::io::Result<u64> {
        assert!(!dest.is_dir());
        assert!(source.is_file());

        let result = std::fs::copy(source, dest);
        result
    }

    /// translate a file based on its pattern-relevant data into a path relative to `self.target_root`
    pub fn translate(&self, img: &ImgInfo) -> PathBuf {
        let dest = self.target_root.clone();
        match img.file_type() {
            FileType::Other => { self.translate_unsupported(img, dest) },
            _ =>               { self.translate_supported(img, dest)   },
        }
    }

    /// translate a file based on its pattern-relevant data into a path relative to `self.target_root`
    /// using [PatternElements] for supported file types
    fn translate_supported(&self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
        for pattern in &self.segments {
            if let Some(seg_str) = pattern.translate(img) {
                dest.push(seg_str);
            }
        }
        dest
    }

    /// translate a file based on its pattern-relevant data into a path relative to `self.target_root`
    /// using [PatternElements] for unsupported file types
    fn translate_unsupported(&self, img: &ImgInfo, mut dest: PathBuf) -> PathBuf {
        for pattern in &self.fallback_segments {
            if let Some(seg_str) = pattern.translate(img) {
                dest.push(seg_str);
            }
        }
        dest
    }

    /// process a [ComparisonErr] into a readable error message
    fn create_cmp_err_msg(e: ComparisonErr, f1: &Path, f2: &Path) -> String {
        let mut cause: Option<&Path> = None;
        let mut msg: Option<String> = None;
        match e {
            ComparisonErr::AccessDenied(c) => {
                cause = match c {
                    Cause::Source => Some(f1),
                    Cause::Target => Some(f2),
                    Cause::NA => None
                };
                msg = Some(String::from("access is denied"));
            }
            ComparisonErr::InvalidFile(c) => {
                cause = match c {
                    Cause::Source => Some(f1),
                    Cause::Target => Some(f2),
                    Cause::NA => None
                };
                msg = Some(String::from("file not found"))
            }
            ComparisonErr::Metadata(c) => {
                cause = match c {
                    Cause::Source => Some(f1),
                    Cause::Target => Some(f2),
                    Cause::NA => None
                };
                msg = Some(String::from("file metadata could not be read"))
            }
            ComparisonErr::Other(c, m) => {
                cause = match c {
                    Cause::Source => Some(f1),
                    Cause::Target => Some(f2),
                    Cause::NA => None
                };
                msg = m;
            }
        }

        format!("error accessing file=\"{}\": {}",
            cause.unwrap().to_str().unwrap_or("<INVALID_UTF-8>"),
            msg.unwrap()
        )
    }

    /// mutate a filename to be unique in `target_folder` by adding incrementing numbers as an
    /// additional suffix in the pattern `<original_filename>.<counter>` where `<counter>` will be
    /// a decimal in range 0 to 999 represented with a fixed width of 3 chars (e.g. `012`).
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

    /// get the number of segments in tuple of `(count_supported, count_unsupported)`
    pub fn get_seg_count(&self) -> (usize,usize) {
        (self.segments.len(), self.fallback_segments.len())
    }

    /// get a slice of supported segments
    pub fn get_segments_supported(&self) -> &[Box<dyn PatternElement + Send>] {
        &self.segments[..]
    }

    /// get a report of processed files
    pub fn get_report(&self) -> Report {
        Report {
            count_success: self.sorted_files,
            count_skipped: self.skipped_files,
            count_duplicate: self.duplicate_counter
        }
    }
}
