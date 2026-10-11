//! The game's reader for its `DATA/CONFIG/*.INF` files (board row E5b): `IniReader_*` 0x221668..0x222040, exactly:
//! the same cursor moves, the same token buffer, the same answers, so a loader written on it reads a file as the
//! game does. Checked against the game's own functions in the function runner on every config file on the disc
//! (`tricky-game/tests/ini.rs`).
//!
//! The files are text: `#` or `;` start a comment to the end of the line; `[NAME]` starts a section; inside one,
//! `KEY = "string"`, `KEY = 12`, `KEY = 1.5`. The reader is a cursor over the file and one token buffer (the last
//! token, section name or quoted string read). Quirk kept: after finding a section, `skip_to_next_section` also
//! skips any sections that are empty, so their keys are read as the found one's.

/// `cIniReader` (+0 size, +4 the text, +8 the cursor, +0xc the token buffer).
#[derive(Clone, Debug)]
pub struct IniReader {
    /// the text and a NUL after it (`IniReader_Open` 0x221668 loads size + 1 bytes and puts a 0 at the end)
    text: Vec<u8>,
    size: usize,
    cur: usize,
    token: Vec<u8>,
}

/// `stricmp`: ASCII case-insensitive equality.
fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// `sscanf("%d")`: optional spaces and sign, then digits; `None` when there are none.
fn scan_int(s: &[u8]) -> Option<i32> {
    let t = std::str::from_utf8(s).ok()?.trim_start();
    let mut end = 0;
    let b = t.as_bytes();
    if end < b.len() && (b[end] == b'-' || b[end] == b'+') {
        end += 1;
    }
    let digits = b[end..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let v: i64 = t[..end + digits].parse().ok()?;
    Some(v as i32)
}

/// `sscanf("%f")`: the longest leading decimal number (sign, digits, point, exponent).
fn scan_float(s: &[u8]) -> Option<f32> {
    let t = std::str::from_utf8(s).ok()?.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let int = b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    i += int;
    let mut frac = 0;
    if i < b.len() && b[i] == b'.' {
        frac = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        i += 1 + frac;
    }
    if int + frac == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            j += 1;
        }
        let e = b[j..].iter().take_while(|c| c.is_ascii_digit()).count();
        if e > 0 {
            i = j + e;
        }
    }
    t[..i].parse().ok()
}

impl IniReader {
    /// `IniReader_Open` 0x221668 on a file's bytes.
    pub fn new(bytes: &[u8]) -> IniReader {
        let mut text = bytes.to_vec();
        text.push(0);
        IniReader { size: bytes.len(), text, cur: 0, token: Vec::new() }
    }

    /// The character at the cursor (0 at and past the end).
    fn peek(&self) -> u8 {
        self.text.get(self.cur).copied().unwrap_or(0)
    }

    /// `IniReader_GetChar` 0x2217d8
    fn get_char(&mut self) -> u8 {
        let c = self.peek();
        self.cur += 1;
        c
    }

    /// The cursor, from the start of the text.
    pub fn pos(&self) -> usize {
        self.cur
    }

    /// The token buffer: the last token, section name or quoted string read.
    pub fn token(&self) -> &[u8] {
        &self.token
    }

    /// `IniReader_SkipLine` 0x221988: to the end of the line (not past it); true at the end of the text.
    fn skip_line(&mut self) -> bool {
        loop {
            let c = self.peek();
            if c == 0 {
                return true;
            }
            if c == b'\r' || c == b'\n' {
                return false;
            }
            self.get_char();
        }
    }

    /// `IniReader_SkipComment` 0x2218c0: true at the end of the text (or a comment that runs to it).
    fn skip_comment(&mut self) -> bool {
        match self.peek() {
            0 => true,
            b'#' | b';' => self.skip_line(),
            _ => false,
        }
    }

