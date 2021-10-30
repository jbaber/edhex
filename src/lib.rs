use ec::hex_unless_dec;
use ec::State;
use regex::Regex;
use std::cmp::min;
use std::io;
use std::io::Read;
use std::io::Write;
use std::num::NonZeroUsize;

// TODO This is deprecated and should be
// replaced with
//     ec = {package = "edhex_core", version = "0.1.0}
// in Cargo.toml.  But that's only going to
// work for after Rust 1.26.0  Far enough in the future, use the Cargo.toml way.
extern crate edhex_core as ec;


macro_rules! skip_bad_range {
    ($command:expr, $all_bytes:expr) => {
        if $command.bad_range(&$all_bytes) {
            println!("? (bad range)");
            continue;
        }
    };
}


#[derive(Debug)]
struct Command {
    range: (usize, usize),
    command: char,
    args: Vec<String>,
}


fn print_help() {
    print!("Input/output is hex unless toggled to decimal with 'x'
h            This (h)elp
<Enter>      Print current byte(s) and move forward to next set of byte(s)
3d4          Move to byte number 3d4 and print from there
+            Move 1 byte forward and print from there
+++          Move 3 bytes forward and print from there
-            Move 1 byte back and print from there
+3d4         Move 3d4 bytes forward and print from there
-3d4         Move 3d4 bytes back and print from there
$            Move to last byte and print it
/deadbeef    If the bytes de ad be ef exist after the current index, move there
               and print.
?deadbeef    If the bytes de ad be ef exist before the current index, move there
               and print.
/            Perform last search again starting at next byte
?            Perform last search (backwards) again starting at previous byte
k            Delete/(k)ill byte at current index and print new line of byte(s)
7dk          Move to byte 7d, (k)ill that byte, and print from there.
1d,72k       Move to byte 1d, (k)ill bytes 1d - 72 inclusive, and print from there.
/deadbeef/k  If the bytes de ad be ef exist after the current index, move there,
               (k)ill those bytes, and print.
i            Prompt you to write out bytes which will be (i)nserted at current index
72i          Move to byte number 72 and prompt you to enter bytes which will be
               (i)nserted there.
/deadbeef/i  If the bytes de ad be ef exist after the current index, move there
               and prompt you to enter bytes which will be (i)nserted there.
12,3dp       (p)rint bytes 12 - 3d inclusive, move to byte 12
m            Toggle whether or not characters are printed after bytes
n            Toggle whether or not byte (n)umbers are printed before bytes
o            Toggle using c(o)lor
p            (p)rint current line of byte(s) (depending on 'W')
s            Print (s)tate of all toggles and 'W'idth
t3d          Print 0x3d lines of con(t)extual bytes after current line [Default 0]
T3d          Print 0x3d lines of con(T)extual bytes before current line [Default 0]
x            Toggle interpreting inputs and displaying output as he(x) or decimal
w            Actually (w)rite changes to the file on disk
W3d          Set (W)idth to 0x3d.  i.e. print a linebreak every 3d bytes [Default 0x10]
q            (q)uit
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

    ec::bytes_from_string(&input)
}


impl Command {
    fn bad_range(&self, all_bytes: &Vec<u8>) -> bool {
        ec::bad_range(all_bytes, self.range)
    }


    fn from_state_and_line(state:&mut State, line: &str) -> Result<Command, String> {
        // TODO Make these constants outside of this function so they don't get
        // created over and over
        // TODO Allow general whitespace, not just literal spaces
        let re_blank_line = Regex::new(r"^ *$").unwrap();
        let re_pluses = Regex::new(r"^ *(?P<pluses>\++) *$").unwrap();
        let re_minuses = Regex::new(r"^ *(?P<minuses>\-+) *$").unwrap();
        let re_search = Regex::new(r"^ *(?P<direction>[/?]) *(?P<bytes>[0-9a-fA-F]+) *$").unwrap();
        let re_search_again = Regex::new(r"^ *(?P<direction>[/?]) *$").unwrap();
        let re_search_kill = Regex::new(r"^ */(?P<bytes>[0-9a-fA-F]+)/k *$").unwrap();
        let re_search_insert = Regex::new(r"^ */(?P<bytes>[0-9a-fA-F]+)/i *$").unwrap();
        let re_single_char_command = Regex::new(r"^ *(?P<command>[hmnopsxqwik]).*$").unwrap();
        let re_range = Regex::new(r"^ *(?P<begin>[0-9a-fA-F.$]+) *, *(?P<end>[0-9a-fA-F.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_specified_index = Regex::new(r"^ *(?P<index>[0-9A-Fa-f.$]+) *(?P<the_rest>.*) *$").unwrap();
        let re_offset_index = Regex::new(r"^ *(?P<sign>[-+])(?P<offset>[0-9A-Fa-f]+) *(?P<the_rest>.*) *$").unwrap();
        let re_matches_nothing = Regex::new(r"^a\bc").unwrap();
        let re_width = Regex::new(r"^ *W *(?P<width>[0-9A-Fa-f]+) *$").unwrap();
        let re_before_context = Regex::new(r"^ *T *(?P<before_context>[0-9A-Fa-f]+) *$").unwrap();
        let re_after_context = Regex::new(r"^ *t *(?P<after_context>[0-9A-Fa-f]+) *$").unwrap();

        let is_blank_line          = re_blank_line.is_match(line);
        let is_single_char_command = re_single_char_command.is_match(line);
        let is_pluses              = re_pluses.is_match(line);
        let is_minuses             = re_minuses.is_match(line);
        let is_range               = re_range.is_match(line);
        let is_search              = re_search.is_match(line);
        let is_search_again        = re_search_again.is_match(line);
        let is_search_kill         = re_search_kill.is_match(line);
        let is_search_insert       = re_search_insert.is_match(line);
        let is_specified_index     = re_specified_index.is_match(line);
        let is_offset_index        = re_offset_index.is_match(line);
        let is_width               = re_width.is_match(line);
        let is_before_context      = re_before_context.is_match(line);
        let is_after_context       = re_after_context.is_match(line);

        let re = if is_blank_line {
            re_blank_line
        }
        else if is_single_char_command {
            re_single_char_command
        }
        else if is_search {
            re_search
        }
        else if is_search_again {
            re_search_again
        }
        else if is_search_insert {
            re_search_insert
        }
        else if is_search_kill {
            re_search_kill
        }
        else if is_pluses {
            re_pluses
        }
        else if is_minuses {
            re_minuses
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
        else if is_before_context {
            re_before_context
        }
        else if is_after_context {
            re_after_context
        }
        else if is_width {
            re_width
        }
        else {
            re_matches_nothing
        };

        let caps = re.captures(line);

        if is_pluses {
            let num_pluses = ec::num_graphemes(caps.unwrap().name("pluses").unwrap().as_str());
            Ok(Command{
                range: (num_pluses, num_pluses),
                command: 'G',
                args: vec![],
            })
        }

        else if is_search_insert {
            match ec::bytes_from_string(caps.unwrap().name("bytes").unwrap().as_str()) {
                Ok(needle) => {
                    if let Some(offset) = ec::index_of_bytes(&needle, &state.all_bytes[state.index..], true) {
                        Ok(Command{
                            range: (state.index + offset, state.index + offset),
                            command: 'i',
                            args: vec![],
                        })
                    }
                    else {
                        Err(format!("{} not found", ec::string_from_bytes(&needle)))
                    }
                },
                Err(error) => {
                    Err(error)
                }
            }
        }

        else if is_search_kill {
            match ec::bytes_from_string(caps.unwrap().name("bytes").unwrap().as_str()) {
                Ok(needle) => {
                    let needle_num_bytes = if needle.len() == 0 {
                        return Err("Searching for empty string".to_owned());
                    }
                    else {
                        needle.len()
                    };

                    if let Some(offset) = ec::index_of_bytes(&needle, &state.all_bytes[state.index..], true) {
                        Ok(Command{
                            range: (state.index + offset, state.index + offset + needle_num_bytes - 1),
                            command: 'k',
                            args: vec![],
                        })
                    }
                    else {
                        Err(format!("{} not found", ec::string_from_bytes(&needle)))
                    }
                },
                Err(error) => {
                    Err(error)
                }
            }
        }

        else if is_search_again {
            if state.last_search.is_none() {
                return Err(format!("No previous search."));
            }

            let needle = state.last_search.to_owned().unwrap();

            let caps = caps.unwrap();
            let forward = caps.name("direction").unwrap().as_str() == "/";

            /* Notice looking after current byte */
            let haystack = if forward {
                &state.all_bytes[(state.index + 1)..]
            }
            else {
                &state.all_bytes[..(state.index - 1)]
            };

            if let Some(offset) = ec::index_of_bytes(&needle, haystack, forward) {
                if forward {
                    Ok(Command{
                        range: (state.index + 1 + offset, state.index + 1 + offset),
                        command: 'g',
                        args: vec![],
                    })
                }
                else {
                    Ok(Command{
                        range: (offset, offset),
                        command: 'g',
                        args: vec![],
                    })
                }
            }
            else {
                Err(format!("{} not found", ec::string_from_bytes(&needle)))
            }
        }

        else if is_search {
            let caps = caps.unwrap();
            let forward = caps.name("direction").unwrap().as_str() == "/";
            match ec::bytes_from_string(caps.name("bytes").unwrap().as_str()) {
                Ok(needle) => {
                    state.last_search = Some(needle.to_owned());

                    let haystack = if forward {
                        &state.all_bytes[state.index..]
                    }
                    else {
                        &state.all_bytes[..state.index]
                    };
                    if let Some(offset) = ec::index_of_bytes(&needle, haystack, forward) {
                        if forward {
                            Ok(Command{
                                range: (state.index + offset, state.index + offset),
                                command: 'g',
                                args: vec![],
                            })
                        }
                        else {
                            Ok(Command{
                                range: (offset, offset),
                                command: 'g',
                                args: vec![],
                            })
                        }
                    }
                    else {
                        Err(format!("{} not found", ec::string_from_bytes(&needle)))
                    }
                },
                Err(error) => {
                    Err(error)
                }
            }
        }

        else if is_minuses {
            let num_minuses = ec::num_graphemes(caps.unwrap().name("minuses").unwrap().as_str());
            Ok(Command{
                range: (num_minuses, num_minuses),
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

        else if is_before_context {
            let caps = caps.unwrap();
            let given = caps.name("before_context").unwrap().as_str();
            if let Ok(before_context) = usize::from_str_radix(given, state.radix) {
              Ok(Command{
                  range: (usize::from(before_context), usize::from(before_context)),
                  command: 'T',
                  args: vec![],
              })
            }
            else {
                Err(format!("Can't interpret {} as a number", given))
            }
        }

        else if is_after_context {
            let caps = caps.unwrap();
            let given = caps.name("after_context").unwrap().as_str();
            if let Ok(after_context) = usize::from_str_radix(given, state.radix) {
              Ok(Command{
                  range: (usize::from(after_context), usize::from(after_context)),
                  command: 't',
                  args: vec![],
              })
            }
            else {
                Err(format!("Can't interpret {} as a number", given))
            }
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


fn get_input_or_die() -> Result<String, i32> {
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_num_bytes) => {

            /* EOF Return error of 0 to indicate time for a clean exit.  */
            if _num_bytes == 0 {
                Err(0)
            }
            else {
                Ok(input.trim().to_string())
            }
        }
        Err(_) => {
            println!("Unable to read input");
            Err(3)
        }
    }
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


/// Returns new index number
fn minuses(state:&mut State, num_minuses:usize) -> Result<usize, String> {
    if state.empty() {
        Err("Empty file".to_owned())
    }
    else if state.index == 0 {
        Err("already at 0th byte".to_owned())
    }
    else if state.index < num_minuses {
        Err(format!("Going back {} bytes would take you beyond the 0th byte", num_minuses))
    }
    else {
        state.index -= num_minuses;
        state.print_bytes_sans_context(state.range(), false);
        Ok(state.index)
    }
}

/// Returns new index number
fn pluses(state:&mut State, num_pluses:usize) -> Result<usize, String> {
    if state.empty() {
        Err("Empty file".to_owned())
    }
    else {
        match state.max_index() {
            Ok(max) => {
                if state.index == max {
                    Err("already at last byte".to_owned())
                }
                else if state.index + num_pluses > max {
                    Err(format!("Moving {} bytes would put you past last byte", num_pluses))
                }
                else {
                    state.index += num_pluses;
                    state.print_bytes_sans_context(state.range(), false);
                    Ok(state.index)
                }
            },
            Err(error) => {
                Err(error)
            },
        }
    }
}


pub fn actual_runtime(filename:&str, quiet:bool, color:bool) -> i32 {
    let file = match ec::filehandle(filename) {
        Ok(Some(filehandle)) => {
            Some(filehandle)
        },
        Ok(None) => None,
        Err(error) => {
            println!("Problem opening '{}' ({:?})", filename, error);
            return 3;
        }
    };

    let original_num_bytes = match ec::num_bytes_or_die(&file) {
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
    let mut state = ec::State{
        radix: 16,
        filename: filename.to_owned(),
        show_byte_numbers: true,
        show_prompt: !quiet,
        color: color,
        show_chars: true,
        unsaved_changes: false,
        index: 0,
        before_context: 0,
        after_context: 0,
        width: NonZeroUsize::new(16).unwrap(),
        all_bytes: all_bytes,
        // TODO calculate based on longest possible index
        n_padding: "      ".to_owned(),
        last_search: None,
    };

    if !quiet {
        println!("h for help\n");
        println!("{}", state);
        println!();
        state.print_bytes_sans_context(state.range(), false);
    }

    loop {
        if state.show_prompt {
            print!("*");
        }
        io::stdout().flush().unwrap();
        let input = match get_input_or_die() {
            Ok(input) => input,
            Err(errcode) => {
                return errcode;
            }
        };

        match Command::from_state_and_line(&mut state, &input) {
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
                        match ec::move_to(&mut state, command.range.0) {
                            Ok(_) => {
                                state.print_bytes();
                            },
                            Err(error) => {
                                println!("? ({})", error);
                            }
                        }
                    },

                    /* +'s */
                    'G' => {
                        match pluses(&mut state, command.range.0) {
                            Err(error) => {
                                println!("? ({})", error);
                            },
                            Ok(_) => {
                                continue;
                            }
                        }
                    },

                    /* -'s */
                    'H' => {
                        match minuses(&mut state, command.range.0) {
                            Err(error) => {
                                println!("? ({})", error);
                            },
                            Ok(_) => {
                                continue;
                            }
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
                                state.print_bytes_sans_context(state.range(),
                                        false);
                            },
                            Err(error) => {
                                println!("? ({})", error);
                            },
                        }
                    },

                    /* Help */
                    'h' => {
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
                        state.print_bytes_sans_context(state.range(), false);
                    },

                    /* Toggle showing char representations of bytes */
                    'm' => {
                        state.show_chars = !state.show_chars;
                        if !quiet {
                            println!("{}", state.show_chars);
                        }
                    },

                    /* Toggle showing byte number */
                    'n' => {
                        state.show_byte_numbers = !state.show_byte_numbers;
                        if !quiet {
                            println!("{}", state.show_byte_numbers);
                        }
                    },

                    /* Toggle color */
                    'o' => {
                        state.color = !state.color;
                        if !quiet {
                            println!("{}", state.color);
                        }
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
                                    state.print_bytes();
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
                        state.print_bytes();
                    },

                    /* Print byte(s) with range */
                    'p' => {
                        if state.empty() {
                            println!("? (Empty file)");
                            continue;
                        };

                        skip_bad_range!(command, state.all_bytes);
                        state.index = command.range.0;
                        if state.print_bytes_sans_context((command.range.0,
                                command.range.1), false).is_some() {
                            state.index = command.range.0;
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

                        state.print_bytes();
                    },

                    /* Quit */
                    'q' => {
                        return 0;
                    },

                    /* Print state */
                    's' => {
                        println!("{}", state);
                    },

                    /* Change after_context */
                    't' => {
                        state.after_context = usize::from(command.range.0);
                    },

                    /* Change before_context */
                    'T' => {
                        state.before_context = usize::from(command.range.0);
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
