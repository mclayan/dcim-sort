use std::io::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::logging::LogMsg;
use crate::{HashAlgorithm, LogReq};
use crate::media::{FileType, ImgInfo};
use crate::pattern::PatternElement;
use crate::pipeline::Report;
use crate::sorting::comparison::{Cause, ComparisonErr, FileComparer};
use crate::sorting::fs_support::{DirCreationRequest, DirManager};
use crate::sorting::translation::Translator;

pub mod fs_support;
pub mod comparison;
mod translation;
mod next;

/// a fallback string in case an OsStr could not be transformed to a [std::String]
pub static PATHSTR_FB: &str = "<INVALID_UTF-8>";


struct AsyncDirChannel {
    tx_dirm: mpsc::Sender<DirCreationRequest>,
    rx_callback: mpsc::Receiver<bool>,
    tx_callback: mpsc::Sender<bool>
}

impl AsyncDirChannel {
    pub fn new(chan_dirmgr: mpsc::Sender<DirCreationRequest>) -> AsyncDirChannel {
        let (tx_cb, rx_cb) = mpsc::channel::<bool>();
        AsyncDirChannel{
            tx_dirm: chan_dirmgr,
            rx_callback: rx_cb,
            tx_callback: tx_cb
        }
    }
}

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
impl Operation {
    pub fn to_str(&self) -> &'static str {
        match self {
            Operation::Copy => "copy",
            Operation::Move => "move",
            Operation::Print => "print"
        }
    }
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

pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement + Send>>,
    fallback_segments: Vec<Box<dyn PatternElement + Send>>,
    dup_handling: DuplicateResolution,
    target_root: PathBuf,
    log: Option<mpsc::Sender<LogReq>>,
    build_count: u32,
    hash_algo: HashAlgorithm
}

pub enum PreCheckResult {
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

    // defaults ====>
    pub fn default_duplicate_handling() -> DuplicateResolution {
        DuplicateResolution::Ignore
    }
    // <====

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
        self.build_clone(dir_creation_tx)
    }

    /// build a single sorter instance without consuming the builder
    pub fn build_clone(&mut self, dir_creation_tx: mpsc::Sender<DirCreationRequest>) -> Sorter {
        /*
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
         */
        self.build_async(dir_creation_tx)
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

    // ================================================================================
    fn build_clone_translator(&mut self) -> Translator {
        let segs = self.clone_segs();
        Translator::new(segs.0, segs.1)
    }

    pub fn build_sync(&mut self) -> Sorter {
        let translator = self.build_clone_translator();
        let comparer = FileComparer::new(false, self.hash_algo);
        Sorter::new(translator, comparer)
    }

    pub fn build_async(&mut self, chan_dir_mgr: mpsc::Sender<DirCreationRequest>) -> Sorter {
        let translator = self.build_clone_translator();
        let comparer = FileComparer::new(false, self.hash_algo);

        Sorter::new_async(translator, comparer, chan_dir_mgr)
    }
}

pub struct SortAction {
    operation: Operation,
    source: PathBuf,
    target: PathBuf
}
impl SortAction {
    pub fn target_exists(&self) -> bool {
        self.target.exists()
    }
}

/// An indicator of what has been performed when executing a [SortAction].
///
/// # Variants
/// - [ActionResult::Moved] the file has been moved to the target
/// - [ActionResult::Copied] the file has been copied to the target and still exists in source
/// - [ActionResult::Skipped] no effective action has been performed and the source file still exists
pub enum ActionResult {
    Moved,
    Copied,
    Skipped
}

enum SorterMode {
    Sync,
    Async(AsyncDirChannel)
}

pub struct Sorter {
    translator: Translator,
    comparer: FileComparer,
    mode: SorterMode
}

/// error to indicate that mutating a filename for conflict resolution failed.
///
/// # Variants
///
/// - InvalidTarget: the target does not exist and therefore mutating it cannot be perrformed
/// - Failed: mutating the target filename did not resolve the conflict after too many tries
pub enum MutationErr {
    InvalidTarget,
    Failed
}

impl Sorter {
    pub fn builder(target_dir: &Path) -> SorterBuilder {
        // TODO: cleanup
        SorterBuilder {
            segments: Vec::new(),
            fallback_segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            target_root: target_dir.to_path_buf(),
            log: None,
            build_count: 0,
            hash_algo: HashAlgorithm::None
        }
    }

    pub fn new(translator: Translator, comparer: FileComparer) -> Sorter {
        Sorter {
            translator: translator,
            comparer: comparer,
            mode: SorterMode::Sync
        }
    }

    pub fn new_async(translator: Translator, comparer: FileComparer, dir_chan: mpsc::Sender<DirCreationRequest>) -> Sorter {
        Sorter {
            translator: translator,
            comparer: comparer,
            mode: SorterMode::Async(
                AsyncDirChannel::new(dir_chan)
            )
        }
    }

    /// get the number of segments in a tuple of (<supported>, <fallback>)
    pub fn get_seg_count(&self) -> (usize, usize) {
        self.translator.get_seg_count()
    }

