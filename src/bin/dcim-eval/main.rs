use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{App, Arg};
use dcim_sort::config::RootCfg;
use dcim_sort::index::Scanner;
use dcim_sort::media::{FileType, ImgInfo, ImgMeta};
use dcim_sort::media::kadamak_exif::KadamakExifProcessor;
use dcim_sort::media::metadata_processor::{MetaProcessor, Priority};
use dcim_sort::media::rexiv_proc::Rexiv2Processor;
use dcim_sort::pattern::device::{CaseNormalization, DevicePart, MakeModelPattern};
use dcim_sort::pattern::fallback::DummyPattern;
use dcim_sort::pattern::general::{DateTimePart, DateTimePattern, ScreenshotPattern};
use dcim_sort::sorting::{PATHSTR_FB, Sorter, SorterBuilder};
use dcim_sort::sorting::translation::Translator;

struct MainArgs {
    files: Vec<PathBuf>,
    cfg_file: Option<PathBuf>,
    out_dir: PathBuf,
    print_sorting: bool,
    print_meta: bool
}

fn parse_args() -> Result<MainArgs, String> {
    let matches = App::new("dcim-eval")
        .arg(Arg::new("FILE")
            .help("input file(s) to process")
            .multiple(true)
            .required(true)
            .index(1))
        .arg(Arg::new("config_file")
            .help("Config file to read (optional)")
            .short('f')
            .long("config")
            .takes_value(true)
            .required(false))
        .arg(Arg::new("out_dir")
            .help("output directory - used as destination root, does not have to exist")
            .short('o')
            .long("output-dir")
            .default_value("sorted"))
        .arg(Arg::new("nprint_sorted")
            .help("suppress printing target path after evaluating metadata")
            .short('T')
            .long("no-print-target")
            .required(false))
        .arg(Arg::new("print_meta")
            .help("print relevant metadata read from input file(s)")
            .short('m')
            .long("print-metadata")
            .required(false))
        .get_matches();

    let mut inp_files = Vec::<PathBuf>::new();
    for arg in matches.values_of("FILE").unwrap() {
        let path = PathBuf::from(arg);
        if !path.is_file() {
            return Err(format!("file does not exist: {}", arg));
        }
        else {
            inp_files.push(path);
        }
    }

    let config_path = match matches.value_of("config_file") {
        Some(s) => Some(PathBuf::from(s)),
        None => None
    };

    let output_dir = PathBuf::from(matches.value_of("out_dir").unwrap());

    Ok(MainArgs{
        files: inp_files,
        cfg_file: config_path,
        out_dir: output_dir,
        print_sorting: !matches.is_present("nprint_sorted"),
        print_meta: matches.is_present("print_meta")
    })
}

fn read_file(inp_file: &Path) -> Result<ImgInfo, String> {
    let path_str = inp_file.to_str().unwrap_or(PATHSTR_FB);
    if !inp_file.is_file() {
        return Err(format!("failed reading file \"{}\": file does not exist", path_str));
    }

    match ImgInfo::new(inp_file.to_path_buf()) {
        Ok(i) => Ok(i),
        Err(e) => Err(format!("error reading file \"{}\": {}", path_str, e))
    }
}


/// build a default MetaProcessor with Rexiv2 as default and Kadamak as fallback
fn build_meta_proc() -> MetaProcessor {
    MetaProcessor::new()
        .processor(Rexiv2Processor::new(), Priority::Highest)
        .processor(KadamakExifProcessor::new(), Priority::Lowest)
        .build_clone()
}

/// build a default sorter/translator configuration
fn build_def_sorter() -> Sorter {
    Sorter::builder()
        .segment(MakeModelPattern::new()
            .case_normalization(CaseNormalization::Lowercase)
            .part(DevicePart::Make)
            .part(DevicePart::Model)
            .separator('_')
            .build())
        .segment(ScreenshotPattern::new(String::from("screenshots")))
        .segment(DateTimePattern::new()
            .part(DateTimePart::Year)
            .part(DateTimePart::Month)
            .separator('-')
            .build())
        .fallback(DummyPattern::new("other_files"))
        .build_sync()
}

/// helper to parse an XML-based config file including pre-checks
fn parse_config_file(filepath: &Path) -> Result<RootCfg, String> {
    let path_str = filepath.to_str().unwrap_or(dcim_sort::sorting::PATHSTR_FB);
    if !filepath.is_file() {
        return Err(format!("Invalid config file: {}", path_str)
        );
    }
    let mut file = match File::open(filepath) {
        Ok(f) => f,
        Err(e) => return Err(format!("Error opening config file \"{}\": {}", path_str, e))
    };

    match RootCfg::read_file(&mut file) {
        Ok(cfg) => Ok(cfg),
        Err(e) => Err(format!("Error parsing config file: {:?}", e))
    }
}

fn main() {
    let cfg = parse_args().unwrap();

    // if a config file is present, read that to build the sorter/translator
    let sorter = match &cfg.cfg_file {
        None => build_def_sorter(),
        Some(file) => {
            let root_cfg = parse_config_file(file.as_path()).unwrap();
            let mut builder = root_cfg.generate_sorter_builder().unwrap();
            builder.build_sync()
        }
    };
    let processor = build_meta_proc();

    for file in &cfg.files {
        let mut file_meta = read_file(file.as_path()).unwrap();
        processor.process(&mut file_meta);
        let action = sorter.calc_simulation(&file_meta, &cfg.out_dir.as_path());

        println!("file: {}", action.get_source().to_str().unwrap_or(PATHSTR_FB));
        if cfg.print_sorting {
            println!("\t==== sorting =====\n\ttype: {}\n\ttarget: {}",
                     match file_meta.file_type(){
                         FileType::JPEG => "JPEG",
                         FileType::PNG => "PNG",
                         FileType::HEIC => "HEIC",
                         FileType::DNG => "DNG",
                         FileType::Other => "other (unsupported metadata)"
                     },
                     action.get_target().to_str().unwrap_or(PATHSTR_FB)
            );
        }

        if cfg.print_meta {
            let meta = file_meta.metadata();
            println!("\t==== metadata ====\n\tmake: {}\n\tmodel: {}\n\ttimestamp: {}\n\tis_screenshot: {}\n\tuser_comment: {}",
                meta.make(),
                meta.model(),
                match meta.created_at() {
                    None => "<none>".to_string(),
                    Some(t) => t.format("%F %T").to_string()
                },
                meta.is_screenshot(),
                meta.user_comment()
            );
        }
        println!();
    }
}