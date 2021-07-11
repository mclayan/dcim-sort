extern crate exif;

use std::fs::File;
use std::io::BufReader;
use clap::{App, Arg};
use std::path::Path;
use std::time::SystemTime;



fn main() {
    let matches = App::new("pexif - Print EXIF data")
        .version("0.1.0")
        .about("MCL")
        .about("Print Exchangeable image file format (Exif) data from image files")
        .arg(Arg::new("FILE")
                 .about("Input file to process")
                 .required(true)
                 .index(1))
        .arg(Arg::new("all_tags")
            .short('a')
            .long("all-tags")
            .takes_value(false)
            .about("print all tags"))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .takes_value(false)
            .about("verbose output")
            .required(false))
        .arg(Arg::new("hex_ids")
            .short('x')
            .long("hex-ids")
            .about("print Exif IDs in hex format")
            .required(false)
            .takes_value(false))
        .arg(Arg::new("tag_names")
            .short('n')
            .long("tag-names")
            .required(false)
            .about("print Exif tag names")
            .takes_value(false))
        .arg(Arg::new("tag")
            .short('t')
            .long("tag")
            .about("tag name to include in output")
            .multiple_occurrences(true)
            .takes_value(true)
            .required(false))
        .get_matches();

    let inp_file = match matches.value_of("FILE") {
        Some(v) => v,
        None => "null"
    };
    let all_tags = matches.is_present("all_tags");
    let print_hex_ids = matches.is_present("hex_ids");
    let print_tag_names = matches.is_present("tag_names");

    if matches.is_present("verbose") {
        println!("input file: {}", inp_file);
        println!("all tags:   {}", all_tags);
    }

    if ! std::path::Path::new(inp_file).exists() {
        eprintln!("File does not exist: {}", inp_file);
        return;
    }

    let tags = get_tags(inp_file);
    print_table_unfiltered(&tags, print_hex_ids, print_tag_names);
}

fn get_tags(file: &str) -> Vec<exif::Field> {
    let file = File::open(file).expect(format!("Failed to open file: {}", file).as_str());
    let mut bufreader = BufReader::new(file);
    let exifreader = exif::Reader::new();
    let exif = exifreader.read_from_container(&mut bufreader).expect("Failed to read exif!");

    let mut tags : Vec<exif::Field> = Vec::new();
    for f in exif.fields() {
        tags.push(f.clone())
    }
    tags
}

fn print_table_unfiltered(tags: &Vec<exif::Field>, hex_ids: bool, names: bool) {
    let mut len_tname : usize = 0;
    if names {
        for t in tags {
            let ln = t.tag.to_string().len();
            if ln > len_tname {
                len_tname = ln;
            }
        }
    }

    let mut header = String::from("| VALUE");
    if (names) {
        header = format!("| {0:^1$} {2}", "NAME", len_tname, header);
    }
    if (hex_ids) {
        header = format!("|  ID   {}", header);
    }
    println!("\t{}\n\t-------------------------------------", header);
    for t in tags {
        let mut ln = format!("| {}", t.display_value());
        if names {
            ln = format!("| {0:<1$} {2}", t.tag, len_tname, ln);
        }
        if hex_ids {
            ln = format!("| {:#06x} {}", t.tag.number(), ln);
        }
        println!("\t{}", ln);
    }
}
