mod history;
pub(crate) mod iter;

use history::History;

use crate::lang::{DefaultLangInterface, LangInterface};
use crossterm::{cursor, event, execute, queue, style, terminal};
use std::cmp::min;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::path::PathBuf;

/// `Repl` interacts with the terminal to provide easy interactive shells.
///
/// Configuration:
/// - `leader`
///   What to print as the prompt:
///
///   ```no_lint
///   > <some-code>
///   ^^- leader
///   ```
/// - `continued_leader`
///   If the command is more than one line long, what to print on subsequent lines
///
///   ```no_lint
///   > <some-code>
///   . <some-code>
///   ```
/// - `path`
///   Path to a file to use as persistent history. If given, on construction, the history will be
///   populated from the contents of this file, and will automatically write it to the file on being
///   dropped. In case a path is not specified, the history is lost when `Repl` is dropped.
/// - `capacity`
///   The maximum amount of commands stored in the history. Default capacity is 64. If there are
///   already 64 commands in the history, the oldest one will be forgotten.
pub struct Repl<L: LangInterface = DefaultLangInterface> {
    /// The history of commands run.
    history: History,
    /// What to print as the prompt:
    ///
    /// > <some-code>
    /// ^^- leader
    leader: &'static str,
    /// The number of characters in the leader, it is stored here since getting number of characters
    /// is an `O(n)` operation for a utf-8 encoded string
    leader_len: usize,
    /// If the command is more than one line long, what to print on subsequent lines
    ///
    /// > <some-code>
    /// . <some-code>
    /// ^^- continued leader
    continued_leader: &'static str,
    /// The number of characters in the continued leader, it is stored here since getting number of
    /// characters is an `O(n)` operation for a utf-8 encoded string
    continued_leader_len: usize,
    _lang_interface: PhantomData<L>,
}

impl Repl<DefaultLangInterface> {
    /// Create a `Repl` with default language interface.
    pub fn newd(
        leader: &'static str,
        continued_leader: &'static str,
        path: Option<PathBuf>,
    ) -> Self {
        Self::with_capacity(leader, continued_leader, 64, path)
    }

    /// Create a `Repl` with default language interface, and specified history capacity.
    pub fn with_capacityd(
        leader: &'static str,
        continued_leader: &'static str,
        capacity: usize,
        path: Option<PathBuf>,
    ) -> Self {
        let should_persist = path.is_some();

        let mut repl = Self {
            history: History::with_capacity(capacity, path),
            leader,
            leader_len: leader.chars().count(),
            continued_leader,
            continued_leader_len: leader.chars().count(),
            _lang_interface: PhantomData,
        };

        if should_persist {
            let _ = repl.history.read_from_file();
        }

        repl
    }
}

impl<L: LangInterface> Repl<L> {
    /// Create a `Repl` with specified language interface.
    pub fn new(
        leader: &'static str,
        continued_leader: &'static str,
        path: Option<PathBuf>,
    ) -> Self {
        Self::with_capacity(leader, continued_leader, 64, path)
    }

    /// Create a `Repl` with specified language interface, and specified history capacity.
    pub fn with_capacity(
        leader: &'static str,
        continued_leader: &'static str,
        capacity: usize,
        path: Option<PathBuf>,
    ) -> Self {
        let should_persist = path.is_some();

        let mut repl = Self {
            history: History::with_capacity(capacity, path),
            leader,
            leader_len: leader.chars().count(),
            continued_leader,
            continued_leader_len: leader.chars().count(),
            _lang_interface: PhantomData,
        };

        if should_persist {
            let _ = repl.history.read_from_file();
        }

        repl
    }

    /// Gives current command based on the cursor
    fn cur<'a>(&'a self, c: &Cursor, lines: &'a [String]) -> &'a [String] {
        if c.use_history {
            // unwrap because if use_history is enabled, there must be at least one element in
            // history
            self.history.cur().unwrap()
        } else {
            lines
        }
    }

    /// Easy access to the current line
    fn cur_str<'a>(&'a self, c: &Cursor, lines: &'a [String]) -> &'a str {
        &self.cur(c, lines)[c.lineno]
    }

    /// Copy the lines from history into the lines buffer
    fn replace_with_history(&self, lines: &mut Vec<String>) {
        let cur = self.history.cur().unwrap();
        lines.resize(cur.len(), String::new());

        for (i, string) in cur.iter().enumerate() {
            lines[i].clear();
            lines[i] += string;
        }
    }

    fn pre_exit(&self) {
        let _ = terminal::disable_raw_mode();
        println!();
        let _ = self.history.write_to_file();
    }

    fn exit(&self) -> ! {
        self.pre_exit();
        std::process::exit(0)
    }