    /// `IniReader_SkipSpace` 0x2217f0: past spaces, tabs, line ends and comments; true at the end of the text or a
    /// section header.
    pub fn skip_space(&mut self) -> bool {
        loop {
            if self.peek() == 0 {
                return true;
            }
            if self.skip_comment() {
                return true;
            }
            let c = self.peek();
            if !matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
                return c == b'[';
            }
            self.get_char();
        }
    }

    /// `IniReader_Expect` 0x221de8: past spaces, then the characters of `s` one by one; false (the game's 0) when all
    /// matched, true when one did not (it stops after the mismatch) or the text ended first.
    pub fn expect(&mut self, s: &[u8]) -> bool {
        self.skip_space();
        let mut i = 0;
        if self.peek() == 0 {
            return s.first().is_some_and(|c| *c != 0);
        }
        while i < s.len() {
            let c = self.get_char();
            if c != s[i] {
                return true;
            }
            i += 1;
            if self.peek() == 0 {
                return i < s.len();
            }
        }
        false
    }

    /// `IniReader_NextSection` 0x221a00: on to the next `[` (skipping comments), its name into the token buffer.
    /// `None` at the end of the text or when the `]` is not where it should be.
    pub fn next_section(&mut self) -> Option<Vec<u8>> {
        while self.peek() != 0 {
            self.skip_comment();
            if self.get_char() == b'[' {
                let mut name = Vec::new();
                while self.peek() != b']' && self.cur < self.text.len() {
                    name.push(self.get_char());
                }
                self.token = name;
                if self.expect(b"]") {
                    return None;
                }
                return Some(self.token.clone());
            }
        }
        None
    }

    /// `IniReader_FindSection` 0x221af0: on to the section named `name` (any case).
    pub fn find_section(&mut self, name: &[u8]) -> bool {
        while self.peek() != 0 {
            match self.next_section() {
                None => return false,
                Some(s) if eq_ignore_case(&s, name) => return true,
                _ => {}
            }
        }
        false
    }

    /// `IniReader_FindSectionMatching` 0x221b60: on to the first section whose name starts `s`, or that `s` starts
    /// with (case matters: `strstr`).
    pub fn find_section_matching(&mut self, s: &[u8]) -> bool {
        while self.peek() != 0 {
            match self.next_section() {
                None => return false,
                Some(name) => {
                    if s.starts_with(&name) || name.starts_with(s) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `IniReader_SkipToNextSection` 0x221918: past spaces and any section headers that follow at once.
    pub fn skip_to_next_section(&mut self) {
        self.skip_space();
        while self.peek() == b'[' {
            self.next_section();
            self.skip_space();
        }
        self.at_section_end();
    }

    /// `IniReader_AtSectionEnd` 0x221e90: true at the end of the text or the next section's header.
    pub fn at_section_end(&mut self) -> bool {
        if self.cur < self.size {
            self.skip_space();
            let c = self.peek();
            c == 0 || c == b'['
        } else {
            true
        }
    }

    /// `IniReader_ReadToken` 0x221c90: past spaces, then a word (to a space, tab, line end or the end) into the
    /// token buffer.
    pub fn read_token(&mut self) -> Vec<u8> {
        self.skip_space();
        let mut t = Vec::new();
        while !matches!(self.peek(), 0 | b' ' | b'\t' | b'\n' | b'\r') {
            t.push(self.get_char());
        }
        self.token = t;
        self.token.clone()
    }

    /// `IniReader_ReadQuoted` 0x221be8: a `"…"` string into the token buffer (the quotes expected, not checked).
    fn read_quoted(&mut self) -> Vec<u8> {
        self.skip_space();
        self.expect(b"\"");
        let mut t = Vec::new();
        while self.peek() != b'"' && self.cur < self.text.len() {
            t.push(self.get_char());
        }
        self.expect(b"\"");
        self.token = t;
        self.token.clone()
    }

    /// `IniReader_ReadString` 0x221ef8: when the last token is `key` (any case), `= "…"` and the string.
    pub fn read_string(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        if !eq_ignore_case(&self.token, key) || self.expect(b"=") {
            return None;
        }
        Some(self.read_quoted())
    }

    /// `IniReader_ReadInt` 0x221fd8: when the last token is `key`, `= n` (`sscanf` "%d" of the next word; `None`
    /// inside when it has no number, where the game leaves its stack word).
    pub fn read_int(&mut self, key: &[u8]) -> Option<Option<i32>> {
        if !eq_ignore_case(&self.token, key) || self.expect(b"=") {
            return None;
        }
        self.skip_space();
        let t = self.read_token();
        Some(scan_int(&t))
    }

    /// `IniReader_ReadFloat` 0x221f70: as [`IniReader::read_int`] with "%f".
    pub fn read_float(&mut self, key: &[u8]) -> Option<Option<f32>> {
        if !eq_ignore_case(&self.token, key) || self.expect(b"=") {
            return None;
        }
        self.skip_space();
        let t = self.read_token();
        Some(scan_float(&t))
    }
}
