use std::cmp::min;
use std::fmt;
use std::num::NonZeroUsize;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use regex::Regex;
use ansi_term::Color;
use ansi_term::Color::Fixed;


macro_rules! skip_bad_range {
    ($command:expr, $all_bytes:expr) => {
        if $command.bad_range(&$all_bytes) {
            println!("? (bad range)");
            continue;
        }
    };
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
<Enter>    Print current byte(s) and move forward to next set of byte(s)
314        Move to byte number 0x314 (or 0d314 depending on 'x') and print from
               there
$          Move to last byte and print it
12,34p     Print bytes 12 - 34 inclusive, then move to byte 0x34
               (or 0d34 depending on 'x')
n          Toggle whether or not byte numbers are printed before bytes
p          Print current byte(s) (depending on 'W')
s          Print state of all toggles and 'W'idth
x          Toggle interpreting inputs and displaying output as hex or decimal
W30        Print a linebreak every 0x30 bytes (or 0d30 bytes depending on 'x')
               [Default 0x10]
q          quit
");
}


impl Command {
    fn bad_range(&self, all_bytes: &Vec<u8>) -> bool {
        all_bytes.len() == 0 || self.range.1 >= all_bytes.len()
    }


    fn from_index_and_line(index: usize, line: &str,
            max_index: usize, radix: u32) -> Result<Command, String> {
        // TODO Make these constants outside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_blank_line = Regex::new(r"^ *$").unwrap();
        let re_single_char_command = Regex::new(r"^ *(?P<command>[?npsxq]).*$").unwrap();
        let re_range = Regex::new(r"^ *(?P<begin>[0-9a-fA-F.$]+) *, *(?P<end>[0-9a-fA-F.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_specified_index = Regex::new(r"^ *(?P<index>[0-9A-Fa-f.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_offset_index = Regex::new(r"^ *(?P<sign>[-+])(?P<offset>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_width = Regex::new(r"^ *W *(?P<width>[0-9A-Fa-f]+) *$").unwrap();

        let is_blank_line          = re_blank_line.is_match(line);
        let is_single_char_command = re_single_char_command.is_match(line);
        let is_range               = re_range.is_match(line);
        let is_specified_index     = re_specified_index.is_match(line);
        let is_offset_index        = re_offset_index.is_match(line);
        let is_width               = re_width.is_match(line);

        let re = if is_blank_line {
            re_blank_line
        }
        else if is_single_char_command {
            re_single_char_command
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
        else if is_width {
            re_width
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
            if let Some(width) = NonZeroUsize::new(usize::from_str_radix(caps.name("width").unwrap().as_str(), radix).unwrap()) {
              Ok(Command{
                  range: (usize::from(width), usize::from(width)),
                  command: 'W',
                  args: vec![],
              })
            }
            else {
                Err("Width must be positive".to_owned())
            }
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


fn string_from_radix(radix: u32) -> String {
    if radix == 10 {
        "decimal".to_owned()
    }
    else {
        "hex".to_owned()
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


fn max_bytes_line(bytes:&[u8], width:NonZeroUsize) -> usize {
    if bytes.len() == 0 {
        0
    }
    else {
        (bytes.len() - 1) / usize::from(width)
    }
}


fn bytes_line(bytes:&[u8], line_number:usize, width:NonZeroUsize) -> &[u8] {
    let width = usize::from(width);
    if line_number * width < bytes.len() {
        let end_index = min(bytes.len(), line_number * width + width);
        &bytes[line_number * width..end_index]
    }

    else {
        &[]
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padded_byte() {
        assert_eq!(padded_byte(2), "02");
        assert_eq!(padded_byte(10), "0a");
    }

    #[test]
    fn test_max_bytes_line() {
        let _1 = NonZeroUsize::new(1).unwrap();
        let _2 = NonZeroUsize::new(2).unwrap();
        let _3 = NonZeroUsize::new(3).unwrap();
        let _4 = NonZeroUsize::new(4).unwrap();
        let _5 = NonZeroUsize::new(5).unwrap();
        let _6 = NonZeroUsize::new(6).unwrap();
        let _7 = NonZeroUsize::new(7).unwrap();
        let _8 = NonZeroUsize::new(8).unwrap();
        let bytes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        assert_eq!(max_bytes_line(&bytes, _1), 12);
        let bytes = vec![8, 6, 7, 5, 3, 0, 9,];
        assert_eq!(max_bytes_line(&bytes, _1), 6);
        assert_eq!(max_bytes_line(&bytes, _2), 3);
        assert_eq!(max_bytes_line(&bytes, _3), 2);
        assert_eq!(max_bytes_line(&bytes, _4), 1);
        assert_eq!(max_bytes_line(&bytes, _5), 1);
        assert_eq!(max_bytes_line(&bytes, _6), 1);
        assert_eq!(max_bytes_line(&bytes, _7), 0);
        assert_eq!(max_bytes_line(&bytes, _8), 0);
        let bytes = vec![8, 6, 7, 5, 3, 0,];
        assert_eq!(max_bytes_line(&bytes, _1), 5);
        assert_eq!(max_bytes_line(&bytes, _2), 2);
        assert_eq!(max_bytes_line(&bytes, _3), 1);
        assert_eq!(max_bytes_line(&bytes, _4), 1);
        assert_eq!(max_bytes_line(&bytes, _5), 1);
        assert_eq!(max_bytes_line(&bytes, _6), 0);
        assert_eq!(max_bytes_line(&bytes, _7), 0);
        assert_eq!(max_bytes_line(&bytes, _8), 0);
        let bytes = vec![8, 6, 7,];
        assert_eq!(max_bytes_line(&bytes, _1), 2);
        assert_eq!(max_bytes_line(&bytes, _2), 1);
        assert_eq!(max_bytes_line(&bytes, _3), 0);
        assert_eq!(max_bytes_line(&bytes, _4), 0);
        assert_eq!(max_bytes_line(&bytes, _5), 0);
        let bytes = vec![8, 6,];
        assert_eq!(max_bytes_line(&bytes, _1), 1);
        assert_eq!(max_bytes_line(&bytes, _2), 0);
        assert_eq!(max_bytes_line(&bytes, _3), 0);
        assert_eq!(max_bytes_line(&bytes, _4), 0);
        let bytes = vec![8,];
        assert_eq!(max_bytes_line(&bytes, _1), 0);
        assert_eq!(max_bytes_line(&bytes, _2), 0);
        assert_eq!(max_bytes_line(&bytes, _3), 0);
        assert_eq!(max_bytes_line(&bytes, _4), 0);
        let bytes = vec![];
        assert_eq!(max_bytes_line(&bytes, _1), 0);
        assert_eq!(max_bytes_line(&bytes, _2), 0);
        assert_eq!(max_bytes_line(&bytes, _3), 0);
        assert_eq!(max_bytes_line(&bytes, _4), 0);
    }

    #[test]
    fn test_bytes_line() {
        let bytes = vec![];
        let _1 = NonZeroUsize::new(1).unwrap();
        let _2 = NonZeroUsize::new(2).unwrap();
        let _3 = NonZeroUsize::new(3).unwrap();
        let _4 = NonZeroUsize::new(4).unwrap();
        let _5 = NonZeroUsize::new(5).unwrap();
        let _6 = NonZeroUsize::new(6).unwrap();
        let _7 = NonZeroUsize::new(7).unwrap();
        let _8 = NonZeroUsize::new(8).unwrap();
        assert_eq!(bytes_line(&bytes, 0, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _2).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 1, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 1, _2).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 2, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 2, _2).to_owned(), vec![]);
        let bytes = vec![8, 6, 7, 5, 3, 0, 9,];
        assert_eq!(bytes_line(&bytes, 0, _1).to_owned(), vec![8,]);
        assert_eq!(bytes_line(&bytes, 1, _1).to_owned(), vec![6,]);
        assert_eq!(bytes_line(&bytes, 2, _1).to_owned(), vec![7,]);
        assert_eq!(bytes_line(&bytes, 3, _1).to_owned(), vec![5,]);
        assert_eq!(bytes_line(&bytes, 4, _1).to_owned(), vec![3,]);
        assert_eq!(bytes_line(&bytes, 5, _1).to_owned(), vec![0,]);
        assert_eq!(bytes_line(&bytes, 6, _1).to_owned(), vec![9,]);
        assert_eq!(bytes_line(&bytes, 7, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 8, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 9, _1).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _2).to_owned(), vec![8, 6,]);
        assert_eq!(bytes_line(&bytes, 1, _2).to_owned(), vec![7, 5,]);
        assert_eq!(bytes_line(&bytes, 2, _2).to_owned(), vec![3, 0,]);
        assert_eq!(bytes_line(&bytes, 3, _2).to_owned(), vec![9,]);
        assert_eq!(bytes_line(&bytes, 4, _2).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 5, _2).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _3).to_owned(), vec![8, 6, 7,]);
        assert_eq!(bytes_line(&bytes, 1, _3).to_owned(), vec![5, 3, 0,]);
        assert_eq!(bytes_line(&bytes, 2, _3).to_owned(), vec![9,]);
        assert_eq!(bytes_line(&bytes, 3, _3).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 4, _3).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _4).to_owned(), vec![8, 6, 7, 5,]);
        assert_eq!(bytes_line(&bytes, 1, _4).to_owned(), vec![3, 0, 9,]);
        assert_eq!(bytes_line(&bytes, 2, _4).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 3, _4).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 4, _4).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _5).to_owned(), vec![8, 6, 7, 5, 3,]);
        assert_eq!(bytes_line(&bytes, 1, _5).to_owned(), vec![0, 9,]);
        assert_eq!(bytes_line(&bytes, 2, _5).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 3, _5).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _6).to_owned(), vec![8, 6, 7, 5, 3, 0,]);
        assert_eq!(bytes_line(&bytes, 1, _6).to_owned(), vec![9,]);
        assert_eq!(bytes_line(&bytes, 2, _6).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 3, _6).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _7).to_owned(), vec![8, 6, 7, 5, 3, 0, 9,]);
        assert_eq!(bytes_line(&bytes, 1, _7).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 2, _7).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 3, _7).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 0, _8).to_owned(), vec![8, 6, 7, 5, 3, 0, 9,]);
        assert_eq!(bytes_line(&bytes, 1, _8).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 2, _8).to_owned(), vec![]);
        assert_eq!(bytes_line(&bytes, 3, _8).to_owned(), vec![]);
    }
}


