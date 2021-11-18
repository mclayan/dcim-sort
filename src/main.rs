mod index;
mod sorting;
mod pattern;
mod media;
mod config;
mod pipeline;

use clap::{App, Arg};
use std::path::{PathBuf, Path};
use crate::index::Scanner;
use crate::sorting::{Sorter, SorterBuilder, Strategy};
use crate::pattern::device::{MakeModelPattern, DevicePart, CaseNormalization};
use crate::pattern::general::{ScreenshotPattern, DateTimePattern, DateTimePart};
use crate::pattern::fallback::{SimpleFileTypePattern};
use crate::media::metadata_processor::{MetaProcessor, MetaProcessorBuilder, Priority};
use crate::media::rexiv_proc::Rexiv2Processor;
use crate::media::kadamak_exif::KadamakExifProcessor;
use crate::config::{RootCfg};
use std::fs::File;
use std::sync::atomic::Ordering::AcqRel;
use std::ops::Index;
use std::any::{type_name, Any};
use std::time;
use chrono::Duration;
use crate::pattern::PatternElement;
use crate::pipeline::{Pipeline, PipelineController};


enum Operation {
    Move,
    Copy,
    Simulate
}

struct MArgs {
    file: String,
    target_root: String,
    max_recursion: u8,
    debug: u64,
    ignore_unknown_types: bool,
    dry_run: bool,
    config_path: Option<PathBuf>,
    operation: Operation,
    thread_count: usize
}

fn main() {
    let args = parse_args();
    if args.debug > 1 {
        println!("files: {}\nmax-recursion: {}\n", 1, args.max_recursion);
    }

    let outdir = PathBuf::from(&args.target_root);

    let mut sorter = match &args.config_path {
        Some(cfg) => {
            let root_cfg = read_config(cfg.as_path());
            root_cfg.generate_sorter_builder(outdir).expect("Failed to read configuration!")
        }
        None => generate_default_sorter(outdir)
    };

    let meta_processor = MetaProcessor::new()
        .processor(Rexiv2Processor::new(), Priority::None)
        .processor(KadamakExifProcessor::new(), Priority::Lowest);


    if !args.dry_run {
        process_files(args, sorter, meta_processor);
    }
}

fn print_config(sorter: &Sorter, args: &MArgs) {
    let seg_count = sorter.get_seg_count();
    println!("=======[ Sorter Configuration ]=========");
    print!("seg_count_supported: {}\nseg_count_fallback: {}\n", seg_count.0, seg_count.1);
    print!("segments:\n");
    let mut i: usize = 0;
    for seg in sorter.get_segments_supported() {
        println!("    [{:02}] {:>22}: {}", i, seg.name(), seg.display());
        i += 1;
    }
    println!();
}

fn process_files(args: MArgs, sorter: SorterBuilder, meta_processor: MetaProcessorBuilder) {
    println!("[INFO] Processing file: {}", &args.file);
    let mut scanner = Scanner::new(args.file.clone()).unwrap();
    scanner.debug(args.debug > 1);
    scanner.ignore_unknown_types(args.ignore_unknown_types);

    let strategy = match args.operation {
        Operation::Move => Strategy::Move,
        Operation::Copy => Strategy::Copy,
        Operation::Simulate => Strategy::Print
    };
    let mut pipeline = PipelineController::new(
        args.thread_count,
        meta_processor,
        sorter,
        strategy
    );

    if args.debug > 0 {
        pipeline.debug();
    }

    let time_start = time::Instant::now();
    scanner.scan_pipeline(&mut pipeline);
    println!("[main] finished scanning, joining threads.");
    let report = pipeline.shutdown();

    let elapsed = chrono::Duration::from_std(time_start.elapsed()).unwrap();
    print!("=== summary ======\n{}", report);
    println!("took {:.4} seconds or {:03}:{:02}:{:02}", elapsed.num_milliseconds() as f64 / 1000.0,
        elapsed.num_hours(),
        elapsed.num_minutes() % 60,
        elapsed.num_seconds() % 3600
    );
}

