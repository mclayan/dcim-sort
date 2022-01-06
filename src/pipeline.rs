use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

use crate::media::ImgInfo;
use crate::media::metadata_processor::{MetaProcessor, MetaProcessorBuilder};
use crate::sorting::{Operation, SorterBuilder, Sorter, DuplicateResolution, ActionResult};
use crate::sorting::fs_support::{DirCreationRequest, DirManager};

pub struct Pipeline {
    processor: MetaProcessor,
    sorter: Sorter,
    sorting_operation: Operation,
    target_root: PathBuf,
    dup_handling: DuplicateResolution,
    report: Report
}

pub enum ControlMsg {
    Shutdown(mpsc::Sender<ControlMsg>),
    Ack,
    AckReport(Report)
}

pub enum Request<T> {
    Input(T),
    Cmd(ControlMsg)
}

#[derive(Copy, Clone)]
pub struct Report {
    pub count_success: u64,
    pub count_skipped: u64,
    pub count_duplicate: u64
}
impl Report {
    pub fn new() -> Report {
        Report{ count_success: 0, count_skipped: 0, count_duplicate: 0 }
    }

    pub fn add(&mut self, other: Report) {
        self.count_duplicate += other.count_duplicate;
        self.count_skipped += other.count_skipped;
        self.count_success += other.count_success;
    }
}
impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "  success  : {}\n  skipped  : {}\n  duplicate: {}\n", self.count_success, self.count_skipped, self.count_duplicate)
    }
}

impl Pipeline {

    pub fn new(processor: MetaProcessor, sorter: Sorter, sorting_operation: Operation, target_root: &Path, dup_handling: DuplicateResolution) -> Pipeline {
        Pipeline {
            processor,
            sorter,
            sorting_operation,
            target_root: target_root.to_path_buf(),
            dup_handling,
            report: Report::new()
        }
    }

    pub fn run(&mut self, rx: mpsc::Receiver<Request<ImgInfo>>) {
        let mut callback: Option<Sender<ControlMsg>> = None;
        for request in &rx {
            match request {
                Request::Input(req) => { self.process(req); }
                Request::Cmd(cmd) => {
                    match cmd {
                        ControlMsg::Shutdown(cb) => {
                            callback = Some(cb);
                            break;
                        }
                        ControlMsg::Ack | ControlMsg::AckReport(_) => {
                            println!("[WARN] unexpected ControlMsg: Ack");
                        }
                    }
                }
            }
        }

        while let Ok(req) = rx.try_recv() {
            match req {
                Request::Input(r) => self.process(r).unwrap(),
                Request::Cmd(_) => continue
            };
        }
        if let Some(cb) = callback {
            cb.send(ControlMsg::AckReport(self.report.clone()));
        }
    }

    pub fn process(&mut self, mut req: ImgInfo) -> Result<(), String> {
        // process metadata
        self.processor.process(&mut req);

        // translate into action
        let action = match &self.sorting_operation {
            Operation::Copy => self.sorter.calc_copy(&req, self.target_root.as_path()),
            Operation::Move => self.sorter.calc_move(&req, self.target_root.as_path()),
            Operation::Print => self.sorter.calc_simulation(&req, self.target_root.as_path())
        };
        if action.target_exists() {
            self.report.count_duplicate += 1;
        }
        // execute action with policy check
        match self.sorter.execute_checked(action, &self.dup_handling)? {
            ActionResult::Moved | ActionResult::Copied => { self.report.count_success += 1; }
            ActionResult::Skipped                      => { self.report.count_skipped += 1; }
        };
        Ok(())
    }
}

pub struct PipelineController {
    threads: Vec<(mpsc::Sender<Request<ImgInfo>>, JoinHandle<()>)>,
    current_thread: usize,
    dir_manager_handle: Option<JoinHandle<()>>,
    is_debug: bool
}

impl PipelineController {
    pub fn new(thread_count: usize, proc_cfg: MetaProcessorBuilder, mut sorter_cfg: SorterBuilder, sorting_operation: Operation, target_root: &Path, dup_handling: DuplicateResolution) -> PipelineController {
        let mut threads = Vec::with_capacity(thread_count);

        let (tx_dm, rx_dm) = mpsc::channel::<DirCreationRequest>();
        let dm_handle = thread::Builder::new()
            .name(String::from("dirmgr01"))
            .spawn(move || {
                let mut dm = match &sorting_operation {
                    Operation::Print => DirManager::new_simulating(),
                    _               => DirManager::new()
                };
                dm.run(rx_dm);
            }).unwrap();

        for i in 0..thread_count {
            let (tx, rx) = mpsc::channel::<Request<ImgInfo>>();
            let processor = proc_cfg.build_clone();
            let sorter = sorter_cfg.build_async(tx_dm.clone());
            let mut pipeline = Pipeline::new(processor, sorter, sorting_operation.clone(), target_root, dup_handling);
            let t = thread::Builder::new()
                .name(format!("pipeline{:03}", i))
                .spawn(move || {
                    pipeline.run(rx);
                }).unwrap();
            threads.push((tx, t));
        }

        //drop tx_dm so if sorters are dropped the DM thread exits the rec loop
        drop(tx_dm);

        PipelineController{
            threads: threads,
            current_thread: 0,
            dir_manager_handle: Some(dm_handle),
            is_debug: false
        }
    }

    pub fn debug(&mut self) {
        self.is_debug = true;
    }

    pub fn process(&mut self, request: ImgInfo) {
        assert!(self.current_thread < self.threads.len());
        let (tx, _) = self.threads.get(self.current_thread).unwrap();
        match tx.send(Request::Input(request)) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("[PipelineControl] error sending request to pipeline[{}]: {}", self.current_thread, e);
                panic!();
            }
        };

        if self.current_thread >= self.threads.len() - 1 {
            self.current_thread = 0;
        }
        else {
            self.current_thread += 1;
        }
    }

    pub fn shutdown(mut self) -> Report {
        let mut p = 0;
        let mut report = Report::new();
        for (tx, handle) in self.threads {
            let (cb_tx, cb_rx) = mpsc::channel::<ControlMsg>();
            // send shutdown cmd to allow processing pending requests
            tx.send(Request::Cmd(ControlMsg::Shutdown(cb_tx)));

            // try 5 times to receive ACK
            for i in 0..5 {
                // maybe a timeout rec should be done here
                match cb_rx.recv() {
                    Ok(response) => match response {
                        ControlMsg::Ack => break,
                        ControlMsg::AckReport(rep) => {
                            if self.is_debug {
                                println!("=== pipeline[{:02}]===\n{}", p, &rep);
                            }
                            report.add(rep);
                            break;
                        },
                        _ => ()
                    },
                    Err(e) => eprintln!("Failed to receive callback for pipeline[{}]: {}", p, e)
                }
            }
            handle.join();
            p += 1;
        }
        self.dir_manager_handle.take().expect("[PipelineController] failed to join DirManager: is None").join();
        report
    }
}