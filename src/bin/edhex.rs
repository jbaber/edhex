fn print_help(name:&str) {
    println!("Usage: {} [options] <filename>

{} will read interactively from the user, or read commands from STDIN

Options:
    -h, --help    Print this help
    -v, --version Print version (if compiled with cargo)
    <filename>    Name of file to be edited
", name, name);
}


fn main() {
    let args = std::env::args().collect::<Vec<String>>();

    if args.len() != 2 {
        print_help(&args[0]);
        std::process::exit(1);
    }

    match &args[1][..] {
        "-h" | "--help" => {
            print_help(&args[0]);
            std::process::exit(0);
        },
        "-" => {
            println!("Not yet implemented.");
            std::process::exit(0);
        },
        "-v" | "--version" => {
            if let Some(version) = option_env!("CARGO_PKG_VERSION") {
                println!("{}", version);
            }
            else {
                println!("Version unknown (not compiled with cargo)");
            }
            std::process::exit(0);
        },
        filename => {
            std::process::exit(edhex::actual_runtime(filename));
        }
    }
}
