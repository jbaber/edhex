use std::env;


fn main() {
    let args = env::args().collect::<Vec<String>>();

    if args.len() != 2 {
        println!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }
}
