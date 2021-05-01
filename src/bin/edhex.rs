fn main() {
    let args = std::env::args().collect::<Vec<String>>();

    if args.len() != 2 {
        println!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    match &args[1][..] {
        "-v" | "--version" => {
            if let Some(version) = option_env!("CARGO_PKG_VERSION") {
                println!("{}", version);
            }
            else {
                println!("Version unknown (not compiled with cargo)");
            }
            std::process::exit(1);
        },
        filename => {
            std::process::exit(edhex::actual_runtime(filename));
        }
    }
}
