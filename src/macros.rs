#[doc(hidden)]
#[macro_export]
macro_rules! history_up {
    ($self:ident, $stdout:ident, $c:ident, $lines:ident, $colour:ident) => {{
        $c.use_history = true;

        let lines = match $self.history.prev() {
            Some(s) => {
                $self.print_lines(&mut $stdout, &mut $c, &s, $colour)?;
                $c.lineno = s.len() - 1;
                if $c.lineno > 0 {
                    queue!($stdout, cursor::MoveDown($c.lineno as u16))?;
                }
                s
            }
            None => match $self.history.cur() {
                Some(s) => s,
                None => {
                    $c.use_history = false;
                    &$lines
                }
            },
        };

        let s_len = lines[$c.lineno].chars().count();

        if $c.charno == 0 || $c.charno > s_len {
            $c.charno = s_len;
        }
    }};

    (retain $self:ident, $stdout:ident, $c:ident, $lines:ident, $colour:ident) => {{
        let lineno = $c.lineno;
        if lineno > 0 {
            queue!($stdout, cursor::MoveUp(lineno as u16))?;
            $c.lineno = 0;
        }

        history_up!($self, $stdout, $c, $lines, $colour);
        if lineno < $c.lineno {
            queue!($stdout, cursor::MoveUp(($c.lineno - lineno) as u16))?;
            $c.lineno = lineno;
        };
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! history_down {
    ($self:ident, $stdout:ident, $c:ident, $lines:ident, $colour:ident) => {{
        let lines = match $self.history.next() {
            Some(s) => s,
            None => {
                $c.use_history = false;
                &$lines
            }
        };

        if $c.lineno > 0 {
            queue!($stdout, cursor::MoveUp($c.lineno as u16))?;
        }
        $c.lineno = 0;
        $self.print_lines(&mut $stdout, &mut $c, lines, $colour)?;

        let s_len = lines[$c.lineno].chars().count();

        if $c.charno == 0 || $c.charno > s_len {
            $c.charno = s_len;
        }
    }};

    (retain $self:ident, $stdout:ident, $c:ident, $lines:ident, $colour:ident) => {{
        let lineno = $c.lineno;
        history_down!($self, $stdout, $c, $lines, $colour);
        let lineno = min(lineno, $self.cur(&$c, &$lines).len() - 1);
        if lineno > 0 {
            queue!($stdout, cursor::MoveDown(lineno as u16))?;
            $c.lineno = lineno;
        };
    }};
}
