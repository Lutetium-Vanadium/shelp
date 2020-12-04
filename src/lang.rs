use std::io::{self, prelude::*};

/// `LangInterface` is a trait used by [Repl](crate::Repl) to provide dependent specific features.
///
/// Implement only the functions which you want, since there are default implementations for all of
/// them.
pub trait LangInterface {
    /// Given a line of text, this function should print the line. This should be overridden if you
    /// want to lint the output in the repl.
    fn print_line(stdout: &mut io::Stdout, line: &str) -> crate::Result<()> {
        stdout
            .write_all(line.as_bytes())
            .map_err(|e| crossterm::ErrorKind::IoError(e))
    }

    /// Given the lines up to the place a new line is being added, this function should give the
    /// indentation of the new line.
    ///
    /// For example, take the following \[rust\] code:
    /// ```
    /// if true { // <------------- PRESSED ENTER HERE
    /// }
    /// ```
    /// Here the first two lines will be give: `['let a = 213;', 'if true {']`, and the expected
    /// indent would be `4` such that the code would become:
    /// ```
    /// if true {
    ///     // CURSOR HERE
    /// }
    /// ```
    ///
    /// This is also used to detect if a new line should be added, or whether the command should be
    /// returned for execution.
    /// Take the following example:
    /// ```unclosed_delimiter
    /// if true { // <------------- PRESSED ENTER HERE
    /// ```
    /// The indentation here would be `4` since its the start of the new block. So a new line will
    /// be added instead of returning all the lines for processing.
    /// ```unclosed_delimiter
    /// if true {
    ///     // CURSOR HERE
    /// ```
    fn get_indent(lines: &[String]) -> usize {
        if let Some(ref line) = lines.last() {
            line.len() - line.trim_start().len()
        } else {
            0
        }
    }
}

pub struct DefaultLangInterface;

impl LangInterface for DefaultLangInterface {}
