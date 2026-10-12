//! The rest of the input (board row E10b): the disc's button maps read as the game reads them
//! (`Pad_LoadButtonMap` 0x179a88 `data/config/btnmap%d.dat`, `Pad_ParseButtonMap` 0x179b18 with `Parse_FindSection`
//! 0x179d98, `Parse_SkipSpace` 0x17a148, `Parse_SkipLine` 0x17a208, the section end `FUN_0017a1a8`, newlib's
//! `sscanf` on the game's pattern, `Pad_ParseButtonNames` 0x179e60 and the pair parsers) and bound to a rider's
//! mapper as `Pad_SetupVibration` 0x152770 does; the front end's key repeat (`PadDevice_UpdateRepeat` 0x17a958);
//! the pad's rumble (`cPS2Device` vf3 / vf4 / vf5 and the motor timers at the end of its poll, vf9 0x17b270).
//! Checked in the function runner (`tests/pad.rs`).

use crate::controls::ButtonMap;

/// `Pad_ParseButtonNames`' table (0x337a90): names in the order they are tried, and their bits.
pub const BUTTON_NAMES: [(&str, u32); 47] = [
    ("NONE", 0xffffffff),
    ("ALL", 0x0),
    ("DPAD_UP", 0x1),
    ("DPAD_DOWN", 0x2),
    ("DPAD_RIGHT", 0x4),
    ("DPAD_LEFT", 0x8),
    ("LJOY_UP", 0x20000),
    ("LJOY_DOWN", 0x40000),
    ("LJOY_RIGHT", 0x80000),
    ("LJOY_LEFT", 0x100000),
    ("RJOY_UP", 0x200000),
    ("RJOY_DOWN", 0x400000),
    ("RJOY_RIGHT", 0x800000),
    ("RJOY_LEFT", 0x1000000),
    ("SQUARE", 0x10),
    ("TRIANGLE", 0x20),
    ("CIRCLE", 0x40),
    ("X", 0x800),
    ("LJOY", 0x4000),
    ("RJOY", 0x8000),
    ("START", 0x80),
    ("SELECT", 0x100),
    ("LSHIFT2", 0x1000),
    ("RSHIFT2", 0x2000),
    ("LSHIFT", 0x200),
    ("RSHIFT", 0x400),
    ("ANALOG_NULL", 0x0),
    ("ANALOG_JOY_L_X", 0x1),
    ("ANALOG_JOY_L_POS_X", 0x2),
    ("ANALOG_JOY_L_NEG_X", 0x4),
    ("ANALOG_JOY_L_Y", 0x8),
    ("ANALOG_JOY_L_POS_Y", 0x10),
    ("ANALOG_JOY_L_NEG_Y", 0x20),
    ("ANALOG_JOY_R_X", 0x40),
    ("ANALOG_JOY_R_POS_X", 0x80),
    ("ANALOG_JOY_R_NEG_X", 0x100),
    ("ANALOG_JOY_R_Y", 0x200),
    ("ANALOG_JOY_R_POS_Y", 0x400),
    ("ANALOG_JOY_R_NEG_Y", 0x800),
    ("ANALOG_PAD_X", 0x1000),
    ("ANALOG_PAD_POS_X", 0x2000),
    ("ANALOG_PAD_NEG_X", 0x4000),
    ("ANALOG_PAD_Y", 0x8000),
    ("ANALOG_PAD_POS_Y", 0x10000),
    ("ANALOG_PAD_NEG_Y", 0x20000),
    ("true", 0x1),
    ("false", 0x0),
];

/// The file the game reads for button map `n` (the rider's stats +0x6c)
pub fn button_map_path(n: u32) -> String {
    format!("data/config/btnmap{n}.dat")
}

/// A button map file as parsed (the parse context, 0x220 bytes on `Pad_SetupVibration`'s stack): +8 / +0xc /
/// +0x10 the counts, +0x14 the `IS_BOOL` pairs (pressed, not pressed), +0x84 the `IS_BOOLGRAB` pairs, +0xfc the
/// `IS_ANALOG` pairs (mask, negate).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PadConfig {
    pub bools: Vec<(u32, u32)>,
    pub grabs: Vec<(u32, u32)>,
    pub analog: Vec<(u32, u32)>,
}

/// The parse context (+0 the text, +4 what is left of it).
struct Ctx<'a> {
    text: &'a [u8],
    at: usize,
    left: i32,
}

