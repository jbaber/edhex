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
use unicode_segmentation::UnicodeSegmentation;


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
    print!("Input/output is hex unless toggled to decimal with 'x'
?          This help
<Enter>    Print current byte(s) and move forward to next set of byte(s)
3d4        Move to byte number 3d4 and print from there
+          Move 1 byte forward and print from there
-          Move 1 byte back and print from there
+3d4       Move 3d4 bytes forward and print from there
-3d4       Move 3d4 bytes back and print from there
$          Move to last byte and print it
k          Delete (kill) byte at current index and print new line of byte(s)
7dk        Move to byte 7d, delete that byte, and print from there.
1d,72k     Move to byte 1d, delete bytes 1d - 72 inclusive, and print from there.
i          Prompt you to write out bytes which will be inserted at current index
72i        Move to byte number 72 and prompt you to enter bytes which will be
             inserted there.
12,3dp     Print bytes 12 - 3d inclusive, move to leftmost byte printed on the
             last line.
n          Toggle whether or not byte numbers are printed before bytes
p          Print current line of byte(s) (depending on 'W')
s          Print state of all toggles and 'W'idth
x          Toggle interpreting inputs and displaying output as hex or decimal
w          Actually write changes to the file on disk
W3d        Print a linebreak every 3d bytes [Default 0x10]
q          quit
");
}


fn read_bytes_from_user() -> Result<Vec<u8>, String> {
    print!("> ");
    io::stdout().flush().unwrap();
    let input = match get_input_or_die() {
        Ok(input) => input,
        Err(_errcode) => {
            return Err("Couldn't read input".to_owned());
        }
    };

    // TODO Allow general whitespace, not just literal spaces
    let re_bytes = Regex::new(r"^ *([0-9a-fA-F][0-9a-fA-F] *)* *$").unwrap();
    if re_bytes.is_match(&input) {
        let nibbles:Vec<String> = input.replace(" ", "").chars().map(|x| x.to_string()).collect();
        Ok(nibbles.chunks(2).map(|x| x.join("")).map(|x| u8::from_str_radix(&x, 16).unwrap()).collect())

    }
    else {
        Err(format!("Couldn't interpret '{}' as a sequence of bytes", &input))
    }
}


fn bad_range(bytes: &Vec<u8>, range: (usize, usize)) -> bool {
    bytes.len() == 0 || range.1 >= bytes.len()
}


impl Command {
    fn bad_range(&self, all_bytes: &Vec<u8>) -> bool {
        bad_range(all_bytes, self.range)
    }


