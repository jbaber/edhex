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
            println!("? (bad range)");
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
<Enter>    Print current byte and move forward one byte
n          Toggle whether or not byte numbers are printed before bytes
N          Print current byte number followed by current byte
p          Print current byte
x          Toggle whether to interpret inputs and display output as hex or decimal
           (and print which has resulted)
X          Print whether inputs and line numbers are in hex or decimal
314        Move to byte number 0x314 (or 0d314 depending on 'x') and print that byte
$          Move to last byte and print it
12,34p     Print bytes 12 - 34 inclusive, then move to byte 0x34 (or 0d34 depending on 'x')
W30        Print a linebreak every 0x30 bytes (or 0d30 bytes depending on 'x')
W0         Print bytes without linebreaks
q          quit
");
}


impl Command {
    fn bad_range(&self, all_bytes: &Vec<u8>) -> bool {
        all_bytes.len() == 0 || self.range.1 >= all_bytes.len()
    }


    fn from_index_and_line(index: usize, line: &str,
            max_index: usize, radix: Option<u32>) -> Result<Command, String> {
        let radix = radix.unwrap_or(16);

        // TODO Make these constants outside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_blank_line = Regex::new(r"^ *$").unwrap();
        let re_single_char_command = Regex::new(r"^ *(?P<command>[?nNpxXq]).*$").unwrap();
        let re_range = Regex::new(r"^ *(?P<begin>[0-9a-fA-F.$]+) *, *(?P<end>[0-9a-fA-F.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_specified_index = Regex::new(r"^ *(?P<index>[0-9A-Fa-f.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_offset_index = Regex::new(r"^ *(?P<sign>[-+])(?P<offset>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_width = Regex::new(r"^ *W *(?P<width>[0-9]+) *$").unwrap();

        let is_single_char_command = re_single_char_command.is_match(line);
        let is_width               = re_width.is_match(line);
        let is_range               = re_range.is_match(line);
        let is_specified_index     = re_specified_index.is_match(line);
        let is_offset_index        = re_offset_index.is_match(line);
        let is_blank_line          = re_blank_line.is_match(line);

        let re = if is_single_char_command {
            re_single_char_command
        }
        else if is_blank_line {
            re_blank_line
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
        else if is_offset_index {
            re_offset_index
        }
        else {
            re_matches_nothing
        };

        let caps = re.captures(line);

        if is_single_char_command {
            Ok(Command{
                range: (0, 0),
                command: caps.unwrap().name("command").unwrap().as_str().chars().next().unwrap(),
                args: vec![],
            })
        }

        else if is_blank_line {
            Ok(Command{
                range: (index, index),
                command: '\n',
                args: vec![],
            })
        }

        else if is_width {
            // println!("is_width");
            let caps = caps.unwrap();
            let width = usize::from_str_radix(caps.name("width").unwrap().as_str(), radix).unwrap();
            Ok(Command{
                range: (width, width),
                command: 'W',
                args: vec![],
            })
        }

        else if is_range {
            // println!("is_range");
            let caps = caps.unwrap();
            let begin = number_dot_dollar(index, max_index,
                    caps.name("begin").unwrap().as_str(), radix);
            if begin.is_err() {
                // Why on Earth doesn't this work?
                // return Err(begin.unwrap());
                return Err("Can't understand beginning of range.".to_owned());
            }
            let begin = begin.unwrap();
            let end = number_dot_dollar(index, max_index,
                    caps.name("end").unwrap().as_str(), radix);
            if end.is_err() {
                // Why on Earth doesn't this work?
                // return end;
                return Err("Can't understand end of range.".to_owned());
            }
            let end = end.unwrap();

            let the_rest = caps.name("the_rest").unwrap().as_str().trim();
            if the_rest.len() == 0 {
                Err("No arguments given".to_owned())
            }
            else {
                Ok(Command{
                    range: (begin, end),
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if is_specified_index {
            // println!("is_specified_index");
            let caps = caps.unwrap();
            let specific_index = number_dot_dollar(index, max_index,
                    caps.name("index").unwrap().as_str(), radix);
            if specific_index.is_err() {
                // Why on Earth doesn't this work?
                // return specific_index;
                return Err("Can't understand index.".to_owned());
            }
            let specific_index = specific_index.unwrap();
            let the_rest = caps.name("the_rest").unwrap().as_str().trim().to_owned();
            if the_rest.len() == 0 {
                Ok(Command{
                    range: (specific_index, specific_index),
                    command: 'g',
                    args: vec![],
                })
            }
            else {
                Ok(Command{
                    range: (specific_index, specific_index),
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else if is_offset_index {
            // println!("is_specified_index");
            let caps = caps.unwrap();
            let index_offset = usize::from_str_radix(caps.name("offset").unwrap().as_str(), radix).unwrap();
            let sign = caps.name("sign").unwrap().as_str();
            let begin = match sign {
                "+" => index + index_offset,
                "-" => index - index_offset,
                _   => {
                    return Err(format!("Unknown sign {}", sign));
                }
            };
            let range = (begin, begin);
            let the_rest = caps.name("the_rest").unwrap().as_str();
            if the_rest.len() == 0 {
                Ok(Command{
                    range: range,
                    command: 'g',
                    args: vec![],
                })
            }
            else {
                Ok(Command{
                    range: range,
                    command: the_rest.chars().next().unwrap(),
                    args: the_rest[1..].split_whitespace().map(|x| x.to_owned()).collect(),
                })
            }
        }

        else {
            Err(format!("Unable to parse '{}'", line.trim()))
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
        width: Option<usize>, show_byte_numbers: bool) {
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


fn number_dot_dollar(index:usize, max_index:usize, input:&str, radix:u32)
        -> Result<usize, String> {
    match input {
        "$" => Ok(max_index),
        "." => Ok(index),
        something_else => {
            if let Ok(number) = usize::from_str_radix(input, radix) {
                Ok(number)
            }
            else {
                return Err(format!("{} isn't a number in base {}", something_else, radix));
            }
        }
    }
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
    let mut show_byte_numbers = true;
    let mut radix = Some(16);

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

        if let Ok(command) = Command::from_index_and_line(index, &input,
                all_bytes.len() - 1, radix) {
            // println!("0x{:?}", command);
            match command.command {
                'e' => {
                    println!("?");
                    continue;
                },
                'g' => {
                    if command.bad_range(&all_bytes) {
                        println!("? (bad range)");
                        continue;
                    }
                    index = command.range.1;
                    print_one_byte(all_bytes[index]);
                },
                'h' => {
                    print_help();
                },
                'n' => {
                    show_byte_numbers = !show_byte_numbers;
                    println!("{}", show_byte_numbers);
                },
                'N' => {
                    println!("{:x}{}{}", index, n_padding, all_bytes[index]);
                },
                'x' => {
                    match radix {
                        Some(16) => {
                            radix = Some(10);
                        },
                        _ => {
                            radix = Some(16);
                        },
                    }
                },
                'X' => {
                    println!("Input and output in {}", match radix {
                        Some(10) => "decimal",
                        _ => "hex",
                    });
                }
                '\n' => {
                    print_one_byte(all_bytes[index]);
                    index += 1;
                }
                'p' => {
                    skip_bad_range!(command, all_bytes);
                    print_bytes(&all_bytes, command.range.0, command.range.1,
                            None, width, show_byte_numbers);
                    index = command.range.1;
                },
                'q' => {
                    return 0;
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
                    println!("? (Don't understand {})", command.command);
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
