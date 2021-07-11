mod image;

use clap::{App, Arg};

struct MArgs {
    files: Vec<String>,
}

fn main() {
    let args = parse_args();

    for file in args.files {
        println!("Processing file: {}", &file);
        let info = image::ImgInfo::new(file).expect("Failed to process file!");
        println!("\n===[ {} ]===", info.path());
        println!("{:#?}\n", info);
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
        .get_matches();


    let inp_files = matches.values_of("FILE").unwrap();
    let mut files : Vec<String> = Vec::with_capacity(inp_files.len());
    for f in inp_files {
        files.push(String::from(f));
    }

    MArgs {
        files
    }
}