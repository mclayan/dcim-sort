use std::path::{Path, PathBuf};
use std::{fs, io, env};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::time::Duration;
use chrono;
use chrono::{Datelike, SecondsFormat};
use crate::pipeline::ControlMsg;

pub enum LogReq {
    Msg(LogMsg),
    Cmd(ControlMsg)
}
pub struct LogMsg {
    sender: String,
    msg: String
}

impl LogMsg {
    pub fn new(sender_id: String, msg: String) -> LogMsg {
        LogMsg {
            sender: sender_id,
            msg
        }
    }
}

pub struct Logger {
    outfile: PathBuf,
    file_handle: Option<File>,
    print_sender: bool
}
impl Logger {
    pub fn new(outdir: &PathBuf, filename: Option<String>) -> io::Result<Logger> {
        let fname = match filename {
            None => Self::generate_filename(),
            Some(s) => {
                if s.is_empty() {
                    Self::generate_filename()
                }
                else if !s.ends_with(".log") {
                    format!("{}.log", s)
                }
                else {
                    s
                }
            }
        };
        if outdir.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "outdir is a file"));
        }
        if !outdir.exists() {
            fs::create_dir_all(outdir)?;
        }
        let mut outfile = outdir.clone();
        outfile.push(fname);
        println!("writing logfile to: {}", outfile.to_str().unwrap_or("<INVALID UTF-8>"));

        Ok(Logger {
            outfile: outfile,
            file_handle: None,
            print_sender: true
        })
    }

    fn generate_filename() -> String {
        let now = chrono::Local::now();
        format!("dcim-sort_{}-{}-{}.log", now.year(), now.month(), now.day())
    }

    pub fn run(&mut self, rx_input: mpsc::Receiver<LogReq>) {
        // failing to open the file for writing should not crash the program
        let mut buff = match OpenOptions::new().create(true).append(true).open(&self.outfile) {
            Ok(file) => {
                Some(BufWriter::new(file))
            }
            Err(e) => {
                eprintln!("[WARN] failed to open log file: {}",
                          &self.outfile.to_str().unwrap_or("<INVALID UTF-8>")
                );
                None
            }
        };

        if let Some(b) = &mut buff {
            write!(b, "==============[ start log ]==============\n[{}] log started\n",
                   chrono::Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
            );
        }
        let mut callback: Option<Sender<ControlMsg>> = None;
        let mut shutdown = false;
        let mut has_data = false;
        loop {
            while let Ok(request) = rx_input.recv_timeout(Duration::from_millis(500)) {
                match request {
                    LogReq::Msg(msg) => match &mut buff {
                        Some(b) => {
                            self.write_msg(b, msg);
                            has_data = true;
                        },
                        None => {
                            self.print_msg(msg);
                        }
                    },
                    LogReq::Cmd(msg) => match msg {
                        ControlMsg::Shutdown(cb) => {
                            callback = Some(cb);
                            shutdown = true;
                        },
                        _ => eprintln!("[WARN]-[LOG] received unexpected ACK message!")
                    }
                };
            }
            if has_data {
                if let Some(b) = &mut buff {
                    b.flush();
                    has_data = false;
                }
            }
            if shutdown {
                break;
            }
        }

        while let Ok(request) = rx_input.try_recv() {
            match request {
                LogReq::Msg(msg) => match &mut buff {
                    Some(b) => {
                        self.write_msg(b, msg);
                    },
                    None => self.print_msg(msg)
                },
                _ => ()
            };
        }

        if let Some(cb) = callback {
            cb.send(ControlMsg::Ack);
        }

        if let Some(mut b) = buff {
            write!(b, "[{}] closing log\n", chrono::Local::now().to_rfc3339_opts(SecondsFormat::Millis, false));
            b.flush();
        }
    }

    fn write_msg(&self, buf: &mut BufWriter<File>, msg: LogMsg) {
        if self.print_sender {
            write!(buf, "[{}] {}\n", msg.sender, msg.msg);
        }
        else {
            write!(buf, "{}\n", msg.msg);
        }
    }

    fn print_msg(&self, msg: LogMsg) {
        if self.print_sender {
            println!("[INFO][{}] {}", msg.sender, msg.msg);
        }
        else {
            println!("[INFO] {}", msg.msg);
        }
    }
}