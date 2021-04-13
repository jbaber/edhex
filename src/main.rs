use std::env;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use regex::Regex;

#[derive(Debug)]
struct Command {
    range: (usize, usize),
    command: char,
    args: Vec<String>,
}


fn print_help() {
    print!("
?          This help
p          Print current byte (in hex)
n          Print current byte number (in hex) followed by current byte (in hex)
314        Move to byte number   314 (in dec) and print that byte (in hex)
0x314      Move to byte number 0x314 and print that byte (in hex)
$          Move to last byte and print it (in hex)
12,34p     Print bytes 12 - 34 inclusive (in hex), then move to byte 34
0x12,0x34p Print bytes 0x12 - 0x34 inclusive (in hex), then move to byte 0x34
w30        Print a linebreak every 30 bytes
w0         Print bytes without linebreaks
q          quit
");
}


impl Command {
    fn from_index_and_line(index: usize, line: &str, max_index: usize) -> Option<Command> {

        // TODO Make these constants utside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_hex_range = Regex::new(r"^ *0x(?P<begin>[0-9a-fA-F]+) *, *0x(?P<end>[0-9a-fA-F]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_range = Regex::new(r"^ *(?P<begin>[0-9]+) *, *(?P<end>[0-9]+) *(?P<the_rest>.*) *$").unwrap();
        let re_hex_range_with_dollar = Regex::new(r"^ *0x(?P<begin>[0-9a-fA-F]+) *, *(?P<end>\$) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_range_with_dollar = Regex::new(r"^ *(?P<begin>[0-9]+) *, *(?P<end>\$) *(?P<the_rest>.*) *$").unwrap();
        let re_hex_specified_index = Regex::new(r"^ *0x(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_specified_index = Regex::new(r"^ *(?P<index>[0-9]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dollar = Regex::new(r"^ *\$ *$").unwrap();
        let re_hex_minus_index = Regex::new(r"^ *-0x(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_hex_plus_index = Regex::new(r"^ *+0x(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_minus_index = Regex::new(r"^ *-(?P<index>[0-9]+) *(?P<the_rest>.*) *$").unwrap();
        let re_dec_plus_index  = Regex::new(r"^ *\+(?P<index>[0-9]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_help = Regex::new(r"^ *\?").unwrap();
        let re_width = Regex::new(r"^ *w *(?P<width>[0-9]+) *$").unwrap();

        let is_help                  = re_help.is_match(line);
        let is_width                 = re_width.is_match(line);
        let is_hex_range             = re_hex_range.is_match(line);
        let is_dec_range             = re_dec_range.is_match(line);
        let is_hex_range_with_dollar = re_hex_range_with_dollar.is_match(line);
        let is_dec_range_with_dollar = re_dec_range_with_dollar.is_match(line);
        let is_hex_specified_index   = re_hex_specified_index.is_match(line);
        let is_dec_specified_index   = re_dec_specified_index.is_match(line);
        let is_dollar                = re_dollar.is_match(line);
        let is_hex_minus_index       = re_hex_minus_index.is_match(line);
        let is_dec_minus_index       = re_dec_minus_index.is_match(line);
        let is_hex_plus_index        = re_hex_plus_index.is_match(line);
        let is_dec_plus_index        = re_dec_plus_index.is_match(line);

        let is_range_with_dollar   = is_hex_range_with_dollar || is_dec_range_with_dollar;
        let is_range               = is_dec_range || is_hex_range || is_range_with_dollar;
        let is_specified_index     = is_dec_specified_index || is_hex_specified_index || is_dollar;
        let is_minus_index         = is_dec_minus_index || is_hex_minus_index;
        let is_plus_index          = is_dec_plus_index || is_hex_plus_index;
        let is_hex                 = is_hex_range || is_hex_specified_index ||
                                     is_hex_minus_index || is_hex_plus_index;

        let is_offset_index        = is_minus_index || is_plus_index;

        let begin: usize;
        let end: usize;

        /* check hex first everywhere since 0x... looks like line 0 followed by a command
         * called 'x' */
        let re = if is_dollar {
            re_dollar
        }
        else if is_help {
            re_help
        }
        else if is_width {
            re_width
        }
        else if is_hex_range {
            re_hex_range
        }
        else if is_dec_range {
            re_dec_range
        }
        else if is_hex_range_with_dollar {
            re_hex_range_with_dollar
        }
        else if is_dec_range_with_dollar {
            re_dec_range_with_dollar
        }
        else if is_hex_specified_index {
            re_hex_specified_index
        }
        else if is_dec_specified_index {
            re_dec_specified_index
        }
        else if is_hex_plus_index {
            re_hex_plus_index
        }
        else if is_dec_plus_index {
            re_dec_plus_index
        }
        else if is_hex_minus_index {
            re_hex_minus_index
        }
        else if is_dec_minus_index {
            re_dec_minus_index
        }
        else {
            re_matches_nothing
        };

        let caps = re.captures(line);

        let radix = if is_hex {
            16
        }
        else {
            10
        };

        if is_help {
            Some(Command{
                range: (0, 0),
                command: 'h',
                args: vec![],
            })
        }
        else if is_width {
            // println!("is_width");
            let caps = caps.unwrap();
            let width = usize::from_str_radix(caps.name("width").unwrap().as_str(), 10).unwrap();
            Some(Command{
                range: (width, width),
                command: 'w',
                args: vec![],
            })
        }
        else if is_range {
            // println!("is_range");
            let caps = caps.unwrap();
            begin = usize::from_str_radix(caps.name("begin").unwrap().as_str(), radix).unwrap();
            end   = if is_range_with_dollar {
                max_index
            }
            else {
                usize::from_str_radix(caps.name("end"  ).unwrap().as_str(), radix).unwrap()
            };
            let the_rest = caps.name("the_rest").unwrap().as_str().trim();
            if the_rest.len() == 0 {
                None
            }
            else {
                Some(Command{
                    range: (begin, end),
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if is_specified_index {
            // println!("is_specified_index");
            let caps = caps.unwrap();
            let specific_index = if is_dollar {
                max_index
            }
            else {
                usize::from_str_radix(caps.name("index").unwrap().as_str(), radix).unwrap()
            };
            let begin = specific_index;
            let the_rest = if is_dollar {
                String::new()
            }
            else {
                caps.name("the_rest").unwrap().as_str().trim().to_owned()
            };
            if the_rest.len() == 0 {
                Some(Command{
                    range: (begin, begin),
                    command: 'g',
                    args: vec![],
                })
            }
            else {
                Some(Command{
                    range: (begin, begin),
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }


        else if is_offset_index {
            // println!("is_specified_index");
            let caps = caps.unwrap();
            let index_offset = usize::from_str_radix(caps.name("index").unwrap().as_str(), radix).unwrap();
            let begin = if is_plus_index {
                index + index_offset
            }
            else {
                index - index_offset
            };
            let range = (begin, begin);
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

        /* Now just a command with arguments */
        else {
            // println!("just a command");
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


fn padded_byte(byte:u8) -> String {
    return if byte < 0x10 {
        format!("0{:x}", byte)
    }
    else {
        format!("{:x}", byte)
    }
}


fn print_bytes(all_bytes:&Vec<u8>, from_index: usize, to_index: usize, n_padding: Option<&str>,
        width: Option<usize>) {
    if n_padding.is_some() {
        for i in from_index..to_index + 1 {
            println!("0x{:x}{}{}", i, n_padding.unwrap(), padded_byte(all_bytes[i]));
        }
    }
    else {
        let mut counter: usize = 0;
        for i in from_index..to_index {
            counter += 1;
            print!("{}", padded_byte(all_bytes[i]));
            if let Some(w) = width {
                if counter >= w {
                    counter = 0;
                    println!();
                }
                else {
                    print!(" ");
                }
            }
            else {
                print!(" ");
            }
        }
        println!("{}", padded_byte(all_bytes[to_index]));
    }
}


fn print_one_byte(byte:u8) {
    println!("0x{}", padded_byte(byte));
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
    // TODO calculate based on longest possible index
    let n_padding = "     ";

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
    // TODO Below here should be a function called main_loop()
    let mut index = max_index;
    let mut width: Option<usize> = None;

    println!("? for help\n\n0x{:x}", index);
    loop {
        print!("*");
        io::stdout().flush().unwrap();
        let input = get_input_or_die();
        if let Some(command) = Command::from_index_and_line(index, &input, max_index) {
            // println!("0x{:?}", command);
            match command.command {
                'e' => {
                    println!("?");
                    continue;
                },
                'g' => {
                    if command.range.1 > max_index {
                        println!("?");
                        continue;
                    }
                    index = command.range.1;
                    print_one_byte(all_bytes[index]);
                },
                'h' => {
                    print_help();
                },
                'n' => {

                    /* n10 should error, just like real ed */
                    if (command.range.1 > max_index) || (command.args.len() != 0) {
                        println!("?");
                        continue;
                    }
                    print_bytes(&all_bytes, command.range.0, command.range.1,
                            Some(n_padding), None);
                    index = command.range.1;
                },
                'p' => {
                    if command.range.1 > max_index {
                        println!("?");
                        continue;
                    }
                    print_bytes(&all_bytes, command.range.0, command.range.1,
                            None, width);
                    index = command.range.1;
                },
                'q' => std::process::exit(0),
                'w' => {
                    width = if command.range.0 > 0 {
                        Some(command.range.0)
                    }
                    else {
                        None
                    }
                },
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
