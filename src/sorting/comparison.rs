use std::fs::{File, Metadata};
use std::io::{BufReader, Read};
use std::path::{Path};

use md5::{Digest, Md5};
use md5::digest::generic_array::{GenericArray};
use sha2::Sha256;

pub static HASH_ALGO_NAMES: [(&str, HashAlgorithm); 3] = [("md5", HashAlgorithm::MD5), ("sha256", HashAlgorithm::SHA256), ("none", HashAlgorithm::None)];

#[derive(Copy, Clone)]
pub enum HashAlgorithm {
    MD5,
    SHA256,
    None
}
impl HashAlgorithm {
    pub fn parse(s: &str) -> HashAlgorithm {
        let mut result = HashAlgorithm::None;
        let inp = s.to_lowercase();
        for o in &HASH_ALGO_NAMES {
            if o.0 == inp.as_str() {
                result = o.1;
                break;
            }
        }
        result
    }

    pub fn names() -> Vec<&'static str> {
        let mut names = Vec::new();
        for i in 0..HASH_ALGO_NAMES.len() {
            names.push(HASH_ALGO_NAMES[i].0);
        }
        names
    }
}

/// Different kinds of error that may happen when trying to compare files.
///
/// # Variants:
///
/// - `AccessDenied`: reading metadata/file contents of `<Cause>` failed
/// - `InvalidFile`: the file `<Cause>` does not exist or is not a file
/// - `Metadata`: the metadata of file `<Cause>` could not be read or does not provide valid data
/// - `Other`: the file `<Cause>` caused an unspecified error with an error message (optional)
pub enum ComparisonErr {
    AccessDenied(Cause),
    InvalidFile(Cause),
    Metadata(Cause),
    Other(Cause, Option<String>)
}
impl ComparisonErr {
    pub fn other<T>(c: Cause) -> Result<T, ComparisonErr> {
        Err(ComparisonErr::Other(c, None))
    }
    pub fn other_msg<T>(c: Cause, msg: String) -> Result<T, ComparisonErr> {
        Err(ComparisonErr::Other(c, Some(msg)))
    }

    pub fn metadata<T>(c: Cause) -> Result<T, ComparisonErr> {
        Err(ComparisonErr::Metadata(c))
    }

    pub fn invalid_file<T>(c: Cause) -> Result<T, ComparisonErr> {
        Err(ComparisonErr::InvalidFile(c))
    }

    pub fn access_denied<T>(c: Cause) -> Result<T, ComparisonErr> {
        Err(ComparisonErr::AccessDenied(c))
    }
}

/// Helper to indicate which of two files caused an error of [ComparisonErr], assuming a context in
/// which exactly two files were specified.
///
/// # Examples:
/// ```
/// use std::path::Path;
/// use dcim_sort::sorting::comparison::ComparisonErr;
///
/// fn compare(source: &Path, target: &Path) -> Result<(), ComparisonErr> {
///     if !source.is_file() {
///         // indicate that argument `source` caused the Err
///         return Err(ComparisonErr::InvalidFile(Cause::Source));
///     }
///     else if !target.is_file() {
///         // indicate that argument `target` caused the Err
///         return Err(ComparisonErr::InvalidFile(Cause::Target));
///     }
///     if let Err(e) = hasher.init() {
///         return Err(ComparisonErr::Other(
///             Cause::NA,
///             Some(String::from("hasher failed to initialize!")
///         )));
///     }
///     Ok(())
/// }
/// ```
pub enum Cause {
    Source,
    Target,
    NA
}


pub struct FileComparer {
    ignore_zero_target: bool,
    hash_algo: HashAlgorithm
}

/// Type to wrap file comparison methods with different strategies (e.g. calculating a file hash).
/// Hashing can be performed if both files exist and have the same file size, or can be turned off
/// completely.
impl FileComparer {

    /// creates a default comparer that used SHA-256 for hashing
    pub fn default() -> FileComparer {
        Self::new(false, HashAlgorithm::SHA256)
    }

    /// create a new comparer
    pub fn new(ignore_zero_target: bool, hash_algo: HashAlgorithm) -> FileComparer {
        FileComparer{
            ignore_zero_target,
            hash_algo
        }
    }

    /// check if two files match.
    ///
    /// **NOTE:** returns always `false` if `hash_algo` is `None` and both file sizes are equal.
    ///
    /// returns `Ok(true)` if files match, `Ok(false)` if not and `Err` in case comparison failed.
    pub fn check_files_matching(&self, src: &Path, target: &Path) -> Result<bool, ComparisonErr> {
        // assure both are files
        if !src.is_file() {
            return Err(ComparisonErr::InvalidFile(Cause::Source));
        }
        if !target.is_file() {
            return Err(ComparisonErr::InvalidFile(Cause::Target));
        }

        // read metadata
        let meta_src = match Self::read_metadata(src) {
            Some(m) => m,
            None => return Err(ComparisonErr::Metadata(Cause::Source))
        };
        let meta_tgt = match Self::read_metadata(target) {
            Some(m) => m,
            None => return Err(ComparisonErr::Metadata(Cause::Target))
        };

        // check if file size matches
        if meta_src.len() != meta_tgt.len() {
            return Ok(false);
        }

        // file sizes match, calculate hashes
        let result= match self.hash_algo {
            HashAlgorithm::MD5 => Self::hash::<Md5>(src)? == Self::hash::<Md5>(target)?,
            HashAlgorithm::SHA256 => Self::hash::<Sha256>(src)? == Self::hash::<Sha256>(target)?,
            HashAlgorithm::None => false
        };

        Ok(result)
    }

    /// calculate a file hash with algorithm `T`
    pub fn hash<T: Digest>(path: &Path) -> Result<GenericArray<u8, T::OutputSize>, ComparisonErr> {
        if !path.is_file() {
            return Err(ComparisonErr::InvalidFile(Cause::NA));
        }

        let mut hasher = T::new();
        let mut buffer: [u8; 64] = [0; 64];
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => return ComparisonErr::other_msg(
                Cause::NA,
                format!("error opening file: {}", e)
            )
        };

        let mut reader = BufReader::new(file);
        loop {
            match reader.read(&mut buffer) {
                Ok(n) => {
                    if n > 0 {
                        hasher.update(&buffer[0..n]);
                    }
                    else {
                        break;
                    }
                },
                Err(e) => return ComparisonErr::other_msg(
                    Cause::NA,
                    format!("error while reading file: {}", e)
                )
            }
        }

        let result: GenericArray<u8, _> = hasher.finalize();
        Ok(result)
    }

    fn read_metadata(f: &Path) -> Option<Metadata> {
        assert!(f.is_file());

        match f.metadata() {
            Err(e) => {
                eprintln!("Error reading file metadata of {}: {}",
                          f.to_str().unwrap_or("<INVALID-UTF8>"),
                          e
                );
                None
            },
            Ok(meta) => {
                Some(meta)
            }
        }
    }
}