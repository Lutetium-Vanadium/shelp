//! `shelp` is a library to create a functional and good looking REPL without having to worry about
//! the generic setup and interacting and the terminal. It provides a configurable interface,
//! allowing you to only need to deal with the language specific parts of the REPL.
//!
//! There are special 2 commands handled by the repl:
//! - `clear` - clears the screen
//! - `exit`  - exits
//! These can be changed with the [`repl.set_clear_keyword()`](Repl::set_clear_keyword) and
//! [`repl.set_exit_keyword()`](Repl::set_exit_keyword) respectively. Any other special commands can
//! be handled within the execution loop.
//!
//! ## How to use
//!
//! Take some program that just prints the input back:
//! ```
//! use shelp::{Repl, Color};
//! let repl = Repl::newd("> ", ". ", None);
//! let mut repl = repl.iter(Color::Green);
//!
//! // Now 'claer' clears the screen instead of 'clear'.
//! repl.set_clear_keyword("claer");
//!
//! // for command in repl {
//! //     // Other special commands can be handled here
//! //     if command == "my_special_command" {
//! //         println!("Special command triggered!");
//! //         continue;
//! //     }
//! //
//! //     <Do something>
//! // }
//! // NOTE the above is commented out for doc test reasons
//! ```
//! Here no [`LangInterface`] is specified, so the default is used.
//! A [`LangInterface`] can be specified by implementing the trait and passing it as the generic
//! type argument.
//!
//! ```
//! use std::io::{self, prelude::*};
//! use shelp::{Repl, Color, LangInterface, Result};
//! // You can use any library, but currently only crossterm is used in the library for terminal.
//! use crossterm::style::Colorize;
//!
//! struct MyLangInterface;
//! // We want to override the linting so numbers are coloured, but we don't have a specific way of
//! // getting the indentation, so we do not override that.
//! impl LangInterface for MyLangInterface {
//!     fn print_line(_: &mut io::Stdout, line: &str) -> Result<()> {
//!         for i in line.chars() {
//!             if i.is_numeric() {
//!                 print!("{}", i.magenta());
//!             } else {
//!                 print!("{}", i);
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//!
//! // Use a particular capacity
//! let mut repl = Repl::<MyLangInterface>::with_capacity("> ", ". ", 128, None);
//!
//! // loop {
//! //     // You can have dynamic colours if you don't use the iterator. It also allows you to use the
//! //     // errors instead of them being ignored.
//! //     // NOTE here it is unwrapped, but it should be dealt with in a better way.
//! //     let command = repl.next(Color::Blue).unwrap();
//! //
//! //     <Do something>
//! // }
//! // NOTE the above is commented out for doc test reasons
//! ```

#[macro_use]
mod macros;
pub(crate) mod lang;
mod repl;

pub use crossterm::{style::Color, Result};
pub use lang::LangInterface;
pub use repl::iter::ReplIter;
pub use repl::Repl;
