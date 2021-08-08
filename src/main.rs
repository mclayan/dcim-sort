mod index;
mod sorting;
mod pattern;
mod media;
mod config;

use clap::{App, Arg};
use std::path::PathBuf;
use crate::index::Scanner;
use crate::sorting::{Sorter, Strategy};
use crate::pattern::device::{MakeModelPattern, DevicePart, CaseNormalization};
use crate::pattern::general::{ScreenshotPattern, DateTimePattern, DateTimePart};
use crate::pattern::fallback::{SimpleFileTypePattern};
use crate::media::metadata_processor::{MetaProcessor, Priority};
use crate::media::rexiv_proc::Rexiv2Processor;
use crate::media::kadamak_exif::KadamakExifProcessor;

struct MArgs {
    files: Vec<String>,
    target_root: String,
    max_recursion: u8,
    debug: u64,
    ignore_unknown_types: bool
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
        .fallback(SimpleFileTypePattern::new().build())
        .build();

    let meta_processor = MetaProcessor::new()
        .processor(Rexiv2Processor::new(), Priority::None)
        .processor(KadamakExifProcessor::new(), Priority::Lowest)
        .build();

    for file in args.files {
        println!("Processing file: {}", &file);
        let mut scanner = Scanner::new(file).unwrap();
        scanner.debug(args.debug > 1);
        scanner.ignore_unknown_types(args.ignore_unknown_types);

        let mut children = scanner.scan();

        children = meta_processor.process_all(children);

        sorter.sort_all(&children, Strategy::Copy);

    }
}

fn parse_args() -> MArgs {
    let name_outdir = "output-dir";
    let name_infile = "FILE";
    let name_max_recursion = "max-recursion";
    let name_debug = "debug";
    let name_ignore_ftype = "ignore-other-types";

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
        .arg(Arg::new(name_ignore_ftype)
            .about("ignore unknown file types (based on file ending)")
            .short('i')
            .long("ignore-unknown")
            .required(false))
        .get_matches();

    let inp_files = matches.values_of(name_infile).unwrap();
    let output_dir = matches.value_of(name_outdir).unwrap();
    let mut files: Vec<String> = Vec::with_capacity(inp_files.len());
    for f in inp_files {
        files.push(String::from(f));
    }

    let max_recursion: u8 = matches.value_of_t_or_exit(name_max_recursion);
    let debug = matches.occurrences_of(name_debug);
    let ignore_unknown = matches.is_present(name_ignore_ftype);

    MArgs {
        files,
        target_root: String::from(output_dir),
        max_recursion,
        debug,
        ignore_unknown_types: ignore_unknown
    }
}