    fn from_state_and_line(state:&State, line: &str) -> Result<Command, String> {
        // TODO Make these constants outside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_blank_line = Regex::new(r"^ *$").unwrap();
        let re_plus = Regex::new(r"^ *\+ *$").unwrap();
        let re_minus = Regex::new(r"^ *\- *$").unwrap();
        let re_single_char_command = Regex::new(r"^ *(?P<command>[?npsxqwik]).*$").unwrap();
        let re_range = Regex::new(r"^ *(?P<begin>[0-9a-fA-F.$]+) *, *(?P<end>[0-9a-fA-F.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_specified_index = Regex::new(r"^ *(?P<index>[0-9A-Fa-f.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_offset_index = Regex::new(r"^ *(?P<sign>[-+])(?P<offset>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_width = Regex::new(r"^ *W *(?P<width>[0-9A-Fa-f]+) *$").unwrap();

        let is_blank_line          = re_blank_line.is_match(line);
        let is_single_char_command = re_single_char_command.is_match(line);
        let is_plus                = re_plus.is_match(line);
        let is_minus               = re_minus.is_match(line);
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
        else if is_plus {
            re_plus
        }
        else if is_minus {
            re_minus
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

        if is_plus {
            Ok(Command{
                range: (0, 0),
                command: 'G',
                args: vec![],
            })
        }
        else if is_minus {
            Ok(Command{
                range: (0, 0),
                command: 'H',
                args: vec![],
            })
        }
        else if is_single_char_command {
            let command = caps.unwrap().name("command").unwrap().as_str().chars().next().unwrap();
            if command == 'p' {
                Ok(Command{
                    range: (state.index, state.index),
                    command: 'Q',
                    args: vec![],
                })
            }
            else {
                Ok(Command{
                    range: (state.index, state.index),
                    command: command,
                    args: vec![],
                })
            }
        }

        else if is_blank_line {
            Ok(Command{
                range: (state.index, state.index),
                command: '\n',
                args: vec![],
            })
        }

        else if is_width {
            // println!("is_width");
            let caps = caps.unwrap();
            if let Some(width) = NonZeroUsize::new(usize::from_str_radix(caps.name("width").unwrap().as_str(), state.radix).unwrap()) {
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
            if state.empty() {
                return Err("Empty file".to_owned());
            }

            let _max_index = match state.max_index() {
                Ok(max) => max,
                Err(error) => {
                    return Err(format!("? ({})", error));
                },
            };

            // println!("is_range");
            let caps = caps.unwrap();
            let begin = number_dot_dollar(state.index, _max_index,
                    caps.name("begin").unwrap().as_str(), state.radix);
            if begin.is_err() {
                // Why on Earth doesn't this work?
                // return Err(begin.unwrap());
                return Err("Can't understand beginning of range.".to_owned());
            }
            let begin = begin.unwrap();
            let end = number_dot_dollar(state.index, _max_index,
                    caps.name("end").unwrap().as_str(), state.radix);
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
            if state.empty() {
                return Err("Empty file".to_owned());
            }

            let _max_index = match state.max_index() {
                Ok(max) => max,
                Err(error) => {
                    return Err(format!("? ({})", error));
                },
            };

            // println!("is_specified_index");
            let caps = caps.unwrap();
            let specific_index = number_dot_dollar(state.index, _max_index,
                    caps.name("index").unwrap().as_str(), state.radix);
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
                let command = the_rest.chars().next().unwrap();
                let args = the_rest[1..].split_whitespace()
                        .map(|x| x.to_owned()).collect();
                Ok(Command{
                    range: (specific_index, specific_index),
                    command:
                        if command == 'p' {
                            'P'
                        }
                        else {
                          command
                        },
                    args: args,
                })
            }
        }

        else if is_offset_index {
            // println!("is_specified_index");
            let caps = caps.unwrap();
            let as_string = caps.name("offset").unwrap().as_str();
            let index_offset = usize::from_str_radix(as_string, state.radix);
            if index_offset.is_err() {
                return Err(format!("{} is not a number", as_string));
            }
            let index_offset = index_offset.unwrap();
            let sign = caps.name("sign").unwrap().as_str();
            let begin = match sign {
                "+" => state.index + index_offset,
                "-" => state.index - index_offset,
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


fn num_bytes_or_die(open_file: &Option<std::fs::File>) -> Result<usize, i32> {
    if open_file.is_none() {
        return Ok(0);
    }

    let metadata = open_file.as_ref().unwrap().metadata();
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
    fn test_num_graphemes() {
        assert_eq!(num_graphemes("hey, there"), 10);
        assert_eq!(num_graphemes("दीपक"), 3);
        assert_eq!(num_graphemes("ﷺ"), 1);
        assert_eq!(num_graphemes("père"), 4);
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
            hex_unless_dec_with_radix(state.all_bytes.len(), state.radix));
    if state.show_byte_numbers {
        println!("Printing byte numbers in {}",
            string_from_radix(state.radix));
    };
    println!("Interpreting input numbers as {}",
            string_from_radix(state.radix));
    println!("Printing a newline every {} bytes",
            hex_unless_dec_with_radix(usize::from(state.width), state.radix));
    if state.unsaved_changes {
        println!("Unwritten changes");
    }
    else {
        println!("No unwritten changes");
    }
}


/// returns index of the byte in the 0-th column of the last row printed
fn print_bytes(state:&State, range:(usize, usize)) -> Option<usize> {
    if state.empty() {
        return None;
    }

    let max = state.max_index();
    if max.is_err() {
        println!("? ({:?})", max);
        return None;
    }
    let max = max.unwrap();

    let from = range.0;
    let to = min(max, range.1);
    if bad_range(&state.all_bytes, (from, to)) {
      println!("? (Bad range: ({}, {}))", range.0, range.1);
      return None;
    }

    let bytes = &state.all_bytes[from..=to];
    let max_bytes_line_num = max_bytes_line(bytes, state.width);
    let mut left_col_byte_num = from;
    for bytes_line_num in 0..=max_bytes_line_num {
        if bytes_line_num != 0 {
            left_col_byte_num += usize::from(state.width);
        }
        if state.show_byte_numbers {
            if state.radix == 10 {
                print!("{:>5}{}", left_col_byte_num, state.n_padding);
            }
            else {
                print!("{:>5x}{}", left_col_byte_num, state.n_padding);
            }
        }
        let cur_line = bytes_line(bytes, bytes_line_num, state.width)
                .iter().map(|x| formatted_byte(*x, true))
                .collect::<Vec<String>>().join(" ");
        let cur_line = cur_line.trim();
        print!("|{}|", cur_line);
        // TODO Do this with format!
        if state.show_chars {
            print!("{}   {}", cur_line.len(), chars_line(bytes, bytes_line_num, state.width));
        }
        println!();
        left_col_byte_num = from + bytes_line_num * usize::from(state.width);
    }
    Some(left_col_byte_num)
}


/// .len gives the number of bytes
/// .chars.count() gives the number of characters (which counts è as two characters.
/// The human concept is unicode "graphemes" or "glyphs" defined to be what
/// think they are.
fn num_graphemes(unicode_string: &str) -> usize {
    return unicode_string.graphemes(true).count();
}

fn number_dot_dollar(index:usize, _max_index:usize, input:&str, radix:u32)
        -> Result<usize, String> {
    match input {
        "$" => Ok(_max_index),
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


fn hex_unless_dec_with_radix(number:usize, radix:u32) -> String {
    let letter = if radix == 10 {
        'd'
    }
    else {
        'x'
    };

    format!("0{}{}", letter, hex_unless_dec(number, radix))
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
    hex_unless_dec_with_radix(state.index, state.radix)
}


struct State {
    radix: u32,
    show_byte_numbers: bool,
    unsaved_changes: bool,
    filename: String,

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
        write!(f, "radix: {}|unsaved_changes: {}|show_byte_numbers: {}|index: {}|width: {}|n_padding: '{}'|filename: {}|",
                self.radix, self.unsaved_changes, self.show_byte_numbers,
                self.index, self.width, self.n_padding, self.filename)
    }
}


impl State {
    fn empty(&self) -> bool {
        self.all_bytes.len() == 0
    }

    fn range(&self) -> (usize, usize) {
        (self.index, self.index + usize::from(self.width) - 1)
    }

    fn max_index(&self) -> Result<usize, String> {
        if self.all_bytes.len() == 0 {
            Err("No bytes, so no max index.".to_owned())
        }
        else {
            Ok(self.all_bytes.len() - 1)
        }
    }
}


pub fn actual_runtime(filename: &str) -> i32 {
    let file = match File::open(filename) {
        Ok(filehandle) => {
            Some(filehandle)
        },
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                None
            }
            else {
                return 3;
            }
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
    if file.is_some() {
        match file.unwrap().read_to_end(&mut all_bytes) {
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
    }


    // TODO Below here should be a function called main_loop()
    let mut state = State{
        radix: 16,
        filename: filename.to_owned(),
        show_byte_numbers: true,
        unsaved_changes: false,
        index: 0,
        width: NonZeroUsize::new(16).unwrap(),
        all_bytes: all_bytes,
        // TODO calculate based on longest possible index
        n_padding: "      ".to_owned(),
    };

    println!("{} bytes\n? for help\n",
            hex_unless_dec(state.all_bytes.len(), state.radix));
    print_state(&state);
    println!();
    print_bytes(&state, state.range());

    loop {
        print!("*");
        io::stdout().flush().unwrap();
        let input = match get_input_or_die() {
            Ok(input) => input,
            Err(errcode) => {
                return errcode;
            }
        };

        match Command::from_state_and_line(&state, &input) {
            Ok(command) => {
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
                        print_bytes(&state, state.range());
                    },

                    /* + */
                    'G' => {
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        }

                        match state.max_index() {
                            Ok(max) => {
                                if state.index == max {
                                    println!("? (already at last byte");
                                }
                                else if state.index > max {
                                    println!("? (past last byte");
                                }
                                else {
                                    state.index += 1;
                                    print_bytes(&state, state.range());
                                }
                            },
                            Err(error) => {
                                println!("? ({})", error);
                            },
                        }
                    },

                    /* - */
                    'H' => {
                        if state.empty() {
                            println!("? (Empty file)");
                        }
                        else if state.index == 0 {
                            println!("? (already at 0th byte");
                        }
                        else {
                            state.index -= 1;
                            print_bytes(&state, state.range());
                        }
                    },

                    /* insert */
                    'i' => {
                        match read_bytes_from_user() {
                            Ok(entered_bytes) => {
                                state.index = command.range.1;
                                // TODO Find the cheapest way to do this (maybe
                                // make state.all_bytes a better container)
                                // TODO Do this with split_off
                                let mut new = Vec::with_capacity(state.all_bytes.len() + entered_bytes.len());
                                for i in 0..state.index {
                                    new.push(state.all_bytes[i]);
                                }
                                // TODO Could use Vec::splice here
                                for i in 0..entered_bytes.len() {
                                    new.push(entered_bytes[i]);
                                }
                                for i in state.index..state.all_bytes.len() {
                                    new.push(state.all_bytes[i]);
                                }
                                state.all_bytes = new;
                                state.unsaved_changes = true;
                                print_bytes(&state, state.range());
                            },
                            Err(error) => {
                                println!("? ({})", error);
                            },
                        }
                    },

                    /* Help */
                    '?'|'h' => {
                        print_help();
                    },

                    /* 'k'ill byte(s) (Can't use 'd' because that's a hex
                    * character! */
                    'k' => {
                        if state.empty() {
                            println!("? (Empty file");
                            continue;
                        }
                        skip_bad_range!(command, state.all_bytes);
                        let mut right_half = state.all_bytes.split_off(command.range.0);
                        right_half = right_half.split_off(command.range.1 - command.range.0 + 1);
                        state.all_bytes.append(&mut right_half);
                        state.index = command.range.0;
                        print_bytes(&state, state.range());
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
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        };

                        match state.max_index() {
                            Ok(max) => {
                                let width = usize::from(state.width);
                                let first_byte_to_show_index = state.index + width;
                                let last_byte_to_show_index = min(
                                        first_byte_to_show_index + width - 1,
                                                max);
                                if first_byte_to_show_index > max {
                                    println!("? (already showing last byte at index {})",
                                            hex_unless_dec(last_byte_to_show_index,
                                                    state.radix));
                                }
                                else {
                                    state.index = first_byte_to_show_index;
                                    print_bytes(&state, (first_byte_to_show_index,
                                            last_byte_to_show_index));
                                }
                            },
                            Err(error) => {
                                println!("? ({})", error);
                            }
                        }
                    }

                    /* Print byte(s) at one place, width long */
                    'P' => {
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        };

                        skip_bad_range!(command, state.all_bytes);
                        state.index = command.range.0;
                        if let Some(last_left_col_index) = print_bytes(&state, state.range()) {
                            state.index = last_left_col_index;
                        }
                        else {
                            println!("? (bad range {:?}", state.range());
                        }
                    },

                    /* Print byte(s) with range */
                    'p' => {
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        };

                        skip_bad_range!(command, state.all_bytes);
                        state.index = command.range.0;
                        if let Some(last_left_col_index) = print_bytes(&state, (command.range.0, command.range.1)) {
                        state.index = last_left_col_index;
                        }
                        else {
                            println!("? (bad range {:?}", command.range);
                        }
                    },

                    /* Print byte(s) at *current* place, width long */
                    'Q' => {
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        };

                        print_bytes(&state, state.range());
                    },

                    /* Quit */
                    'q' => {
                        return 0;
                    },

                    /* Print state */
                    's' => {
                        print_state(&state);
                    },

                    /* Write out */
                    'w' => {
                        let result = std::fs::write(filename, &state.all_bytes);
                        if result.is_err() {
                            println!("? (Couldn't write to {})", state.filename);
                        }
                    },

                    /* Change width */
                    'W' => {
                        if let Some(width) = NonZeroUsize::new(command.range.0) {
                            state.width = width;
                        }
                    },

                    /* Catchall error */
                    _ => {
                        println!("? (Don't understand command '{}')", command.command);
                        continue;
                    },
                }
            },
            Err(error) => {
                println!("? ({})", error);
                continue;
            }
        }
    }
}
