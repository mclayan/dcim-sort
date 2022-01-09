use std::fs::File;
use std::path::{Path, PathBuf};
use std::time;
use clap::{App, AppSettings, Arg};
use dcim_sort::config::RootCfg;
use dcim_sort::index::Scanner;
use dcim_sort::media::kadamak_exif::KadamakExifProcessor;
use dcim_sort::media::metadata_processor::{MetaProcessor, MetaProcessorBuilder, Priority};
use dcim_sort::media::rexiv_proc::Rexiv2Processor;
use dcim_sort::pattern::device::{CaseNormalization, DevicePart, MakeModelPattern};
use dcim_sort::pattern::fallback::SimpleFileTypePattern;
use dcim_sort::pattern::general::{DateTimePart, DateTimePattern, ScreenshotPattern};
use dcim_sort::pipeline::{Pipeline, PipelineController};
use dcim_sort::sorting::comparison::HashAlgorithm;
use dcim_sort::sorting::{ActionResult, DuplicateResolution, Operation, PATHSTR_FB, Sorter, SorterBuilder};

/// helper struct to collect common options from command-line args
struct MArgs {
    file: String,
    target_root: String,
    max_recursion: u8,
    debug: u64,
    ignore_unknown_types: bool,
    dry_run: bool,
    config_path: Option<PathBuf>,
    operation: Operation,
    thread_count: usize,
    hash_operation: HashAlgorithm
}

/// helper struct to collect pipeline configurations.
struct RuntimeCfg {
    scanner: Scanner,
    proc_builder: MetaProcessorBuilder,
    sorter_builder: SorterBuilder,
    output_dir: PathBuf,
    operation: Operation,
    dup_policy: DuplicateResolution,
    thread_count: usize
}

/// parse command-line args
fn parse_args() -> MArgs {
    let about_hash_algo = format!(
        "hash algorithm used for comparing files in case the same file exist already in the target directory. Possible values are: {:?}",
        HashAlgorithm::names());

    let name_outdir = "output-dir";
    let name_threads = "max-threads";
    let name_infile = "FILE";
    let name_max_recursion = "max-recursion";
    let name_debug = "debug";
    let name_ignore_ftype = "ignore-other-types";
    let name_cfg_path = "config";
    let name_simulate = "dry-run";
    let name_operation = "OPERATION";
    let name_hash_algo = "hash-algorithm";
    let name_hash_algo_none = "hash-algorithm-none";


    let matches = App::new("dcim-sort - sort images from DCIM folders")
        .version("0.1.0")
        .author("MCL")
        .about("Sort images from (unintuitive) DCIM file structures")
        .setting(AppSettings::UnifiedHelpMessage)
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
            .default_value("0")
            .about("maximum count of threads. Setting to 0 will disable threading."))
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
        .arg(Arg::new(name_hash_algo)
            .about(about_hash_algo.as_str())
            .multiple(false)
            .short('h')
            .long("hash-algorithm")
            .takes_value(true)
            .default_value(HashAlgorithm::names()[0])
        )
        .arg(Arg::new(name_hash_algo_none)
            .about("disables file hashing for comparison (same as '-h none')")
            .multiple(false)
            .short('H')
            .required(false)
            .takes_value(false)
        )
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

    let override_no_hash = matches.is_present(name_hash_algo_none);
    let hash_algo = match override_no_hash {
        true => HashAlgorithm::None,
        false => HashAlgorithm::parse(matches.value_of(name_hash_algo).unwrap())
    };

    let operation = match matches.subcommand_name().expect("Missing operation!") {
        "simulate" => Operation::Print,
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
        thread_count: max_threads,
        hash_operation: hash_algo
    }
}

/// a default sorter configuration
 fn generate_default_sorter() -> SorterBuilder {
    Sorter::builder()
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

/// main procedure for multi-threading scenarios
fn process_threaded(mut cfg: RuntimeCfg, args: &MArgs) {

    let mut controller = PipelineController::new(
        args.thread_count,
        cfg.proc_builder,
        cfg.sorter_builder,
        cfg.operation,
        cfg.output_dir.as_path(),
        cfg.dup_policy
    );

    let time_start = time::Instant::now();

    cfg.scanner.scan_pipeline(&mut controller);
    let report = controller.shutdown();

    let elapsed = chrono::Duration::from_std(time_start.elapsed()).unwrap();
    println!("finished in {:.4} seconds or {:03}:{:02}:{:02}", elapsed.num_milliseconds() as f64 / 1000.0,
             elapsed.num_hours(),
             elapsed.num_minutes() % 60,
             elapsed.num_seconds() % 3600
    );
    println!("{}", report);
}

/// main procedure for single-threaded scenarios
fn process_sync(mut cfg: RuntimeCfg, args: &MArgs) {
    let mut pipeline = Pipeline::new(
        cfg.proc_builder.build_clone(),
        cfg.sorter_builder.build_sync(),
        cfg.operation,
        cfg.output_dir.as_path(),
        cfg.dup_policy
    );

    let files = cfg.scanner.scan();
    for file in files {
        let fpath = String::from(file.path().to_str().unwrap_or(PATHSTR_FB));
        match pipeline.process(file) {
            Err(e) => panic!("Error while processing file: {}", e),
            Ok(r) => if args.debug > 0 {
                match r {
                    ActionResult::Moved => {
                        println!("moved \"{}\"", fpath);
                    }
                    ActionResult::Copied => {
                        println!("copied \"{}\"", fpath);
                    }
                    ActionResult::Skipped => {
                        println!("skipped \"{}\"", fpath);
                    }
                }
            }
        }
    }

}

/// helper to parse an XML-based config file including pre-checks
fn parse_config_file(filepath: &Path) -> Result<RootCfg, String>{
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

/// helper for constructing pipeline configuration from args and wrap it up in a struct
fn create_config(args: &MArgs) -> RuntimeCfg {
    let (dup_policy, sorter_builder) = match &args.config_path {
        None => (SorterBuilder::default_duplicate_handling(), generate_default_sorter()),
        Some(path) => {
            let root_cfg = parse_config_file(path.as_path()).unwrap();
            let dup_handling = root_cfg.get_sorter_cfg().get_duplicate_handling();
            let sorter_builder = root_cfg.generate_sorter_builder().unwrap()
                .hash_algorithm(args.hash_operation);
            (dup_handling, sorter_builder)
        }
    };

    let meta_proc_builder = MetaProcessor::new()
        .processor(Rexiv2Processor::new(), Priority::None)
        .processor(KadamakExifProcessor::new(), Priority::Lowest);

    let input_file = PathBuf::from(&args.file);
    if !input_file.exists() {
        panic!("Input file does not exist: \"{}\"", &args.file);
    }
    let mut scanner = Scanner::new(input_file.as_path()).unwrap();
    scanner.set_max_depth(args.max_recursion);
    scanner.ignore_unknown_types(args.ignore_unknown_types);


    let output_root = PathBuf::from(&args.target_root);
    if output_root.is_file() {
        panic!("specified output directory is an existing normal file: {}", &args.target_root);
    }

    RuntimeCfg{
        scanner: scanner,
        proc_builder: meta_proc_builder,
        sorter_builder: sorter_builder,
        output_dir: output_root,
        operation: args.operation,
        dup_policy: dup_policy,
        thread_count: args.thread_count
    }
}

fn main() {
    let args = parse_args();
    let cfg = create_config(&args);

    if args.thread_count <= 0 {
        process_sync(cfg, &args);
    }
    else {
        process_threaded(cfg, &args);
    }

}