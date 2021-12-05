use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc;

enum Mode {
    Exec,
    Collect
}

pub struct DirCreationRequest {
    target: PathBuf,
    callback: mpsc::Sender<bool>
}
impl DirCreationRequest {
    pub fn new(path: &PathBuf, callback: mpsc::Sender<bool>) -> DirCreationRequest {
        DirCreationRequest {
            target: path.clone(),
            callback: callback
        }
    }
}

struct CachedPath {
    hash: u64,
    path: PathBuf
}


pub struct DirManager {
    mode: Mode,
    cache: Vec<u64>,
}

impl DirManager {

    pub fn new() -> DirManager {
        DirManager {
            mode: Mode::Exec,
            cache: Vec::new(),
        }
    }

    pub fn new_simulating() -> DirManager {
        DirManager {
            mode: Mode::Collect,
            cache: Vec::new(),
        }
    }

    pub fn run(&mut self, rx_input: mpsc::Receiver<DirCreationRequest>) {
        for request in rx_input {
            let tgt = request.target;
            let hash = Self::hash_path(&tgt);
            let mut is_cached = false;
            for pp in &self.cache {
                if *pp == hash {
                    is_cached = true;
                    request.callback.send(true).unwrap();
                    break;
                }
            }
            if !is_cached {
                match self.mode {
                    Mode::Exec => {
                        match std::fs::create_dir_all(tgt) {
                            Err(e) => {
                                println!("Failed to create destination directory: {}", e);
                                request.callback.send(false).unwrap();
                            },
                            Ok(_) => {
                                self.cache.push(hash);
                                request.callback.send(true).unwrap();
                            }
                        };
                    }
                    Mode::Collect => {
                        self.cache.push(hash);
                    }
                };
            }
        }
    }

    fn hash_path(path: &PathBuf) -> u64 {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }
}