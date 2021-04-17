use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use regex::Regex;
use ansi_term::Color;
use ansi_term::Color::Fixed;


macro_rules! skip_bad_range {
    ($command:ident, $all_bytes:ident) => {
        if $command.bad_range(&$all_bytes) {
            println!("?");
            continue;
        }
    }
}


/* Byte formatting stuff lifted from hexyl */
const COLOR_NULL: Color = Fixed(1);
const COLOR_ASCII_PRINTABLE: Color = Color::Cyan;
const COLOR_ASCII_WHITESPACE: Color = Color::Green;
const COLOR_ASCII_OTHER: Color = Color::Purple;
const COLOR_NONASCII: Color = Color::Yellow;

pub enum ByteCategory {
    Null,
    AsciiPrintable,
    AsciiWhitespace,
    AsciiOther,
    NonAscii,
}

#[derive(Copy, Clone)]
struct Byte(u8);

impl Byte {
    fn category(self) -> ByteCategory {
        if self.0 == 0x00 {
            ByteCategory::Null
        } else if self.0.is_ascii_graphic() {
            ByteCategory::AsciiPrintable
        } else if self.0.is_ascii_whitespace() {
            ByteCategory::AsciiWhitespace
        } else if self.0.is_ascii() {
            ByteCategory::AsciiOther
        } else {
            ByteCategory::NonAscii
        }
    }

    fn color(self) -> &'static Color {
        use crate::ByteCategory::*;

        match self.category() {
            Null => &COLOR_NULL,
            AsciiPrintable => &COLOR_ASCII_PRINTABLE,
            AsciiWhitespace => &COLOR_ASCII_WHITESPACE,
            AsciiOther => &COLOR_ASCII_OTHER,
            NonAscii => &COLOR_NONASCII,
        }
    }

    fn as_char(self) -> char {
        use crate::ByteCategory::*;

        match self.category() {
            Null => '0',
            AsciiPrintable => self.0 as char,
            AsciiWhitespace if self.0 == 0x20 => ' ',
            AsciiWhitespace => '_',
            AsciiOther => '•',
            NonAscii => '×',
        }
    }
}

#[derive(Debug)]
struct Command {
    range: (usize, usize),
    command: char,
    args: Vec<String>,
}


