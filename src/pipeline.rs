use std::fmt::{Display, Formatter};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

use crate::media::ImgInfo;
use crate::media::metadata_processor::{MetaProcessor, MetaProcessorBuilder};
use crate::sorting::{fs_support::DirManager, Sorter, SorterBuilder, Strategy};
use crate::sorting::fs_support::DirCreationRequest;

pub struct Pipeline {
    processor: MetaProcessor,
    sorter: Sorter,
    sorting_strategy: Strategy,
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

    pub fn new(processor: MetaProcessor, sorter: Sorter, sorting_strategy: Strategy) -> Pipeline {
        Pipeline {
            processor,
            sorter,
            sorting_strategy
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
                Request::Input(r) => self.process(r),
                Request::Cmd(_) => continue
            };
        }
        if let Some(cb) = callback {
            let report = self.sorter.get_report();
            cb.send(ControlMsg::AckReport(report));
        }
    }

    pub fn process(&mut self, mut req: ImgInfo) {
        // process metadata
        self.processor.process(&mut req);

        // sort file
        self.sorter.sort(&req, self.sorting_strategy);
    }
}

pub struct PipelineController {
    threads: Vec<(mpsc::Sender<Request<ImgInfo>>, JoinHandle<()>)>,
    current_thread: usize,
    dir_manager_handle: Option<JoinHandle<()>>,
    is_debug: bool
}

impl PipelineController {
    pub fn new(thread_count: usize, proc_cfg: MetaProcessorBuilder, mut sorter_cfg: SorterBuilder, sorting_strategy: Strategy) -> PipelineController {
        let mut threads = Vec::with_capacity(thread_count);

        let (tx_dm, rx_dm) = mpsc::channel::<DirCreationRequest>();
        let dm_handle = thread::spawn(move || {
            let mut dm = match &sorting_strategy {
                Strategy::Print => DirManager::new_simulating(),
                _               => DirManager::new()
            };
            dm.run(rx_dm);
        });

        for i in 0..thread_count {
            let (tx, rx) = mpsc::channel::<Request<ImgInfo>>();
            let processor = proc_cfg.build_clone();
            let sorter = sorter_cfg.build_clone(tx_dm.clone());
            let mut pipeline = Pipeline::new(processor, sorter, sorting_strategy.clone());
            let t = thread::spawn(move || {
                pipeline.run(rx);
            });
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