impl Ctx<'_> {
    fn ch(&self, k: usize) -> u8 {
        *self.text.get(self.at + k).unwrap_or(&0)
    }

    /// `Parse_SkipSpace` 0x17a148: past characters below '!' (signed: bytes from 0x80 count as space) while any
    /// are left; true while some are.
    fn skip_space(&mut self) -> bool {
        let mut n = self.left;
        if n < 1 {
            return false;
        }
        let c = self.ch(0) as i8;
        if c != 0 && c < b'!' as i8 {
            loop {
                n -= 1;
                self.left = n;
                self.at += 1;
                let d = self.ch(0) as i8;
                if d == 0 {
                    break;
                }
                if (b' ' as i8) < d || n < 1 {
                    break;
                }
            }
        }
        0 < n
    }

    /// `Parse_SkipLine` 0x17a208: to the next newline and past it; true while characters are left.
    fn skip_line(&mut self) -> bool {
        let mut n = self.left;
        if n < 1 {
            return false;
        }
        if self.ch(0) != 0 {
            if self.ch(0) != b'\n' {
                loop {
                    n -= 1;
                    self.at += 1;
                    self.left = n;
                    if self.ch(0) == 0 {
                        return 0 < n;
                    }
                    if self.ch(0) == b'\n' || n < 1 {
                        break;
                    }
                }
            }
            if self.ch(0) != 0 {
                self.at += 1;
                self.left = n - 1;
                n -= 1;
            }
        }
        0 < n
    }

    /// `strncmp`(text here, s, len s) == 0
    fn starts(&self, s: &[u8]) -> bool {
        for (k, b) in s.iter().enumerate() {
            let c = self.ch(k);
            if c != *b {
                return false;
            }
            if c == 0 {
                return true;
            }
        }
        true
    }

    /// `FUN_0017a1a8`: the end of a section ("};", its line skipped) or of the text
    fn section_end(&mut self) -> bool {
        if 0 < self.left {
            if self.skip_space() {
                if !self.starts(b"};") {
                    return false;
                }
                self.skip_line();
            }
            return true;
        }
        false
    }

    /// `Parse_FindSection` 0x179d98: a line starting with the name whose next line starts with '{'; the text left
    /// after that line.
    fn find_section(&mut self, name: &[u8]) -> bool {
        loop {
            if !self.skip_space() {
                return false;
            }
            if self.starts(name) {
                if !self.skip_line() {
                    return false;
                }
                if !self.skip_space() {
                    return false;
                }
                if self.starts(b"{") {
                    return self.skip_line();
                }
            }
            if !self.skip_line() {
                return false;
            }
        }
    }
}

/// newlib's `sscanf`(text, "{%*[ \t]%*[a-zA-Z],%*[ \t](%[0-9a-zA-Z_| \t]),%*[ \t](%[0-9a-zA-Z_| \t])"): the two
/// button lists of an entry; the count of lists read (each `%[` needs a character at least).
pub fn scan_entry(text: &[u8]) -> (usize, Vec<u8>, Vec<u8>) {
    let mut p = 0usize;
    let at = |p: usize| *text.get(p).unwrap_or(&0);
    let lit = |p: &mut usize, c: u8| -> bool {
        if at(*p) == c && c != 0 {
            *p += 1;
            true
        } else {
            false
        }
    };
    let set = |p: &mut usize, ok: &dyn Fn(u8) -> bool| -> Option<Vec<u8>> {
        let s = *p;
        while at(*p) != 0 && ok(at(*p)) {
            *p += 1;
        }
        if *p == s {
            None
        } else {
            Some(text[s..*p].to_vec())
        }
    };
    let space = |c: u8| c == b' ' || c == b'\t';
    let alpha = |c: u8| c.is_ascii_alphabetic();
    let list = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'|' || c == b' ' || c == b'\t';
    let mut a = vec![];
    let mut b = vec![];
    if !lit(&mut p, b'{') || set(&mut p, &space).is_none() || set(&mut p, &alpha).is_none() || !lit(&mut p, b',') || set(&mut p, &space).is_none() || !lit(&mut p, b'(') {
        return (0, a, b);
    }
    match set(&mut p, &list) {
        Some(s) => a = s,
        None => return (0, a, b),
    }
    if !lit(&mut p, b')') || !lit(&mut p, b',') || set(&mut p, &space).is_none() || !lit(&mut p, b'(') {
        return (1, a, b);
    }
    match set(&mut p, &list) {
        Some(s) => b = s,
        None => return (1, a, b),
    }
    (2, a, b)
}