    // public functions to create initial actions ====>
    pub fn calc_copy(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Copy)
    }

    pub fn calc_move(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Move)
    }

    pub fn calc_simulation(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Print)
    }
    // <====

    // helper functions to implement duplicate policy handling ====>

    /// perform a pre-check on the operation to determine if it should be executed according to the
    /// policy of handling duplicates (if the target exists).
    pub fn evaluate_execution(&self, action: &SortAction, policy: &DuplicateResolution) -> PreCheckResult {
        let src = action.source.as_path();
        let target = action.target.as_path();

        if !src.is_file() {
            return PreCheckResult::Error(
                format!("source file does not exist: {}",
                        src.to_str().unwrap_or(PATHSTR_FB)
                )
            );
        }
        if !target.exists() {
            return PreCheckResult::Execute;
        }

        // both src and target exist, evaluate strategy
        match policy {
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

    /// mutate a filename to be unique in `target_folder` by adding incrementing numbers as an
    /// additional suffix in the pattern `<original_filename>.<counter>` where `<counter>` will be
    /// a decimal in range 0 to 999 represented with a fixed width of 3 chars (e.g. `012`).
    pub fn mutate_target_filename(mut action: SortAction) -> Result<SortAction, MutationErr> {
        if !action.target.exists() {
            return Err(MutationErr::InvalidTarget);
        }

        let mut target = action.target.clone();
        let filename = match action.target.file_name() {
            Some(name) => match name.to_str() {
                Some(s) => s,
                None => return Err(MutationErr::InvalidTarget)
            },
            None => return Err(MutationErr::InvalidTarget)
        };

        let mut counter: u16 = 1;

        while target.exists() {
            let name = format!("{}.{:03}", filename, counter);
            &target.set_file_name(name);
            if counter < 999 {
                counter += 1;
            } else {
                return Err(MutationErr::Failed);
            }
        }
        action.target = target;

        Ok(action)
    }
    // <====

    // execution functions ====>

    /// execute an action with the given operation, consuming the input action.
    ///
    /// **WARNING:** does not perform any policy checks and will overwrite existing files.
    pub fn execute(&self, action: SortAction) -> Result<ActionResult, String> {
        let (source, target) = (action.source.as_path(), action.target.as_path());

        // pre-checks to assure operation can be completed
        if !source.is_file() {
            return Err(format!("Invalid operation, source file does not exist: \"{}\"",
                &action.source.to_str().unwrap_or(PATHSTR_FB)
            ));
        }

        // check if any parent directories have to be created
        match target.parent() {
            // no parent dir that may have to be created
            None => (),
            // parent dir, check if exists
            Some(parent) => {
                if !parent.is_dir() {
                    match &self.mode {
                        // synchronous mode, directly create path
                        SorterMode::Sync => DirManager::create_path(parent)?,
                        // asynchronous mode, request creation via channel
                        SorterMode::Async(chan) => {
                            let req = DirCreationRequest::new(parent, chan.tx_callback.clone());
                            chan.tx_dirm.send(req).expect("Failed to send dir creation request: channel is closed");
                            let result = chan.rx_callback.recv().expect("Error receiving callback: channel is closed or hung up");
                            if !result {
                                return Err(format!("Could not create target directory \"{}\": DirMgr returned false",
                                    parent.to_str().unwrap_or(PATHSTR_FB)
                                ));
                            }
                        }
                    }
                }
            }
        }

        let result = match &action.operation {
            Operation::Copy => std::fs::rename(source, target),
            Operation::Move => match std::fs::copy(source, target) {
                    Ok(bytes) => {
                        if bytes <= 0 {
                            println!("[WARN]: copied {} bytes for src=\"{}\"",
                                     bytes,
                                     &action.source.to_str().unwrap_or(PATHSTR_FB)
                            );
                        }
                        Ok(())
                    },
                    Err(e) => Err(e)
            },
            Operation::Print => {
                println!("\"{}\" -> \"{}\"",
                    source.to_str().unwrap_or(PATHSTR_FB),
                    target.to_str().unwrap_or(PATHSTR_FB),
                );
                Ok(())
            }
        };

        match result {
            Ok(_) => Ok(match &action.operation {
                Operation::Print => ActionResult::Skipped,
                Operation::Move => ActionResult::Moved,
                Operation::Copy => ActionResult::Copied
            }),
            Err(e) => Err(format!("failed to execute operation=\"{}\": {}",
                &action.operation.to_str(),
                e
            ))
        }
    }

    /// consume an action and execute an operation following a policy pre-check (see
    /// [Self::evaluate_execution]), returning the action which has actually been
    /// performed. If indicated by the pre-check, the target filename may be mutated
    /// to resolve conflicting filenames in the target directory.
    ///
    /// # Errors
    /// This functions returns an [Err(String)] in case any errors were received while
    /// executing the action with an error message taht can be printed.
    pub fn execute_checked(&self, mut action: SortAction, policy: &DuplicateResolution) -> Result<ActionResult, String> {
        let precheck_result = self.evaluate_execution(&action, policy);

        match precheck_result {
            PreCheckResult::Execute => self.execute(action),
            PreCheckResult::Skip => Ok(ActionResult::Skipped),
            PreCheckResult::RenameTarget => {
                action = match Self::mutate_target_filename(action) {
                    Ok(a) => a,
                    Err(e) => {
                        return Err(format!("error renaming target: {}", match e {
                            MutationErr::InvalidTarget => "the target does not exist",
                            MutationErr::Failed => "a non-conflicting filename could not be created"
                        }
                        ));
                    }
                };
                self.execute(action)
            }
            PreCheckResult::Error(e) => Err(e)
        }
    }

    // <====

    // private helper functions ====>

    fn calc_action(&self, file: &ImgInfo, target_root: &Path, op: Operation) -> SortAction {
        let target = self.translator.translate(file, target_root);
        SortAction{
            operation: op,
            source: file.path().to_path_buf(),
            target: target,
        }
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

    // <====
}
