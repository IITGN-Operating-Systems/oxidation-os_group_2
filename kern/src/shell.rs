use shim::io::Write;
use shim::io;
use shim::path::{Path, PathBuf};

use stack_vec::StackVec;

use pi::atags::Atags;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;

use core::prelude::rust_2024::derive;

use core::fmt::Debug;
use core::iter::Iterator;
use core::result::Result;
use core::result::Result::{Err, Ok};

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
    Empty,
    TooManyArgs,
}

/// A structure representing a single shell command.
struct Command<'a> {
    args: StackVec<'a, &'a str>,
}

impl<'a> Command<'a> {
    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args = StackVec::new(buf);
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(arg).map_err(|_| Error::TooManyArgs)?;
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        self.args[0]
    }
}

/// Starts a shell using `prefix` as the prefix for each line. This function
/// returns if the `exit` command is called.
use core::str::from_utf8;
const MAX_LINE_LENGTH: usize = 512;
pub fn shell(prefix: &str) -> ! {
    kprintln!("{}", WELCOME_TXT);

    let mut console = CONSOLE.lock();
    loop {
        kprint!("{} ", prefix);
        let mut storage = [0; MAX_LINE_LENGTH]; // maxiumum command size
        let mut line: StackVec<u8> = StackVec::new(&mut storage);
        let mut idx = 0;

        // get bytes
        loop {
            match console.read_byte() {
                b'\r' | b'\n' => break,
                8 | 127 => {
                    if idx != 0 {
                        console.write_byte(8u8);
                        console.write_byte(b' ');
                        console.write_byte(8u8);
                        idx -= 1;
                        line.pop();
                    }
                }
                byte if (byte as char).is_ascii() && idx < MAX_LINE_LENGTH => match line.push(byte) {
                    Ok(()) => {
                        kprint!("{}", byte as char);
                        idx += 1;
                    }
                    Err(()) => {
                        console
                            .write("failed".as_bytes())
                            .expect("failed to write to console");
                    }
                },
                _ => {
                    console.write_byte(7u8); // rings the bell
                }
            }
        }
        kprintln!("");
        match from_utf8(line.into_slice()){ 
            Ok(command_string) if command_string.len() != 0 => {
                let mut buf = [""; 64];
                match Command::parse(command_string, &mut buf) {
                    Ok(command) if command.path() == "echo" => {
                        command.args.iter().skip(1).for_each(|s| kprint!("{} ", *s));
                        kprintln!("");
                    },
                    Ok(command) if command.path() == "welcome"=> {
                        kprintln!("{}", WELCOME_TXT);
                    },
                    Ok(command) => {
                        kprintln!("unknown command: {}", command.path());
                    },
                    Err(Error::TooManyArgs)  => {
                        kprintln!("error: too many arguments");
                    },
                    _ => {
                        kprintln!("error: failed to parse");
                    }
                }
            }, 
            _ => {}
        }
    }
}
