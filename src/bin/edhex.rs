// TODO This is deprecated and should be
// replaced with
//     ec = {package = "edhex_core", version = "0.1.0}
// in Cargo.toml.  But that's only going to
// work for after Rust 1.26.0  Far enough in the future, use the Cargo.toml way.
extern crate edhex_core as ec;
extern crate clap;
use clap::{Arg, App};
use std::path::Path;


fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let called_name = &args[0];
    let version = match &edhex::cargo_version() {
        Ok(version) => {
            version.to_owned()
        },
        Err(message) => {
            message.to_owned()
        }
    };
    let about = "A hex editor that works vaguely like ed.\nIt can read \
            interactively from the user or read commands from STDIN.";

    let default_prefs_path = ec::preferences_file_path();
    let default_prefs_path_s = format!("{}", default_prefs_path.display());
    let default_state_path = ec::state_file_path();
    let default_state_path_s = format!("{}", default_state_path.display());
    let matches = App::new(called_name)
        .version(version.as_str())
        .about(about)
        .set_term_width(79)
        .arg(Arg::with_name("nocolor").short("n").long("no-color")
                .takes_value(false))
        .arg(Arg::with_name("readonly").short("R").long("read-only")
                .takes_value(false).help("Read-only mode.  Don't let you \
                        write changes to disk."))
        // .default_value is not flexible enough
        .arg(Arg::with_name("prefs-filename.json").short("p").long("preferences")
                .takes_value(true)
                .help(&format!("Load preferences from <prefs-filename{}{}{}",
                        ".json>\n[DEFAULT: ", default_prefs_path_s, "]"))
        )
        // .default_value is not flexible enough
        .arg(Arg::with_name("state-filename.json").short("s").long("state")
                .takes_value(true)
                .help(&format!("Load state from <state-filename{}{}{}{}{}",
                        ".json>\nAny file named ", default_state_path_s,
                        " will be automatically loaded.\n",
                        "If given -p, those preferences will take precedence\n",
                        "over preferences in <state-filename.json>"))
        )
        .arg(Arg::with_name("quiet").short("q").long("quiet")
                .takes_value(false).help("Don't print prompts or initial \
                        help text and state\ne.g. for clean output when \
                        piping commands in"))
        .arg(Arg::with_name("filename").required(false).help("Name of file to \
                be edited.  If not given, a new file will be \
                created on write"))
        .get_matches();

    let quiet = matches.is_present("quiet");
    let color = !matches.is_present("nocolor");
    let filename_given = matches.is_present("filename");
    let filename = match matches.value_of("filename") {
        None => "",
        Some(filename) => filename,
    };
    let readonly = matches.is_present("readonly");
    let prefs_path = match matches.value_of("prefs-filename.json") {
        Some(path_s) => Path::new(path_s).to_path_buf(),
        None => default_prefs_path,
    };
    let state_path = match matches.value_of("state-filename.json") {
        Some(path_s) => Path::new(path_s).to_path_buf(),
        None => default_state_path,
    };


    if !filename_given {
        println!("No filename provided\nOpening empty buffer");
    }

    if filename_given && !ec::path_exists(filename) {
        println!("Will create new file named '{}' on write", filename);
    }

    if filename_given && ec::path_exists(filename) && !ec::is_a_regular_file(filename) {
        println!("{} isn't a regular file", filename);
        std::process::exit(1);
    }

    std::process::exit(edhex::actual_runtime(&filename, quiet, color, readonly,
            prefs_path.to_path_buf(), state_path.to_path_buf()))
}
