mod image;
mod index;

use clap::{App, Arg};
use std::path::PathBuf;
use crate::index::Scanner;

struct MArgs {
    files: Vec<String>,
    max_recursion: u8,
    debug: bool
}

fn main() {
    let args = parse_args();
    if args.debug {
        println!("files: {}\nmax-recursion: {}\n", args.files.len(), args.max_recursion);
    }

    for file in args.files {
        println!("Processing file: {}", &file);
        let mut scanner = Scanner::new(file).unwrap();
        scanner.debug(args.debug);
        let children = scanner.scan();
        if args.debug {
            for child in children {
                println!("===[ {} ]===\n{:#?}", child.path().to_str().unwrap_or("INVALID_UTF8"), child);
            }
        }
    }
}

fn parse_args() -> MArgs {

    let matches = App::new("dcim-sort - sort images from DCIM folders")
        .version("0.1.0")
        .author("MCL")
        .about("Sort images from (unintuitive) DCIM file structures")
        .arg(Arg::new("FILE")
            .multiple(true)
            .about("Input file to process")
            .required(true))
        .arg(Arg::new("max_recursion")
            .multiple(false)
            .short('n')
            .long("max-recursion")
            .about("maximum recursion level while scanning")
            .takes_value(true)
            .default_value("10")
            .required(false))
        .arg(Arg::new("debug")
            .required(false)
            .long("debug")
            .short('d')
            .about("show debug messages")
            .takes_value(false))
        .get_matches();

    let inp_files = matches.values_of("FILE").unwrap();
    let mut files : Vec<String> = Vec::with_capacity(inp_files.len());
    for f in inp_files {
        files.push(String::from(f));
    }

    let max_recursion : u8 = matches.value_of_t_or_exit("max_recursion");
    let debug = matches.is_present("debug");

    MArgs {
        files,
        max_recursion,
        debug
    }
}