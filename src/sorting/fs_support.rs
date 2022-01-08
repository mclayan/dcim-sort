use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use crate::sorting::PATHSTR_FB;

pub struct DirCreationRequest {
    target: PathBuf,
    callback: mpsc::Sender<bool>,
    cache_only: bool
}
impl DirCreationRequest {
    pub fn new(path: &Path, callback: mpsc::Sender<bool>) -> DirCreationRequest {
        DirCreationRequest {
            target: path.to_path_buf(),
            callback: callback,
            cache_only: false
        }
    }

    pub fn new_simulating(path: &Path, callback: mpsc::Sender<bool>) -> DirCreationRequest {
        DirCreationRequest {
            target: path.to_path_buf(),
            callback: callback,
            cache_only: true
        }
    }
}

struct CachedPath {
    hash: u64,
    path: PathBuf
}


pub struct DirManager {
    cache: Vec<u64>,
}

impl DirManager {

    pub fn new() -> DirManager {
        DirManager {
            cache: Vec::new(),
        }
    }

    pub fn run(&mut self, rx_input: mpsc::Receiver<DirCreationRequest>) {
        for request in rx_input {
            let tgt = request.target;
            match self.create_path(tgt.as_path(), request.cache_only) {
                Ok(_) => request.callback.send(true).unwrap(),
                Err(e) => {
                    eprintln!("[{}] failed to create path=\"{}\": {}",
                        std::thread::current().name().unwrap_or("logmgr"),
                        tgt.to_str().unwrap_or(PATHSTR_FB),
                        e
                    );
                    request.callback.send(false).unwrap();
                }
            }
        }
    }

    pub fn create_path(&mut self, path: &Path, cache_only: bool) -> Result<(), String> {
        let hash = Self::hash_path(path);
        let mut is_cached = false;
        for pp in &self.cache {
            if *pp == hash {
                return Ok(());
            }
        }
        match cache_only {
            false => match std::fs::create_dir_all(path) {
                Err(e) => Err(format!("Failed to create destination directory: {}", e)),
                Ok(_) => {
                    self.cache.push(hash);
                    Ok(())
                }
            },
            true => {
                self.cache.push(hash);
                Ok(())
            }
        }
    }

    fn hash_path(path: &Path) -> u64 {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }
}