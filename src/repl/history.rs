use std::cell::Cell;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, prelude::*};
use std::path::PathBuf;

/// Maintains REPL history of previously executed commands
///
/// NOTE: The commands need not have executed successfully.
pub struct History {
    /// The underlying buffer of history.
    /// Each command is stored as [Vec<String>] where each String refers to a line.
    ///
    /// The list of all commands is stored in a [VecDeque] since it must be allowed to insert and
    /// pop efficiently in **opposite** directions. It stores the history in reverse, since index 0
    /// is meant to be the previously executed command and index 1 the one before that and so on.
    /// So it must be efficient to push commands to the front of the buffer without recopying
    /// everything.
    buffer: VecDeque<Vec<String>>,
    /// An index for the current position in history for ease of use.
    ///
    /// The `next()`, `prev()` and `cur()` functions operate on this index.
    /// It is wrapped in `Cell` for interior mutability, so that it can be modified using only a
    /// shared reference.
    /// There is a need for a state where no history is in currently being used. For that state, -1
    /// is used.
    iter_i: Cell<isize>,
    /// File to persist the history
    path: Option<PathBuf>,
}

impl History {
    pub fn with_capacity(capacity: usize, path: Option<PathBuf>) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity + 1),
            iter_i: Cell::new(-1),
            path,
        }
    }

    fn at_capacity(&self) -> bool {
        self.buffer.len() == self.buffer.capacity()
    }

    pub fn push(&mut self, lines: Vec<String>) {
        // Make sure to not reallocate and keep within the capacity
        if self.at_capacity() {
            self.buffer.pop_back();
        }

        self.reset_iter();
        self.buffer.push_front(lines);
    }

    // Each command is separated by a '---'
    // So for example if there are 2 commands:
    // ```
    // let a = 2
    // ```
    // and
    // ```
    // if a < 2 {
    //     a += 4
    // }
    // ```
    //
    // The history file produced would be:
    // ```
    // let a = 2
    // ---
    // if a < 2 {
    //     a += 4
    // }
    // ---
    // ```
    /// Reads from history file and appends it to the current history buffer
    pub fn read_from_file(&mut self) -> io::Result<()> {
        let contents = fs::read_to_string(self.path.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Path to persisted file not found")
        })?)?;
        let mut lines = Vec::new();
        for line in contents.lines() {
            if line.starts_with("---") {
                self.push(lines);
                lines = Vec::new();
            } else {
                lines.push(line.to_owned());
            }
        }
        Ok(())
    }

    /// Writes to the history path
    pub fn write_to_file(&self) -> io::Result<()> {
        let mut f = fs::File::create(self.path.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Path to persisted file not found")
        })?)?;

        for lines in self.buffer.iter().rev() {
            for line in lines {
                f.write_all(line.as_bytes())?;
                f.write_all(b"\n")?;
            }
            f.write_all(b"---\n")?;
        }

        Ok(())
    }

    fn _len(&self) -> isize {
        self.buffer.len() as isize
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    fn _at(&self, index: isize) -> Option<&Vec<String>> {
        if index >= 0 {
            Some(&self.buffer[index as usize])
        } else {
            None
        }
    }

    pub fn cur(&self) -> Option<&Vec<String>> {
        if self.len() > 0 {
            self._at(self.iter_i.get())
        } else {
            None
        }
    }

    pub fn prev(&self) -> Option<&Vec<String>> {
        let iter_i = self.iter_i.get() + 1;

        if iter_i < self._len() {
            self.iter_i.set(iter_i);
            self._at(iter_i)
        } else {
            None
        }
    }

    pub fn next(&self) -> Option<&Vec<String>> {
        let iter_i = self.iter_i.get() - 1;

        // It is was already -1, so there definitely isn't a next to give.
        if iter_i >= -1 {
            self.iter_i.set(iter_i);
            self._at(iter_i)
        } else {
            None
        }
    }

    pub fn reset_iter(&self) {
        self.iter_i.set(-1);
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.reset_iter();
    }
}

impl std::ops::Index<usize> for History {
    type Output = Vec<String>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.buffer[index]
    }
}

impl Drop for History {
    fn drop(&mut self) {
        let _ = self.write_to_file();
    }
}