fn parse_args() -> MArgs {
    let name_outdir = "output-dir";
    let name_threads = "max-threads";
    let name_infile = "FILE";
    let name_max_recursion = "max-recursion";
    let name_debug = "debug";
    let name_ignore_ftype = "ignore-other-types";
    let name_cfg_path = "config";
    let name_simulate = "dry-run";
    let name_operation = "OPERATION";


    let matches = App::new("dcim-sort - sort images from DCIM folders")
        .version("0.1.0")
        .author("MCL")
        .about("Sort images from (unintuitive) DCIM file structures")
        .arg(Arg::new(name_outdir)
            .required(false)
            .short('o')
            .long("output")
            .default_value("sorted")
            .about("Output directory"))
        .arg(Arg::new(name_threads)
            .required(false)
            .short('p')
            .long("max-threads")
            .default_value("4")
            .about("Max Thread Count"))
        .arg(Arg::new(name_max_recursion)
            .multiple(false)
            .short('n')
            .long("max-recursion")
            .about("maximum recursion level while scanning")
            .takes_value(true)
            .default_value("10")
            .required(false))
        .arg(Arg::new(name_debug)
            .required(false)
            .multiple(true)
            .long("debug")
            .short('d')
            .about("show debug messages")
            .takes_value(false))
        .arg(Arg::new(name_ignore_ftype)
            .about("ignore unknown file types (based on file ending)")
            .short('i')
            .long("ignore-unknown")
            .required(false))
        .arg(Arg::new(name_cfg_path)
            .about("configuration file input")
            .short('f')
            .long("config")
            .required(false)
            .takes_value(true))
        .arg(Arg::new(name_simulate)
            .about("configure and exit without processing")
            .short('t')
            .long("dry-run")
            .required(false)
            .takes_value(false))
        .arg(Arg::new(name_infile)
            .multiple(false)
            .about("input file to process. In case of a folder, all children are processed recursively.")
            .required(true))
        .subcommand(App::new("simulate")
            .about("only simulate processing with generated targets printed to STDOUT"))
        .subcommand(App::new("move")
            .about("move files"))
        .subcommand(App::new("copy")
            .about("copy files instead of moving"))
        .subcommand_placeholder("OPERATION", "OPERATIONS")
        .get_matches();

    let file = matches.value_of(name_infile).unwrap();
    let output_dir = matches.value_of(name_outdir).unwrap();


    let max_recursion: u8 = matches.value_of_t_or_exit(name_max_recursion);
    let max_threads: usize = matches.value_of_t_or_exit(name_threads);
    let debug = matches.occurrences_of(name_debug);
    let ignore_unknown = matches.is_present(name_ignore_ftype);
    let dry_run = matches.is_present(name_simulate);

    let cfg_path = match matches.is_present(name_cfg_path) {
        true => {
            let s = matches.value_of(name_cfg_path).unwrap();
            let p = PathBuf::from(s);
            if !p.is_file() {
                panic!("[ERROR] file does not exist: {}", s);
            }
            Some(p)
        }
        false => None
    };

    let operation = match matches.subcommand_name().expect("Missing operation!") {
        "simulate" => Operation::Simulate,
        "move" => Operation::Move,
        "copy" => Operation::Copy,
        o => panic!("Invalid operation: {}", o)
    };


    MArgs {
        file: String::from(file),
        target_root: String::from(output_dir),
        max_recursion,
        debug,
        ignore_unknown_types: ignore_unknown,
        dry_run,
        config_path: cfg_path,
        operation,
        thread_count: max_threads
    }
}

pub fn read_config(path: &Path) -> RootCfg {

    if !(path.exists() && path.is_file()) {
        panic!("[ERROR] could not open configuration file: file does not exist: \"{}\"",
               path.to_str().unwrap_or("<NON-PRINTABLE>")
        );
    }

    let mut file = File::open(path).expect("[ERROR] could not open configuration file");
    RootCfg::read_file(&mut file).unwrap()
}

pub fn generate_default_sorter(outdir: PathBuf) -> SorterBuilder {
    Sorter::new(outdir)
        .segment(MakeModelPattern::new()
            .part(DevicePart::Make)
            .part(DevicePart::Model)
            .separator('_')
            .replace_spaces(true)
            .case_normalization(CaseNormalization::Lowercase)
            .fallback(String::from("unknown_device"))
            .build())
        .segment(ScreenshotPattern::new(String::from("screenshots")))
        .segment(DateTimePattern::new()
            .part(DateTimePart::Year)
            .part(DateTimePart::Month)
            .build())
        .fallback(SimpleFileTypePattern::new().build())
}