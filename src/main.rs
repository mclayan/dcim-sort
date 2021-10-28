mod index;
mod sorting;
mod pattern;
mod media;
mod config;

use clap::{App, Arg};
use std::path::{PathBuf, Path};
use crate::index::Scanner;
use crate::sorting::{Sorter, Strategy};
use crate::pattern::device::{MakeModelPattern, DevicePart, CaseNormalization};
use crate::pattern::general::{ScreenshotPattern, DateTimePattern, DateTimePart};
use crate::pattern::fallback::{SimpleFileTypePattern};
use crate::media::metadata_processor::{MetaProcessor, Priority};
use crate::media::rexiv_proc::Rexiv2Processor;
use crate::media::kadamak_exif::KadamakExifProcessor;
use crate::config::{RootCfg};
use std::fs::File;
use std::sync::atomic::Ordering::AcqRel;
use std::ops::Index;
use std::any::{type_name, Any};
use crate::pattern::PatternElement;


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
    operation: Operation
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
            root_cfg.generate_sorter(outdir).expect("Failed to read configuration!")
        }
        None => generate_default_sorter(outdir)
    };

    let meta_processor = MetaProcessor::new()
        .processor(Rexiv2Processor::new(), Priority::None)
        .processor(KadamakExifProcessor::new(), Priority::Lowest)
        .build();

    if args.debug > 0 {
        print_config(&sorter, &args);
    }

    if !args.dry_run {
        process_files(args, &mut sorter, meta_processor);
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

fn process_files(args: MArgs, sorter: &mut Sorter, meta_processor: MetaProcessor) {

    println!("[INFO] Processing file: {}", &args.file);
    let mut scanner = Scanner::new(args.file.clone()).unwrap();
    scanner.debug(args.debug > 1);
    scanner.ignore_unknown_types(args.ignore_unknown_types);

    let mut children = scanner.scan();

    children = meta_processor.process_all(children);

    match args.operation {
        Operation::Move =>      { sorter.sort_all(&children, Strategy::Move);  }
        Operation::Copy =>      { sorter.sort_all(&children, Strategy::Copy);  }
        Operation::Simulate =>  { sorter.sort_all(&children, Strategy::Print); }
    }


}

fn parse_args() -> MArgs {
    let name_outdir = "output-dir";
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
        .arg(Arg::new(name_max_recursion)
            .multiple_occurrences(false)
            .short('n')
            .long("max-recursion")
            .about("maximum recursion level while scanning")
            .takes_value(true)
            .default_value("10")
            .required(false))
        .arg(Arg::new(name_debug)
            .required(false)
            .multiple_occurrences(true)
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
            .multiple_occurrences(false)
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
        operation
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

pub fn generate_default_sorter(outdir: PathBuf) -> Sorter {
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
        .build()
}