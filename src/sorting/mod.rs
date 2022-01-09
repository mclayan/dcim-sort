use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::media::ImgInfo;
use crate::logging::LogReq;
use crate::pattern::PatternElement;
use crate::sorting::comparison::{HashAlgorithm, Cause, ComparisonErr, FileComparer};
use crate::sorting::fs_support::{DirCreationRequest, DirManager};
use crate::sorting::translation::Translator;

pub mod fs_support;
pub mod comparison;
pub mod translation;

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

/// Existing target files should be compared and handled according to the variant of this enum if
/// both files differ.
///
/// # Variants
/// - [Comparison::Rename] rename the target file and keep both
/// - [Comparison::FavorTarget] favour the target file by not overwriting it with the source file
/// - [Comparison::FavorSource] always overwrite the target with the source file
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

/// The result of a pre-check performed on a SortAction to detect possible existing target files
/// and evaluation of a policy that tells what to do in that case.
///
/// # Variants
/// - [PreCheckResult::Execute] The action should be executed as-is
/// - [PreCheckResult::Skip] The action should skipped
/// - [PreCheckResult::RenameTarget] The target filename should be renamed to avoid overwriting
/// - [PreCheckResult::Error] An error happened while evaluating the policy
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

/// A struct containing the bundled information of source file, target location + filename and
/// the operation to apply.
pub struct SortAction {
    operation: Operation,
    source: PathBuf,
    target: PathBuf
}
impl SortAction {
    pub fn target_exists(&self) -> bool {
        self.target.exists()
    }

    pub fn get_source(&self) -> &Path {
        self.source.as_path()
    }

    pub fn get_target(&self) -> &Path {
        self.target.as_path()
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

enum SorterMode {
    Sync(DirManager),
    Async(AsyncDirChannel)
}

/// Utility to perform the actual sorting including duplicate checks and resolution. It is
/// primarily designed for the following flow, processing an [ImgInfo] with existing metadata:
/// 1. Create SortAction: translates the known metadata into target path segments and bundles
///     them with the operation to apply
/// 2. Evaluate execution: perform some pre-checks to avoid overwriting any files in the target
///     directory. A policy defines how to handle existing targets (see [DuplicateResolution])
/// 3. Execute the action
///
/// Note: step no. 2 and 3 can be combined with [Sorter::execute_checked]
///
/// # Examples
///
/// ```rust
/// let target_root = PathBuf::from("sorted/");
/// let input_file = ImgInfo::new(PathBuf::from("input/IMG0001.JPG")).unwrap();
/// let sorter = Sorter::builder(target_root.as_path())
///     .segment(SimpleFileTypePattern::new().build())
///     .build_sync();
///
/// // create a new SortAction (here the ImgInfo has no metadata due to missing Pre-Processing)
/// let action = sorter.calc_copy(&input_file, target_root.as_path());
/// let result = sorter.execute_checked(action, &DuplicateResolution::Ignore);
/// ```
pub struct Sorter {
    translator: Translator,
    comparer: FileComparer,
    mode: SorterMode
}
impl Sorter {
    pub fn builder() -> SorterBuilder {
        // TODO: cleanup
        SorterBuilder {
            segments: Vec::new(),
            fallback_segments: Vec::new(),
            dup_handling: DuplicateResolution::Compare(Comparison::Rename),
            log: None,
            hash_algo: HashAlgorithm::None
        }
    }

    pub fn new(translator: Translator, comparer: FileComparer) -> Sorter {
        Sorter {
            translator: translator,
            comparer: comparer,
            mode: SorterMode::Sync(DirManager::new())
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

    /// create a new [SortAction] with operation=copy
    pub fn calc_copy(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Copy)
    }

    /// create a new [SortAction] with operation=move
    pub fn calc_move(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Move)
    }

    /// create a new [SortAction] with operation=simulate (print)
    pub fn calc_simulation(&self, file: &ImgInfo, target_root: &Path) -> SortAction {
        self.calc_action(file, target_root, Operation::Print)
    }


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

    /// execute an action with the given operation, consuming the input action.
    ///
    /// **WARNING:** does not perform any policy checks and will overwrite existing files.
    pub fn execute(&mut self, action: SortAction) -> Result<ActionResult, String> {
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
                    if parent.is_file() {
                        return Err(
                            format!("failed to create parent directory \"{}\": a normal file with that name already exists!",
                                parent.to_str().unwrap_or(PATHSTR_FB)
                            )
                        );
                    }
                    match &mut self.mode {
                        // synchronous mode, directly create path
                        SorterMode::Sync(dm) => dm.create_path(parent,
                                                               matches!(&action.operation, Operation::Print)
                        )?,
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
            Operation::Move => std::fs::rename(source, target),
            Operation::Copy => match std::fs::copy(source, target) {
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
    /// executing the action with an error message that can be printed.
    pub fn execute_checked(&mut self, mut action: SortAction, policy: &DuplicateResolution) -> Result<ActionResult, String> {
        let precheck_result = self.evaluate_execution(&action, policy);

        match precheck_result {
            PreCheckResult::Execute => self.execute(action),
            PreCheckResult::Skip => match &action.operation {
                Operation::Print => self.execute(action),
                _                => Ok(ActionResult::Skipped)
            },
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

    fn calc_action(&self, file: &ImgInfo, target_root: &Path, op: Operation) -> SortAction {
        let mut target_folder = self.translator.translate(file, target_root);
        let fname = file.path().file_name().expect("source filename is invalid!");
        target_folder.push(fname);
        SortAction{
            operation: op,
            source: file.path().to_path_buf(),
            target: target_folder,
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
}

/// A builder to generate new Sorter instances
pub struct SorterBuilder {
    segments: Vec<Box<dyn PatternElement + Send>>,
    fallback_segments: Vec<Box<dyn PatternElement + Send>>,
    dup_handling: DuplicateResolution,
    log: Option<mpsc::Sender<LogReq>>,
    hash_algo: HashAlgorithm
}
impl SorterBuilder {

    /// the default duplicate handling policy
    pub fn default_duplicate_handling() -> DuplicateResolution {
        DuplicateResolution::Ignore
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

    fn build_clone_translator(&mut self) -> Translator {
        let segs = self.clone_segs();
        Translator::new(segs.0, segs.1)
    }

    /// build a new synchronous builder
    pub fn build_sync(&mut self) -> Sorter {
        let translator = self.build_clone_translator();
        let comparer = FileComparer::new(false, self.hash_algo);
        Sorter::new(translator, comparer)
    }

    /// build a new asynchronous sorter
    pub fn build_async(&mut self, chan_dir_mgr: mpsc::Sender<DirCreationRequest>) -> Sorter {
        let translator = self.build_clone_translator();
        let comparer = FileComparer::new(false, self.hash_algo);

        Sorter::new_async(translator, comparer, chan_dir_mgr)
    }
}