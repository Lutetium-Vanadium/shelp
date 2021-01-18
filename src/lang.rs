use std::io::{self, prelude::*};

/// `LangInterface` is a trait used by [Repl](crate::Repl) to provide dependent specific features.
///
/// Implement only the functions which you want, since there are default implementations for all of
/// them.
pub trait LangInterface {
    /// Given the lines of text, this function should print the line at index. This should be
    /// overridden if you want to lint the output in the repl.
    ///
    /// ## Why are all lines given, not just what needs to be printed?
    ///
    /// Earlier, only the line that needed to be printed was given, but this caused issues while
    /// linting multi-line programs which had specific things over multiple lines. For example take
    /// the following lines:
    /// ```ignore
    /// <some-code> /* Multi
    ///     line
    /// Comment */
    /// ```
    /// This probably would not be linted properly, since in line 2, the context that the word 'line'
    /// is within a comment is lost. For correct behaviour (subject to language linting rules), all
    /// the lines should be processed, but only `lines[index]` should be written to `stdout`.
    fn print_line(stdout: &mut io::Stdout, lines: &[String], index: usize) -> crate::Result<()> {
        stdout
            .write_all(lines[index].as_bytes())
            .map_err(crossterm::ErrorKind::IoError)
    }

    /// Given the lines up to the place a new line is being added, this function should give the
    /// indentation of the new line.
    ///
    /// For example, take the following \[rust\] code (note `█` represents the cursor):
    /// ```ignore
    /// if true { █ // <------------- PRESSED ENTER HERE
    /// }
    /// ```
    /// Here the first two lines will be give: `['let a = 213;', 'if true {']`, and the expected
    /// indent would be `4` such that the code would become:
    /// ```ignore
    /// if true {
    ///     █ // CURSOR HERE
    /// }
    /// ```
    ///
    /// This is also used to detect if a new line should be added, or whether the command should be
    /// returned for execution.
    /// Take the following example:
    /// ```ignore
    /// if true { █ // <------------- PRESSED ENTER HERE
    /// ```
    /// The indentation here would be `4` since its the start of the new block. So a new line will
    /// be added instead of returning all the lines for processing.
    /// ```ignore
    /// if true {
    ///     █ // CURSOR HERE
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
