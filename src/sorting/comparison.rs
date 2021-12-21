use std::fs::{File, Metadata};
use std::io::{BufReader, Read};
use std::path::{Path};

use md5::{Digest, Md5};
use md5::digest::generic_array::{GenericArray};
use sha2::Sha256;

pub enum HashAlgorithm {
    MD5,
    SHA256,
}
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
pub enum Cause {
    Source,
    Target,
    NA
}
pub struct FileComparer {
    ignore_zero_target: bool,
    hash_algo: HashAlgorithm
}

impl FileComparer {

    pub fn default() -> FileComparer {
        Self::new(false, HashAlgorithm::SHA256)
    }

    pub fn new(ignore_zero_target: bool, hash_algo: HashAlgorithm) -> FileComparer {
        FileComparer{
            ignore_zero_target,
            hash_algo
        }
    }

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
            HashAlgorithm::SHA256 => Self::hash::<Sha256>(src)? == Self::hash::<Sha256>(target)?
        };

        Ok(result)
    }

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