fn print_help() {
    print!("Input is interpreted as hex unless toggled to decimal with 'x'
?          This help
n          Toggle whether or not byte numbers are printed before bytes
N          Print current byte number followed by current byte
p          Print current byte
x          Toggle whether to interpret inputs as hex or decimal (and print which has resulted)
X          Print whether inputs are interpreted in hex or decimal
314        Move to byte number 0x314 (or 0d314 depending on 'x') and print that byte
$          Move to last byte and print it
12,34p     Print bytes 12 - 34 inclusive, then move to byte 0x34 (or 0d34 depending on 'x')
W30        Print a linebreak every 30 bytes
W0         Print bytes without linebreaks
q          quit
");
}


impl Command {
    fn bad_range(&self, all_bytes: &Vec<u8>) -> bool {
        all_bytes.len() == 0 || self.range.1 >= all_bytes.len()
    }


    fn from_index_and_line(index: usize, line: &str,
            max_index: usize, radix: Option<usize>) -> Option<Command> {

        // TODO Make these constants outside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_toggle_byte_numbers = Regex::new(r"^ *n.*$").unwrap();
        let re_print_byte_number = Regex::new(r"^ *N.*$").unwrap();
        let re_range = Regex::new(r"^ *(?P<begin>[0-9a-fA-F.$]+) *, *(?P<end>[0-9a-fA-F.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_specified_index = Regex::new(r"^ *(?P<index>[0-9A-Fa-f.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_minus_index = Regex::new(r"^ *-(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_plus_index = Regex::new(r"^ *+(?P<index>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_help = Regex::new(r"^ *\?").unwrap();
        let re_width = Regex::new(r"^ *W *(?P<width>[0-9]+) *$").unwrap();

        let is_help                = re_help.is_match(line);
        let is_toggle_byte_numbers = re_toggle_byte_numbers.is_match(line);
        let is_print_byte_number   = re_print_byte_number.is_match(line);
        let is_width               = re_width.is_match(line);
        let is_range               = re_range.is_match(line);
        let is_specified_index     = re_specified_index.is_match(line);
        let is_minus_index         = re_minus_index.is_match(line);
        let is_plus_index          = re_plus_index.is_match(line);

        let is_offset_index        = is_minus_index || is_plus_index;

        let begin: usize;
        let end: usize;

        let re = if is_help {
            re_help
        }
        else if is_toggle_byte_numbers {
            re_toggle_byte_numbers
        }
        else if is_print_byte_number {
            re_print_byte_number
        }
        else if is_width {
            re_width
        }
        else if is_range {
            re_range
        }
        else if is_specified_index {
            re_specified_index
        }
        else if is_plus_index {
            re_plus_index
        }
        else if is_minus_index {
            re_minus_index
        }
        else {
            re_matches_nothing
        };

        let caps = re.captures(line);

        /* TODO START HERE rewriting as above help text descripbes */

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
                command: 'W',
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


fn open_or_die(filename: &str) -> Result<std::fs::File, i32> {
    match File::open(filename) {
        Ok(filehandle) => {
            Ok(filehandle)
        }
        Err(_) => {
            println!("Couldn't open '{}'", filename);
            Err(3)
        }
    }

}


fn get_input_or_die() -> Result<String, i32> {
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_num_bytes) => {
            Ok(input.trim().to_string())
        }
        Err(_) => {
            println!("Unable to read input");
            Err(3)
        }
    }
}


fn num_bytes_or_die(open_file: &std::fs::File) -> Result<usize, i32> {
    let metadata = open_file.metadata();
    match metadata {
        Ok(metadata) => {
            Ok(metadata.len() as usize)
        }
        Err(_) => {
            println!("Couldn't find file size");
            Err(2)
        }
    }
}


fn formatted_byte(byte:u8, color:bool) -> String {
    if color {
        Byte(byte).color().paint(padded_byte(byte)).to_string()
    }
    else {
        padded_byte(byte)
    }
}


fn padded_byte(byte:u8) -> String {
    format!("{:02x}", byte)
}


fn print_bytes(all_bytes:&Vec<u8>, from_index: usize, to_index: usize, n_padding: Option<&str>,
        width: Option<usize>) {
    if n_padding.is_some() {
        for i in from_index..to_index + 1 {
            println!("0x{:x}{}{}", i, n_padding.unwrap(), formatted_byte(all_bytes[i], true));
        }
    }
    else {
        let mut counter: usize = 0;
        for i in from_index..to_index {
            counter += 1;
            print!("{}", formatted_byte(all_bytes[i], true));
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
        println!("{}", formatted_byte(all_bytes[to_index], true));
    }
}


fn print_one_byte(byte:u8) {
    println!("{}", formatted_byte(byte, true));
}


pub fn actual_runtime(filename: &str) -> i32 {
    let mut file = match open_or_die(&filename) {
        Ok(file) => {
            file
        },
        Err(errcode) => {
            return errcode;
        }
    };

    let num_bytes = match num_bytes_or_die(&file) {
        Ok(num_bytes) => {
            num_bytes
        },
        Err(errcode) => {
            return errcode;
        }
    };

    // TODO calculate based on longest possible index
    let n_padding = "     ";

    /* Read all bytes into memory just like real ed */
    // TODO A real hex editor needs to buffer
    let mut all_bytes = Vec::new();
    match file.read_to_end(&mut all_bytes) {
        Err(_) => {
            println!("Couldn't read {}", filename);
            return 4;
        },
        Ok(num_bytes_read) => {
            if num_bytes_read != num_bytes {
                println!("Only read {} of {} bytes of {}", num_bytes_read,
                        num_bytes, filename);
                return 5;
            }
        }
    }


    // TODO Below here should be a function called main_loop()
    let mut index = num_bytes - 1;
    let mut width: Option<usize> = None;

    println!("? for help\n\n0x{:x}", index);
    loop {

        print!("*");
        io::stdout().flush().unwrap();
        let input = match get_input_or_die() {
            Ok(input) => input,
            Err(errcode) => {
                return errcode;
            }
        };

        if let Some(command) = Command::from_index_and_line(index, &input,
                all_bytes.len() - 1) {
            // println!("0x{:?}", command);
            match command.command {
                'e' => {
                    println!("?");
                    continue;
                },
                'g' => {
                    if command.bad_range(&all_bytes) {
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
                    skip_bad_range!(command, all_bytes);

                    /* n10 should error, just like real ed */
                    if command.args.len() != 0 {
                        println!("?");
                        continue;
                    }
                    print_bytes(&all_bytes, command.range.0, command.range.1,
                            Some(n_padding), None);
                    index = command.range.1;
                },
                'p' => {
                    skip_bad_range!(command, all_bytes);
                    print_bytes(&all_bytes, command.range.0, command.range.1,
                            None, width);
                    index = command.range.1;
                },
                'q' => {
                    return 0
                },
                'W' => {
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