fn print_state(state:&State) {
    println!("At byte {} of {}", lino(&state),
            hex_unless_dec(state.all_bytes.len(), state.radix));
    if state.show_byte_numbers {
        println!("Printing byte numbers in {}",
            string_from_radix(state.radix));
    };
    println!("Interpreting input numbers as {}",
            string_from_radix(state.radix));
    if state.radix == 10 {
        println!("Printing a newline every 0d{} bytes", state.width);
    }
    else {
        println!("Printing a newline every 0x{:x} bytes", state.width);
    }
}


fn print_bytes(state:&State, range:(usize, usize)) {
    let bytes = &state.all_bytes[range.0..=range.1];
    let max_bytes_line_num = max_bytes_line(bytes, state.width);
    for bytes_line_num in 0..=max_bytes_line_num {
        if state.show_byte_numbers {
            let left_col_byte_num = range.0 + bytes_line_num * usize::from(state.width);
            if state.radix == 10 {
                print!("{:>5}{}", left_col_byte_num, state.n_padding);
            }
            else {
                print!("{:>5x}{}", left_col_byte_num, state.n_padding);
            }
        }
        let cur_line = bytes_line(bytes, bytes_line_num, state.width)
                .iter().map(|x| formatted_byte(*x, true)).collect::<Vec<String>>().join(" ");
        println!("{}", cur_line);
    }
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


fn hex_unless_dec(number:usize, radix:u32) -> String {
    if radix == 10 {
        format!("{}", number)
    }
    else {
        format!("{:x}", number)
    }
}


fn lino(state:&State) -> String {
    hex_unless_dec(state.index, state.radix)
}


struct State {
    radix: u32,
    show_byte_numbers: bool,

    /* Current byte number, 0 to len -1 */
    index: usize,

    width: NonZeroUsize,

    /* The bytes in memory */
    all_bytes: Vec<u8>,

    /* Spaces to put between a byte number and a byte when displaying */
    n_padding: String,
}


impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "radix: {}|show_byte_numbers: {}|index: {}|width: {}|n_padding: '{}'|",
                self.radix, self.show_byte_numbers, self.index, self.width,
                self.n_padding)
    }
}


