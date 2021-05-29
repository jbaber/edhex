fn print_help(name:&str) {
    println!("Usage: {} [options] <filename>

{} will read interactively from the user, or read commands from STDIN

Options:
    -h, --help     Print this help
    -n, --no-color 
    -q, --quiet    Don't print prompts or initial help text and state
                   e.g. for clean output when piping commands into the program
    -v, --version  Print version (if compiled with cargo)
    <filename>     Name of file to be edited
", name, name);
}


fn main() {
    let args = std::env::args().collect::<Vec<String>>();

    if args.iter().position(|x| x == "-h" || x == "--help").is_some() {
        print_help(&args[0]);
        std::process::exit(0);
    }

    if args.iter().position(|x| x == "-v" || x == "--version").is_some() {
        if let Some(version) = option_env!("CARGO_PKG_VERSION") {
            println!("{}", version);
        }
        else {
            println!("Version unknown (not compiled with cargo)");
        }
        std::process::exit(0);
    }

    let quiet = args.iter().position(|x| x == "-q" || x == "--quiet" || x == "-qn" || x == "-nq").is_some();
    let color = !args.iter().position(|x| x == "-n" || x == "--no-color" || x == "-qn" || x == "-nq").is_some();
    let filename = args.last();

    if let Some(filename) = filename {
        return std::process::exit(edhex::actual_runtime(&filename, quiet, color));
    }
    else {
        return std::process::exit(1);
    }
}