/// `Pad_ParseButtonNames` 0x179e60: the names in a list (separated by spaces, tabs and '|'), each the first in
/// the table it starts with that ends there, their bits or-ed; an unknown name ends the list.
pub fn parse_button_names(s: &[u8]) -> u32 {
    let mut bits = 0u32;
    let mut p = 0usize;
    let at = |p: usize| *s.get(p).unwrap_or(&0);
    if at(0) == 0 {
        return 0;
    }
    loop {
        let mut found = false;
        for (name, b) in BUTTON_NAMES.iter() {
            let n = name.len();
            let rest = &s[p.min(s.len())..];
            let len = rest.iter().position(|c| *c == 0).unwrap_or(rest.len());
            let matches = name.bytes().enumerate().all(|(k, c)| at(p + k) == c);
            if matches {
                let c = at(p + n.min(len));
                if c == 0 || c == b' ' || c == b'|' || c == b'\t' {
                    p += n;
                    bits |= b;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return bits;
        }
        loop {
            let c = at(p);
            if c == 0 {
                return bits;
            }
            if c != b' ' && c != b'|' && c != b'\t' {
                break;
            }
            p += 1;
        }
        if at(p) == 0 {
            return bits;
        }
    }
}

/// `Pad_ParseButtonMap` 0x179b18 on a button map file's text (`Pad_LoadButtonMap` hands it the whole file).
pub fn parse_button_map(text: &[u8]) -> PadConfig {
    let mut cfg = PadConfig::default();
    let mut c = Ctx { text, at: 0, left: text.len() as i32 };
    for (k, name) in [&b"IS_BOOL"[..], b"IS_BOOLGRAB", b"IS_ANALOG"].iter().enumerate() {
        let mut ok = c.find_section(name);
        while ok && !c.section_end() {
            if !c.skip_space() {
                if k == 2 {
                    return cfg;
                }
                break;
            }
            let (n, a, b) = scan_entry(&text[c.at.min(text.len())..]);
            if n == 2 {
                let (pa, pb) = (parse_button_names(&a), parse_button_names(&b));
                match k {
                    0 => cfg.bools.push((pa, pb)),
                    1 => cfg.grabs.push((pa, pb)),
                    _ => cfg.analog.push((pa, (pb != 0) as u32)),
                }
            }
            ok = c.skip_line();
        }
    }
    cfg
}

/// `Pad_SetupVibration` 0x152770's binding of a parsed map to a rider's mapper: the 15 grab combos to the rider's
/// trick slots (`slots`, as `ButtonMap::btnmap` takes them), the 14 buttons to actions 0x1b..0x28 (`Input_BindButton`:
/// pressed, and pressed or-ed into the looked-at set), the 16 axes (`Input_BindAxis`). (The map's "shown" buttons,
/// cInput +0x1cc, and the vibration object it then makes are not kept.)
pub fn bind(cfg: &PadConfig, slots: [u16; 15]) -> ButtonMap {
    let mut m = ButtonMap { pressed: [0; 0x29], looked_at: [0; 0x29], axes: [(0, false); 16] };
    let mut button = |a: usize, pressed: u32, other: u32| {
        m.pressed[a] = pressed;
        m.looked_at[a] = other | pressed;
    };
    for (k, s) in slots.iter().enumerate() {
        let (p, o) = cfg.grabs.get(k).copied().unwrap_or((0, 0));
        button(*s as usize, p, o);
    }
    for k in 0..14 {
        let (p, o) = cfg.bools.get(k).copied().unwrap_or((0, 0));
        button(0x1b + k, p, o);
    }
    for k in 0..16 {
        let (mask, neg) = cfg.analog.get(k).copied().unwrap_or((0, 0));
        m.axes[k] = (mask, neg != 0);
    }
    m
}

/// The front end's keys (0x3857c0): a button (or direction) and the key it gives, plain and with a shoulder held.
pub const KEYS: [(u32, u32, u32); 14] = [
    (0x100, 0x1b, 0x1b),
    (0x80, 0xd, 0xd),
    (0x10, 0x61, 0x41),
    (0x20, 0x62, 0x42),
    (0x40, 0x63, 0x43),
    (0x800, 0x64, 0x44),
    (0x100008, 0x4b00, 0x4700),
    (0x80004, 0x4d00, 0x4f00),
    (0x40002, 0x5000, 0x5100),
    (0x20001, 0x4800, 0x4900),
    (0x400, 0x72, 0x72),
    (0x2000, 0x52, 0x52),
    (0x200, 0x6c, 0x6c),
    (0x1000, 0x4c, 0x4c),
];

/// A pad's key repeat (`cPadDevice` +8 repeat on, +0x10 the buttons, +0x2c this frame's key, +0x30 the key held,
/// +0x34 the frames held).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRepeat {
    pub repeat: u32,
    pub key: u32,
    pub held: u32,
    pub frames: i32,
}

impl KeyRepeat {
    /// `PadDevice_UpdateRepeat` 0x17a958: the first of `KEYS` whose buttons are down gives the key (the shifted
    /// one while L1, R1, L2 or R2 is held); a new key is given at once and held (−7 frames); a key held gives
    /// itself again after more than 7 frames (then every 8) when repeat is on.
    pub fn update(&mut self, buttons: u32) {
        let mut k = 0;
        for (mask, plain, shifted) in KEYS {
            if buttons & mask != 0 {
                k = if buttons & 0x3600 == 0 { plain } else { shifted };
                break;
            }
        }
        self.key = 0;
        if k == 0 || self.held != k {
            self.key = k;
            self.frames = -7;
            self.held = k;
        } else {
            self.frames += 1;
            if 7 < self.frames && self.repeat != 0 {
                self.key = k;
                self.frames = 0;
            }
        }
    }
}

/// A pad's rumble (`cPS2Device` +0x48 allowed, +0x78 / +0x79 the small and large motors, +0x80 to send, +0x84 /
/// +0x88 their frames left, −1 for on until told).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rumble {
    pub allowed: u32,
    pub small: u8,
    pub large: u8,
    pub send: u32,
    pub small_frames: i32,
    pub large_frames: i32,
}

