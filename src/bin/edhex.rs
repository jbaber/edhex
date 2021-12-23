// TODO This is deprecated and should be
// replaced with
//     ec = {package = "edhex_core", version = "0.1.0}
// in Cargo.toml.  But that's only going to
// work for after Rust 1.26.0  Far enough in the future, use the Cargo.toml way.
extern crate edhex_core as ec;

fn print_help(name:&str) {
    println!("Usage: {} [options] <filename>

{} will read interactively from the user, or read commands from STDIN

Options:
    -h, --help         Print this help
    -n, --no-color
    -q, --quiet        Don't print prompts or initial help text and state
                       e.g. for clean output when piping commands into the program
    -v, --version      Print version (if compiled with cargo)
    -c, --core-version Print version of core library (if compiled with cargo)
    <filename>     Name of file to be edited
", name, name);
}


fn print_cargo_version() {
    if let Some(version) = option_env!("CARGO_PKG_VERSION") {
        println!("{}", version);
    }
    else {
        println!("Version unknown (not compiled with cargo)");
    }
}


fn main() {
    let args = std::env::args().collect::<Vec<String>>();

    let possible_args = [
        "-h", "--help", "-v", "--version", "-q", "--quiet", "-n", "--no-color",
    ];

    if args.iter().position(|x| x == "-h" || x == "--help").is_some() {
        print_help(&args[0]);
        std::process::exit(0);
    }

    if args.iter().position(|x| x == "-v" || x == "--version").is_some() {
        // ec::print_cargo_version();
        print_cargo_version();
        std::process::exit(0);
    }

    if args.iter().position(|x| x == "-c" || x == "--core-version").is_some() {
        ec::print_cargo_version();
        std::process::exit(0);
    }

    let quiet = args.iter().position(|x| x == "-q" || x == "--quiet" || x == "-qn" || x == "-nq").is_some();
    let color = !args.iter().position(|x| x == "-n" || x == "--no-color" || x == "-qn" || x == "-nq").is_some();

    /* First non-flag considered the filename */
    let mut filename_given = false;
    let mut filename = "";
    for index in 1..args.len() {
        let cur = &args[index];
        if !possible_args.iter().position(|x| x == &cur).is_some() {
            filename_given = true;
            filename = &cur;
        }
    }


    if !filename_given {
        println!("No filename provided");
        print_help(&args[0]);
        std::process::exit(1)
    }

    std::process::exit(edhex::actual_runtime(&filename, quiet, color))
}