    /// Print a command
    fn print_lines(
        &self,
        stdout: &mut std::io::Stdout,
        c: &mut Cursor,
        lines: &[String],
        colour: style::Color,
    ) -> crate::Result<()> {
        if c.lineno > 0 {
            queue!(stdout, cursor::MoveUp(c.lineno as u16))?;
        }

        queue!(
            stdout,
            terminal::Clear(terminal::ClearType::CurrentLine),
            terminal::Clear(terminal::ClearType::FromCursorDown),
        )?;
        let mut is_first = true;

        for line in lines {
            let leader = if is_first {
                is_first = false;
                self.leader
            } else {
                self.continued_leader
            };

            queue!(
                stdout,
                cursor::MoveToColumn(0),
                style::SetForegroundColor(colour),
                style::Print(leader),
            )?;
            L::print_line(stdout, line)?;
            queue!(stdout, style::Print("\n"))?;
        }

        let leader_len = if c.lineno == 0 {
            self.leader_len
        } else {
            self.continued_leader_len
        };

        c.charno = min(c.charno, lines[c.lineno].chars().count());

        execute!(
            stdout,
            cursor::MoveUp((lines.len() - c.lineno) as u16),
            cursor::MoveToColumn((leader_len + c.charno) as u16)
        )
    }

    /// The main function, gives the next command
    pub fn next(&mut self, colour: style::Color) -> crate::Result<String> {
        let mut stdout = std::io::stdout();
        let mut lines = Vec::new();
        lines.push(String::new());

        let mut c = Cursor::default();

        terminal::enable_raw_mode()?;

        execute!(
            stdout,
            style::SetForegroundColor(colour),
            style::Print(self.leader),
            style::ResetColor
        )?;

        loop {
            let s = match event::read()? {
                event::Event::Key(e) => match e.code {
                    event::KeyCode::Char(chr)
                        if e.modifiers.contains(event::KeyModifiers::CONTROL) && chr == 'c' =>
                    {
                        self.exit()
                    }
                    event::KeyCode::Char(chr)
                        if e.modifiers.contains(event::KeyModifiers::CONTROL) && chr == 'l' =>
                    {
                        let lineno = c.lineno;
                        c.lineno = 0;

                        queue!(
                            stdout,
                            terminal::Clear(terminal::ClearType::All),
                            cursor::MoveTo(0, 0)
                        )?;
                        let lines = self.cur(&c, &lines);
                        self.print_lines(&mut stdout, &mut c, lines, colour)?;
                        c.lineno = lineno;
                        c.charno = min(c.charno, lines[c.lineno].chars().count());

                        if c.lineno > 0 {
                            queue!(stdout, cursor::MoveDown(lineno as u16))?;
                        }
                        queue!(
                            stdout,
                            cursor::MoveToColumn((self.continued_leader_len + c.charno) as u16),
                        )?;

                        self.cur_str(&c, &lines)
                    }
                    event::KeyCode::Char(chr) => {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        let byte_i = get_byte_i(&lines[c.lineno], c.charno);

                        lines[c.lineno].insert(byte_i, chr);
                        c.charno += 1;

                        &lines[c.lineno]
                    }
                    event::KeyCode::Tab => {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        lines[c.lineno].insert_str(c.charno, "    ");
                        c.charno += 4;

                        &lines[c.lineno]
                    }

                    event::KeyCode::Home => {
                        c.charno = 0;
                        self.cur_str(&c, &lines)
                    }
                    event::KeyCode::End => {
                        let s = self.cur_str(&c, &lines);
                        c.charno = s.chars().count();
                        s
                    }
                    event::KeyCode::Left if c.charno > 0 => {
                        c.charno -= 1;
                        self.cur_str(&c, &lines)
                    }
                    event::KeyCode::Right => {
                        let s = self.cur_str(&c, &lines);

                        if c.charno < s.chars().count() {
                            c.charno += 1;
                        };

                        s
                    }

                    event::KeyCode::PageUp => history_up!(retain self, stdout, c, lines, colour),
                    // At the top of the current block, go to previous history block
                    event::KeyCode::Up if c.lineno == 0 => {
                        history_up!(self, stdout, c, lines, colour)
                    }
                    // In the middle of a block, go up one line
                    event::KeyCode::Up => {
                        c.lineno -= 1;
                        queue!(stdout, cursor::MoveUp(1))?;
                        let s = self.cur_str(&c, &lines);
                        c.charno = min(s.chars().count(), c.charno);
                        s
                    }

                    event::KeyCode::PageDown => {
                        history_down!(retain self, stdout, c, lines, colour)
                    }
                    // At the bottom of the block, and in history. This means that there are more
                    // blocks down, either further down the history or when history is over, the
                    // editable lines itself
                    event::KeyCode::Down
                        if c.use_history && (c.lineno + 1) == self.history.cur().unwrap().len() =>
                    {
                        history_down!(self, stdout, c, lines, colour)
                    }
                    // When in the end of editable lines, nothing should be done
                    event::KeyCode::Down if !c.use_history && (c.lineno + 1) == lines.len() => {
                        &lines[c.lineno]
                    }
                    // Somewhere in the block, go to next line
                    event::KeyCode::Down => {
                        c.lineno += 1;
                        queue!(stdout, cursor::MoveDown(1))?;
                        let s = self.cur_str(&c, &lines);
                        c.charno = min(s.chars().count(), c.charno);
                        s
                    }

                    // Regular case, just need to delete a character
                    event::KeyCode::Backspace if c.charno > 0 => {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        c.charno -= 1;
                        let byte_i = get_byte_i(&lines[c.lineno], c.charno);
                        lines[c.lineno].remove(byte_i);
                        &lines[c.lineno]
                    }
                    // It is the last character, and it is not the last line
                    event::KeyCode::Backspace if c.lineno > 0 => {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        c.lineno -= 1;
                        c.charno = lines[c.lineno].chars().count();
                        let line = lines.remove(c.lineno + 1);
                        lines[c.lineno] += &line;

                        execute!(stdout, cursor::MoveUp(1))?;
                        self.print_lines(&mut stdout, &mut c, &lines, colour)?;

                        &lines[c.lineno]
                    }

                    // Regular delete, just need to delete one character
                    event::KeyCode::Delete
                        if c.charno < self.cur_str(&c, &lines).chars().count() =>
                    {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        let byte_i = get_byte_i(&lines[c.lineno], c.charno);
                        lines[c.lineno].remove(byte_i);
                        &lines[c.lineno]
                    }
                    event::KeyCode::Delete if (c.lineno + 1) < self.cur(&c, &lines).len() => {
                        if c.use_history {
                            self.replace_with_history(&mut lines);
                            c.use_history = false;
                        };

                        let line = lines.remove(c.lineno + 1);
                        lines[c.lineno] += &line;

                        self.print_lines(&mut stdout, &mut c, &lines, colour)?;

                        &lines[c.lineno]
                    }

                    event::KeyCode::Enter => {
                        let is_command = if !c.use_history && lines.len() == 1 {
                            match &lines[0] as &str {
                                "exit" => {
                                    self.exit();
                                }
                                "clear" => {
                                    c.charno = 0;
                                    lines[0].clear();

                                    queue!(
                                        stdout,
                                        terminal::Clear(terminal::ClearType::All),
                                        cursor::MoveTo(0, 0),
                                    )?;

                                    true
                                }
                                _ => false,
                            }
                        } else {
                            false
                        };

                        if is_command {
                            c.charno = 0;
                            lines[0].clear();
                            execute!(
                                stdout,
                                style::SetForegroundColor(colour),
                                style::Print(self.leader),
                                style::ResetColor,
                            )?;

                            continue;
                        }

                        if c.use_history && (c.lineno + 1) == self.history.cur().unwrap().len() {
                            break;
                        }
                        let indent = L::get_indent(&self.cur(&c, &lines)[0..(c.lineno + 1)]);

                        if !c.use_history && (c.lineno + 1) == lines.len() && indent == 0 {
                            break;
                        } else {
                            if c.use_history {
                                self.replace_with_history(&mut lines);
                                c.use_history = false;
                            }

                            c.lineno += 1;
                            c.charno = (indent * 4) as usize;
                            lines.insert(c.lineno, " ".repeat(c.charno));
                            execute!(stdout, style::Print("\n"))?;
                            self.print_lines(&mut stdout, &mut c, &lines, colour)?;
                            ""
                        }
                    }
                    _ => self.cur_str(&c, &lines),
                },
                _ => self.cur_str(&c, &lines),
            };

            queue!(
                stdout,
                terminal::Clear(terminal::ClearType::CurrentLine),
                cursor::MoveToColumn(0),
                style::SetForegroundColor(colour),
            )?;

            let (leader, leader_len) = if c.lineno == 0 {
                (self.leader, self.leader_len)
            } else {
                (self.continued_leader, self.continued_leader_len)
            };

            queue!(stdout, style::Print(leader))?;
            L::print_line(&mut stdout, s)?;
            execute!(
                stdout,
                cursor::MoveToColumn((leader_len + c.charno + 1) as u16)
            )?;
        }

        terminal::disable_raw_mode()?;
        println!();

        let src = self.cur(&c, &lines).join("\n");

        if src.trim().len() > 0 {
            if c.use_history {
                self.history.push(self.history.cur().unwrap().clone());
            } else {
                self.history.push(lines);
            }
        }

        Ok(src)
    }
}

impl<L: LangInterface> Drop for Repl<L> {
    fn drop(&mut self) {
        self.pre_exit();
    }
}

fn get_byte_i(string: &str, i: usize) -> usize {
    string
        .char_indices()
        .nth(i)
        .map(|c| c.0)
        .unwrap_or_else(|| string.len())
}

#[derive(Debug, Default)]
struct Cursor {
    use_history: bool,
    lineno: usize,
    charno: usize,
}
