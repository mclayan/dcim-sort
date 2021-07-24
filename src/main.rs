mod index;
mod sorting;
mod pattern;
mod media;

use clap::{App, Arg};
use std::path::PathBuf;
use crate::index::Scanner;
use crate::sorting::{Sorter, Strategy};
use crate::pattern::device::{MakeModelPattern, DevicePart, CaseNormalization};
use crate::pattern::general::{ScreenshotPattern, DateTimePattern, DateTimePart};
use crate::media::metadata_processor::MetaProcessor;
use crate::media::rexiv_proc::Rexiv2Processor;
use crate::media::FileMetaProcessor;

struct MArgs {
    files: Vec<String>,
    target_root: String,
    max_recursion: u8,
    debug: u64,
}

fn main() {
    let args = parse_args();
    if args.debug > 1 {
        println!("files: {}\nmax-recursion: {}\n", args.files.len(), args.max_recursion);
    }

    let outdir = PathBuf::from(&args.target_root);

    let mut sorter = Sorter::new(outdir)
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
        .build();

    let meta_processor = MetaProcessor::new()
        .processor(Rexiv2Processor::new())
        .build();

    for file in args.files {
        println!("Processing file: {}", &file);
        let mut scanner = Scanner::new(file).unwrap();
        scanner.debug(args.debug > 1);

        let mut children = scanner.scan();

        children = meta_processor.process_all(children);

        sorter.sort_all(&children, Strategy::Copy);
        /*
        for child in children {
            let new_path = sorter.translate(&child);
            if args.debug > 0 {
                let path_old = child.path().to_str().unwrap_or("INVALID_UTF8");
                let path_new = new_path.to_str().unwrap_or("INVALID_UTF8");
                println!("===[ {} ]===\ntarget={}", path_old, path_new);
                if args.debug > 1 {
                    println!("{:#?}", child);
                }
                println!();
            }

        }
         */

    }
}

fn parse_args() -> MArgs {
    let name_outdir = "output-dir";
    let name_infile = "FILE";
    let name_max_recursion = "max-recursion";
    let name_debug = "debug";

    let matches = App::new("dcim-sort - sort images from DCIM folders")
        .version("0.1.0")
        .author("MCL")
        .about("Sort images from (unintuitive) DCIM file structures")
        .arg(Arg::new(name_infile)
            .multiple(true)
            .about("Input file to process")
            .required(true))
        .arg(Arg::new(name_outdir)
            .required(false)
            .short('o')
            .long("output")
            .default_value("sorted")
            .about("Output directory"))
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
        .get_matches();

    let inp_files = matches.values_of(name_infile).unwrap();
    let output_dir = matches.value_of(name_outdir).unwrap();
    let mut files: Vec<String> = Vec::with_capacity(inp_files.len());
    for f in inp_files {
        files.push(String::from(f));
    }

    let max_recursion: u8 = matches.value_of_t_or_exit(name_max_recursion);
    let debug = matches.occurrences_of(name_debug);

    MArgs {
        files,
        target_root: String::from(output_dir),
        max_recursion,
        debug,
    }
}