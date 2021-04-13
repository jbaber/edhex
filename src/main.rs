use std::env;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use regex::Regex;


#[derive(Debug)]
struct Command {
    range: (usize, usize),

    /* 'g' means goto
     * 'p' means print
     */
    command: char,
    args: Vec<String>,
}


impl Command {
    fn from_index_and_line(index: usize, line: &str) -> Option<Command> {

        // TODO Make these constants utside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_hex_range = Regex::new(r"^ *0x(?P<begin>[0-9a-fA-F]+) *, *0x(?P<end>[0-9a-fA-F]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_range = Regex::new(r"^ *(?P<begin>[0-9]+) *, *(?P<end>[0-9]+) *(?P<the_rest>.*) *$").unwrap();
        let re_hex_specified_index = Regex::new(r"^ *0x(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_specified_index = Regex::new(r"^ *(?P<index>[0-9]+) *(?P<the_rest>.*) *$").unwrap();

        if re_hex_range.is_match(line) {
            let caps = re_hex_range.captures(line).unwrap();
            let range = (usize::from_str_radix(caps.name("begin").unwrap().as_str(), 16).unwrap(),
                         usize::from_str_radix(caps.name("end"  ).unwrap().as_str(), 16).unwrap());
            let the_rest = caps.name("the_rest").unwrap().as_str().trim();
            if the_rest.len() == 0 {
                None
            }
            else {
                Some(Command{
                    range: range,
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if re_dec_range.is_match(line) {
            let caps = re_dec_range.captures(line).unwrap();
            let range = (usize::from_str_radix(caps.name("begin").unwrap().as_str(), 10).unwrap(),
                        usize::from_str_radix(caps.name("end"  ).unwrap().as_str(), 10).unwrap());
            let the_rest = caps.name("the_rest").unwrap().as_str().trim();
            if the_rest.len() == 0 {
                None
            }
            else {
                Some(Command{
                    range: range,
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if re_hex_specified_index.is_match(line) {
            let caps = re_hex_specified_index.captures(line).unwrap();
            let range = (usize::from_str_radix(caps.name("index").unwrap().as_str(), 16).unwrap(),
                         usize::from_str_radix(caps.name("index").unwrap().as_str(), 16).unwrap());
            let the_rest = caps.name("the_rest").unwrap().as_str().trim();
            if the_rest.len() == 0 {
                Some(Command{
                    range: range,
                    command: 'g',
                    args: vec![],
                })
            }
            else {
                Some(Command{
                    range: range,
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if re_dec_specified_index.is_match(line) {
            let caps = re_dec_specified_index.captures(line).unwrap();
            let range = (usize::from_str_radix(caps.name("index").unwrap().as_str(), 10).unwrap(),
                         usize::from_str_radix(caps.name("index").unwrap().as_str(), 10).unwrap());
            let the_rest = caps.name("the_rest").unwrap().as_str();
            if the_rest.len() == 0 {
                Some(Command{
                    range: range,
                    command: 'g',
                    args: vec![],
                })
            }
            else {
                Some(Command{
                    range: range,
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        /* Not a range, so just a command with arguments */
        else {
            let line = line.trim();
            match line.len() {
                0 => Some(Command{
                    range: (index + 1, index + 1),
                    command: 'g',
                    args: vec![],
                }),
                _ => Some(Command{
                    range: (index, index),
                    command: line.chars().next().unwrap(),
                    args: line[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                }),
            }
        }
    }
}


fn open_or_die(filename: &str) -> std::fs::File {
    match File::open(filename) {
        Ok(filehandle) => {
            filehandle
        }
        Err(_) => {
            println!("Couldn't open '{}'", filename);
            std::process::exit(3)
        }
    }

}


fn get_input_or_die() -> String {
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_num_bytes) => {
            input.trim().to_string()
        }
        Err(_) => {
            println!("Unable to read input");
            std::process::exit(3)
        }
    }
}


fn num_bytes_or_die(open_file: &std::fs::File) -> usize {
    let metadata = open_file.metadata();
    match metadata {
        Ok(metadata) => {
            metadata.len() as usize
        }
        Err(_) => {
            println!("Couldn't find file size");
            std::process::exit(2)
        }
    }
}


fn print_one_byte(byte:u8) {
    if byte < 10 {
        println!("0x0{:x}", byte);
    }
    else {
        println!("0x{:x}", byte);
    }
}


fn main() {
    let args = env::args().collect::<Vec<String>>();

    if args.len() != 2 {
        println!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];
    let mut file = open_or_die(&filename);
    let num_bytes = num_bytes_or_die(&file);

    /* Read all bytes into memory just like real ed */
    let mut all_bytes = Vec::new();
    match file.read_to_end(&mut all_bytes) {
        Err(_) => {
            println!("Couldn't read {}", filename);
            std::process::exit(4);
        },
        Ok(num_bytes_read) => {
            if num_bytes_read != num_bytes {
                println!("Only read {} of {} bytes of {}", num_bytes_read,
                        num_bytes, filename);
                std::process::exit(5);
            }
        }
    }


    let max_index = num_bytes - 1;
    let mut index = max_index;

    println!("0x{:x}", index);
    loop {
        print!("*");
        io::stdout().flush().unwrap();
        let input = get_input_or_die();
        if let Some(command) = Command::from_index_and_line(index, &input) {
            // println!("0x{:?}", command);
            match command.command {
                'q' => std::process::exit(0),
                'g' => {
                    if command.range.0 > max_index {
                        println!("?");
                        continue;
                    }
                    index = command.range.0;
                    print_one_byte(all_bytes[index]);
                },
                'p' => {
                    if command.range.0 > max_index {
                        println!("?");
                        continue;
                    }
                    index = command.range.0;
                    print_one_byte(all_bytes[index]);
                },
                'n' => {
                    if command.range.0 > max_index {
                        println!("?");
                        continue;
                    }
                    index = command.range.0;
                    print!("0x{:x}    ", index);
                    print_one_byte(all_bytes[index]);
                },
                '$' => {
                    index = max_index;
                    print_one_byte(all_bytes[index]);
                }
                _ => {
                    println!("?");
                    continue;
                },
            }
        }
        else {
            println!("?");
            continue;
        }
    }
}
