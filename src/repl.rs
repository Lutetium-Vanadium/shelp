mod history;

use history::History;

use crate::lang::{DefaultLangInterface, LangInterface};
use crossterm::{cursor, event, event::EventStream, execute, queue, style, terminal};
use futures::StreamExt;
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
/// - `clear_keyword`
///   Clears the screen. See [`set_clear_keyword`](Repl::set_exit_keyword)
pub struct Repl<L: LangInterface = DefaultLangInterface> {
    /// The history of commands run.
    history: History,
    /// What to print as the prompt:
    ///
    /// > <some-code>
    /// ^^- leader
    leader: &'static str,

    attached: Option<usize>,
    /// The number of characters in the leader, it is stored here since getting number of characters
    /// is an `O(n)` operation for a utf-8 encoded string
    leader_len: usize,
    /// The name of the attached module.
    module: Option<&'static str>,
    /// The length of the attached module string name.
    module_len: usize,
    /// If the command is more than one line long, what to print on subsequent lines
    ///
    /// > <some-code>
    /// . <some-code>
    /// ^^- continued leader
    continued_leader: &'static str,
    /// The number of characters in the continued leader, it is stored here since getting number of
    /// characters is an `O(n)` operation for a utf-8 encoded string
    continued_leader_len: usize,
    /// The keyword which corresponds to the clear command (default is 'clear')
    clear_keyword: &'static str,
    _lang_interface: PhantomData<L>,
    /// The async event stream for the REPL.
    event_stream: EventStream,
    // Maintain future variables
    lines: Vec<String>,
    c: Cursor,
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
            attached: None,
            leader_len: leader.chars().count(),
            module: None,
            module_len: 0,
            continued_leader,
            continued_leader_len: leader.chars().count(),
            clear_keyword: "clear",
            _lang_interface: PhantomData,
            event_stream: EventStream::new(),
            lines: Vec::new(),
            c: Cursor::default(),
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
            attached: None,
            leader_len: leader.chars().count(),
            module: None,
            module_len: 0,
            continued_leader,
            continued_leader_len: leader.chars().count(),
            clear_keyword: "clear",
            _lang_interface: PhantomData,
            event_stream: EventStream::new(),
            lines: Vec::new(),
            c: Cursor::default(),
        };

        if should_persist {
            let _ = repl.history.read_from_file();
        }

        repl
    }

    /// Sets the clear keyword. If you don't want any clear keyword, set it to an empty string
    pub fn set_clear_keyword(&mut self, clear_keyword: &'static str) {
        self.clear_keyword = clear_keyword
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

    /// Inform the REPL we have selected a module.
    pub fn module(&mut self, module: &'static str) {
        self.module = Some(module);
        self.module_len = module.len();
    }

    /// Inform the REPL we have exited from a module's context.
    pub fn exit_module(&mut self) {
        self.module = None;
        self.module_len = 0;
    }

    /// Tell the REPL we have attached to a module
    pub fn attach(&mut self, id: usize) {
        self.attached = Some(id)
    }

    /// Tell the REPL we have detached from all modules.
    pub fn detach(&mut self) {
        self.attached = None
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

        self.history.reset_iter();
    }

    fn pre_exit(&self) {
        let _ = terminal::disable_raw_mode();
        println!();
        let _ = self.history.write_to_file();
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

        for index in 0..lines.len() {
            let leader = match (is_first, self.attached) {
                (true, Some(id)) => {
                    is_first = false;
                    format!("(Attached: {}) {}", id, self.leader)
                }
                (true, None) => {
                    is_first = false;
                    if let Some(name) = self.module {
                        // If there is a module prepend it
                        format!("{}{}", print_module_name(name), self.leader)
                    } else {
                        format!("{}", self.leader)
                    }
                }
                (false, _) => self.continued_leader.to_string(),
            };

            queue!(
                stdout,
                cursor::MoveToColumn(0),
                style::SetForegroundColor(colour),
                style::Print(leader),
            )?;
            L::print_line(stdout, lines, index)?;
            queue!(stdout, style::Print("\n"))?;
        }

        let leader_len = if c.lineno == 0 {
            let extra_len = if let Some(id) = self.attached {
                id.to_string().chars().count() + 13
            } else {
                if self.module.is_some() {
                    self.module_len + 3 // Add the brackets around the name and space
                } else {
                    0
                }
            };
            extra_len + self.leader_len
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

    /// Resets the cursor and the lines after input has been received
    fn reset_lines(&mut self) {
        self.lines = Vec::new();
        self.c = Cursor::default();
    }

    /// The main function, gives the next command
    pub async fn next(&mut self, colour: style::Color) -> crate::Result<String> {
        let mut stdout = std::io::stdout();
        let mut lines = self.lines.clone();
        let mut c = self.c.clone();

        let leader = if let Some(_id) = self.attached {
            "".to_string()
        } else {
            terminal::enable_raw_mode()?;
            if let Some(name) = self.module {
                format!("{}{}", print_module_name(name), self.leader)
            } else {
                self.leader.to_string()
            }
        };

        if lines.is_empty() {
            lines.push(String::new());

            execute!(
                stdout,
                style::SetForegroundColor(colour),
                style::Print(&leader),
                style::ResetColor
            )?;
        }

        loop {
            // Update the temporary variables
            self.lines = lines.clone();
            self.c = c.clone();

            match self.event_stream.next().await {
                Some(Ok(event::Event::Key(e))) => {
                    match e.code {
                        event::KeyCode::Char('c')
                            if e.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            terminal::disable_raw_mode()?;
                            self.reset_lines();
                            return Ok(String::from("exit"));
                        }
                        event::KeyCode::Char('d')
                            if e.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            terminal::disable_raw_mode()?;
                            println!();
                            // Empty line
                            self.reset_lines();
                            execute!(stdout, style::SetForegroundColor(colour))?;
                            return Ok(String::from("detach"));
                        }
                        event::KeyCode::Esc => {
                            // Escapes from a module
                            terminal::disable_raw_mode()?;
                            println!();
                            // Empty line
                            self.reset_lines();
                            execute!(stdout, style::SetForegroundColor(colour))?;
                            return Ok(String::from("back"));
                        }
                        event::KeyCode::Char('l')
                            if e.modifiers.contains(event::KeyModifiers::CONTROL) =>
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
                        }
                        event::KeyCode::Char(chr) => {
                            if c.use_history {
                                self.replace_with_history(&mut lines);
                                c.use_history = false;
                            };

                            let byte_i = get_byte_i(&lines[c.lineno], c.charno);

                            lines[c.lineno].insert(byte_i, chr);
                            c.charno += 1;
                        }
                        event::KeyCode::Tab => {
                            if c.use_history {
                                self.replace_with_history(&mut lines);
                                c.use_history = false;
                            };

                            lines[c.lineno].insert_str(c.charno, "    ");
                            c.charno += 4;
                        }

                        event::KeyCode::Home => {
                            c.charno = 0;
                        }
                        event::KeyCode::End => {
                            c.charno = self.cur_str(&c, &lines).chars().count();
                        }
                        event::KeyCode::Left if c.charno > 0 => {
                            c.charno -= 1;
                        }
                        event::KeyCode::Right => {
                            if c.charno < self.cur_str(&c, &lines).chars().count() {
                                c.charno += 1;
                            };
                        }

                        event::KeyCode::PageUp => {
                            history_up!(retain self, stdout, c, lines, colour)
                        }
                        // At the top of the current block, go to previous history block
                        event::KeyCode::Up if c.lineno == 0 => {
                            history_up!(self, stdout, c, lines, colour)
                        }
                        // In the middle of a block, go up one line
                        event::KeyCode::Up => {
                            c.lineno -= 1;
                            queue!(stdout, cursor::MoveUp(1))?;
                            c.charno = min(self.cur_str(&c, &lines).chars().count(), c.charno);
                        }

                        event::KeyCode::PageDown => {
                            history_down!(retain self, stdout, c, lines, colour)
                        }
                        // At the bottom of the block, and in history. This means that there are more
                        // blocks down, either further down the history or when history is over, the
                        // editable lines itself
                        event::KeyCode::Down
                            if c.use_history
                                && (c.lineno + 1) == self.history.cur().unwrap().len() =>
                        {
                            history_down!(self, stdout, c, lines, colour)
                        }
                        // When in the end of editable lines, nothing should be done
                        event::KeyCode::Down if !c.use_history && (c.lineno + 1) == lines.len() => {
                        }
                        // Somewhere in the block, go to next line
                        event::KeyCode::Down => {
                            c.lineno += 1;
                            queue!(stdout, cursor::MoveDown(1))?;
                            c.charno = min(self.cur_str(&c, &lines).chars().count(), c.charno);
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
                        }
                        event::KeyCode::Delete if (c.lineno + 1) < self.cur(&c, &lines).len() => {
                            if c.use_history {
                                self.replace_with_history(&mut lines);
                                c.use_history = false;
                            };

                            let line = lines.remove(c.lineno + 1);
                            lines[c.lineno] += &line;

                            self.print_lines(&mut stdout, &mut c, &lines, colour)?;
                        }

                        event::KeyCode::Enter => {
                            if self.attached.is_some() {
                                terminal::disable_raw_mode()?;
                                println!();
                                // Empty line
                                self.reset_lines();
                                execute!(stdout, style::SetForegroundColor(colour))?;
                                return Ok(String::from("detach"));
                            }

                            if self.cur(&c, &lines[..])[0].trim().is_empty() {
                                execute!(
                                    stdout,
                                    cursor::MoveToNextLine(1),
                                    style::SetForegroundColor(colour),
                                    style::Print(&leader)
                                )?;
                                // Empty line
                                self.reset_lines();
                                continue;
                            }

                            if !c.use_history && lines.len() == 1 {
                                if lines[0] == self.clear_keyword {
                                    c.charno = 0;
                                    lines[0].clear();

                                    execute!(
                                        stdout,
                                        terminal::Clear(terminal::ClearType::All),
                                        cursor::MoveTo(0, 0),
                                        style::SetForegroundColor(colour),
                                        style::Print(&leader),
                                        style::ResetColor,
                                    )?;

                                    // Command executed, no need to do any other checks
                                    continue;
                                }
                            }

                            if c.use_history && (c.lineno + 1) == self.history.cur().unwrap().len()
                            {
                                // On the last line, break out of loop to return code for execution
                                break;
                            }
                            let indent = L::get_indent(&self.cur(&c, &lines)[0..(c.lineno + 1)]);

                            if !c.use_history && (c.lineno + 1) == lines.len() && indent == 0 {
                                // On the last line, break out of loop to return code for execution
                                break;
                            } else {
                                if c.use_history {
                                    self.replace_with_history(&mut lines);
                                    c.use_history = false;
                                }

                                c.lineno += 1;
                                c.charno = indent;
                                lines.insert(c.lineno, " ".repeat(indent));
                                execute!(stdout, style::Print("\n"))?;
                                self.print_lines(&mut stdout, &mut c, &lines, colour)?;
                            }
                        }
                        _ => {}
                    }
                }
                Some(Ok(_)) => {} // ignore other events
                Some(Err(e)) => {
                    self.reset_lines();
                    return Err(e);
                }
                None => {
                    self.reset_lines();
                    return Err(crossterm::ErrorKind::IoError(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Events ended",
                    )));
                }
            }

            queue!(
                stdout,
                terminal::Clear(terminal::ClearType::CurrentLine),
                cursor::MoveToColumn(0),
                style::SetForegroundColor(colour),
            )?;

            let (leader, leader_len) = if c.lineno == 0 {
                let extra_len = if let Some(id) = self.attached {
                    id.to_string().chars().count() + 13
                } else {
                    if self.module.is_some() {
                        self.module_len + 3 // Add the brackets around the name and space
                    } else {
                        0
                    }
                };
                (leader.as_str(), extra_len + self.leader_len)
            } else {
                (self.continued_leader, self.continued_leader_len)
            };

            queue!(stdout, style::Print(leader))?;
            L::print_line(&mut stdout, self.cur(&c, &lines[..]), c.lineno)?;
            execute!(
                stdout,
                cursor::MoveToColumn((leader_len + c.charno + 1) as u16)
            )?;
        }

        terminal::disable_raw_mode()?;
        println!();

        let src = self.cur(&c, &lines).join("\n");

        if c.use_history {
            self.history.push(self.history.cur().unwrap().clone());
        } else {
            self.history.push(lines);
        }

        self.reset_lines();
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

/// The format for printing a modules name
fn print_module_name(name: &'static str) -> String {
    format!("({}) ", name)
}

#[derive(Debug, Default, Clone)]
struct Cursor {
    use_history: bool,
    lineno: usize,
    charno: usize,
}