impl Default for Rumble {
    /// as `cPS2Device::cPS2Device` 0x17ac40 leaves it
    fn default() -> Self {
        Rumble { allowed: 1, small: 0, large: 0, send: 1, small_frames: -1, large_frames: -1 }
    }
}

impl Rumble {
    /// vf3 0x17b590: the small motor on or off, for `frames`
    pub fn set_small(&mut self, on: u8, frames: i32) {
        if self.allowed != 0 {
            self.small = on;
            self.send = 1;
            self.small_frames = frames;
        }
    }

    /// vf4 0x17b5b0: the large motor at `amount` (× 255, truncated) for `frames`
    pub fn set_large(&mut self, amount: f32, frames: i32) {
        if self.allowed != 0 {
            self.send = 1;
            self.large_frames = frames;
            self.large = (amount * 255.0) as i32 as u8;
        }
    }

    /// vf5 0x17b5e8: rumble allowed or not (both motors off on a change)
    pub fn allow(&mut self, on: u32) {
        if self.allowed != on {
            self.allowed = on;
            self.send = 1;
            self.small = 0;
            self.large = 0;
        }
    }

    /// The end of the poll (vf9 0x17b270): with the pad ready (state 10) each motor's frames count down to 0 and
    /// it stops there (sent); not ready, both off. `Some(motors)` when they are sent (`scePadSetActDirect`).
    pub fn tick(&mut self, ready: bool) -> Option<(u8, u8)> {
        let mut large;
        if ready {
            let s = self.small_frames;
            if s < 1 {
                if s == 0 {
                    self.small_frames = -1;
                    self.send = 1;
                    self.small = 0;
                }
            } else {
                self.small_frames = s - 1;
            }
            large = self.large_frames;
            if large < 1 {
                if large == 0 {
                    self.large_frames = -1;
                    self.send = 1;
                    self.large = 0;
                }
                return self.flush();
            }
            large -= 1;
        } else {
            self.large = 0;
            self.small_frames = -1;
            self.small = 0;
            large = -1;
        }
        self.large_frames = large;
        self.flush()
    }

    fn flush(&mut self) -> Option<(u8, u8)> {
        if self.send != 0 {
            self.send = 0;
            Some((self.small, self.large))
        } else {
            None
        }
    }
}