fn range(state:&State) -> (usize, usize) {
    (state.index, state.index + usize::from(state.width) - 1)
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

    let original_num_bytes = match num_bytes_or_die(&file) {
        Ok(num_bytes) => {
            num_bytes
        },
        Err(errcode) => {
            return errcode;
        }
    };

    /* Read all bytes into memory just like real ed */
    // TODO A real hex editor needs to buffer
    let mut all_bytes = Vec::new();
    match file.read_to_end(&mut all_bytes) {
        Err(_) => {
            println!("Couldn't read {}", filename);
            return 4;
        },
        Ok(num_bytes_read) => {
            if num_bytes_read != original_num_bytes {
                println!("Only read {} of {} bytes of {}", num_bytes_read,
                        original_num_bytes, filename);
                return 5;
            }
        }
    }


    // TODO Below here should be a function called main_loop()
    let mut state = State{
        radix: 16,
        show_byte_numbers: true,
        index: 0,
        width: NonZeroUsize::new(16).unwrap(),
        all_bytes: all_bytes,
        // TODO calculate based on longest possible index
        n_padding: "      ".to_owned(),
    };

    // TODO Handle new file with *no* bytes yet.
    println!("{} bytes\n? for help\n",
            hex_unless_dec(state.all_bytes.len(), state.radix));
    print_state(&state);
    println!();
    print_bytes(&state, range(&state));
    loop {
        print!("*");
        io::stdout().flush().unwrap();
        let input = match get_input_or_die() {
            Ok(input) => input,
            Err(errcode) => {
                return errcode;
            }
        };

        if let Ok(command) = Command::from_index_and_line(state.index, &input,
                state.all_bytes.len() - 1, state.radix) {
            // println!("{:?}", command);
            match command.command {

                /* Error */
                'e' => {
                    println!("?");
                    continue;
                },

                /* Go to */
                'g' => {
                    if command.bad_range(&state.all_bytes) {
                        println!("? (bad range)");
                        continue;
                    }
                    state.index = command.range.1;
                    print_bytes(&state, range(&state));
                },

                /* Help */
                '?'|'h' => {
                    print_help();
                },

                /* Toggle showing byte number */
                'n' => {
                    state.show_byte_numbers = !state.show_byte_numbers;
                    println!("{}", state.show_byte_numbers);
                },

                /* Toggle hex/dec */
                'x' => {
                    state.radix = if state.radix == 16 {
                        10
                    }
                    else {
                        16
                    }
                },

                /* User pressed enter */
                '\n' => {
                    let width = usize::from(state.width);
                    let max_index = state.all_bytes.len() - 1;
                    let first_byte_to_show_index = state.index + width;
                    let last_byte_to_show_index = min(
                            first_byte_to_show_index + width - 1,
                                    max_index);
println!("{} - {}", hex_unless_dec(first_byte_to_show_index, state.radix), hex_unless_dec(last_byte_to_show_index, state.radix));
                    if first_byte_to_show_index > max_index {
                        println!("? (already showing last byte at index {})",
                                hex_unless_dec(last_byte_to_show_index,
                                        state.radix));
                    }
                    else {
                        state.index = first_byte_to_show_index;
                        print_bytes(&state, (first_byte_to_show_index,
                                last_byte_to_show_index));
                    }
                }

                /* Print byte(s) */
                'p' => {
                    skip_bad_range!(command, state.all_bytes);
                    print_bytes(&state, (command.range.0, command.range.1));
                    state.index = command.range.1;
                },

                /* Quit */
                'q' => {
                    return 0;
                },

                /* Print state */
                's' => {
                    print_state(&state);
                },

                /* Change width */
                'W' => {
                    if let Some(width) = NonZeroUsize::new(command.range.0) {
                        state.width = width;
                    }
                },

                /* Catchall error */
                _ => {
                    println!("? (Don't understand {})", command.command);
                    continue;
                },
            }
        }

        /* Couldn't parse command */
        else {
            println!("?");
            continue;
        }
    }
}
