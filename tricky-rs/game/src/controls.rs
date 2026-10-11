//! The pad word the boarder states read (board row G1a5b1): `cPlayer::GetControls` 0x151250 exactly. Each boarder
//! state (rider+0x428) packs the pad mapper's answers into its own layout: six-bit sticks (`axis × 31`, truncated),
//! one-bit buttons and two-bit directions (−1, 0, 1). Checked bit for bit in the function runner
//! (`tests/controls.rs`). Which buttons and sticks the mapper's action ids and axes are is row G1a5b2.

/// The pad mapper (player+0x870): its axes (`Pad_GetAxis` 0x1797f8) and its four button queries, by the vtable
/// slot each goes through (0x179758 → +0xc, 0x179780 → +0x14, 0x1797a8 → +0x1c, 0x1797d0 → +0x24).
pub trait PadMapper {
    fn axis(&mut self, axis: u32) -> f32;
    fn q0c(&mut self, action: u32) -> u32;
    fn q14(&mut self, action: u32) -> u32;
    fn q1c(&mut self, action: u32) -> u32;
    fn q24(&mut self, action: u32) -> u32;
}

/// What `GetControls` reads besides the pad.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlsIn {
    /// the boarder's state (rider+0x428)
    pub state: u32,
    /// an overlay is up (`OverlayMan_IsAnyVisible`) and no race is running (no race object, or race+0x54 = 0):
    /// the pad is not read
    pub paused: bool,
}

/// A stick as six bits: `(int)(axis × 31) & 0x3f`.
fn six(a: f32) -> u32 {
    ((a * 31.0) as i32 as u32) & 0x3f
}

/// A stick as a direction: −1 below −0.5, 1 above 0.5, else 0 (NaN reads −1), two bits.
fn dir(a: f32) -> u32 {
    let v: i32 = if -0.5 <= a { if a <= 0.5 { 0 } else { 1 } } else { -1 };
    (v as u32) & 3
}

fn set(w: &mut u32, mask_bits: u32, shift: u32, v: u32) {
    *w = *w & !(mask_bits << shift) | (v & mask_bits) << shift;
}

/// The d-pad-or-stick two-bit field: action 0x26 reads 3 (−1), action 0x27 1, else the direction of axis 5.
fn dpad(p: &mut dyn PadMapper) -> u32 {
    if p.q0c(0x26) != 0 {
        3
    } else if p.q0c(0x27) != 0 {
        1
    } else {
        dir(p.axis(5))
    }
}

/// `cPlayer::GetControls` 0x151250: the pad word for the boarder's state. `trick` is the trick slot
/// (player+0x868, 0x1a = none) the spin state's AI-trick button fills from `random_easy_trick`
/// (`AIComputer_RandomEasyTrick` 0x11d3d8). Returns the word; `slope` is set when the state also calls
/// `Boarder_UpdateSlopeTimeScale` 0x153130 (states 2–5, 7, 0xb, 0xc, 0x12 and 0xf).
pub fn get_controls(c: ControlsIn, p: &mut dyn PadMapper, trick: &mut u32, random_easy_trick: &mut dyn FnMut() -> u32, slope: &mut bool) -> u32 {
    let mut w = 0u32;
    *slope = false;
    if c.paused {
        if c.state == 0x10 {
            w = 0x1a;
        }
        return w;
    }
    match c.state {
        2 | 3 | 4 | 5 | 7 | 0xb | 0xc | 0x12 => {
            *slope = true;
            set(&mut w, 0x3f, 5, six(p.axis(4)));
            set(&mut w, 1, 1, p.q1c(0x1b));
            set(&mut w, 1, 2, p.q0c(0x1b));
            set(&mut w, 1, 0, p.q1c(0x25));
            set(&mut w, 1, 3, p.q14(0x1d));
            set(&mut w, 1, 4, p.q24(0x1d));
            set(&mut w, 0x3f, 17, six(p.axis(0)));
            set(&mut w, 0x3f, 11, six(p.axis(1)));
            let d = dpad(p);
            set(&mut w, 3, 23, d);
            set(&mut w, 3, 25, dir(p.axis(6)));
            *trick = 0x1a;
        }
        8 | 9 | 0x11 => set(&mut w, 0x3f, 0, six(p.axis(0xf))),
        10 => {
            set(&mut w, 1, 1, p.q0c(0x1b));
            set(&mut w, 1, 0, p.q1c(0x25));
        }
        0xd => {
            set(&mut w, 1, 1, p.q1c(0x25));
            set(&mut w, 0x3f, 8, six(p.axis(4)));
            set(&mut w, 0x3f, 14, six(p.axis(0xd)));
            set(&mut w, 0x3f, 20, six(p.axis(0xe)));
            set(&mut w, 0x3f, 2, six(p.axis(1)));
            let d = if p.q0c(0x26) != 0 {
                3
            } else if p.q0c(0x27) != 0 {
                1
            } else {
                dir(p.axis(5))
            };
            set(&mut w, 3, 26, d);
            set(&mut w, 3, 28, dir(p.axis(6)));
            *trick = 0x1a;
            if p.q0c(0x24) != 0 {
                w |= 1;
            } else {
                // any trick button (0..0x19) held sets bit 0; every one is asked
                for b in 0..=0x19 {
                    if p.q0c(b) != 0 {
                        w |= 1;
                    }
                }
            }
        }
        0xe => {
            set(&mut w, 0x3f, 17, six(p.axis(0xc)));
            set(&mut w, 0x3f, 5, six(p.axis(0xd)));
            set(&mut w, 0x3f, 11, six(p.axis(0xe)));
            set(&mut w, 1, 0, p.q1c(0x25));
            set(&mut w, 1, 1, p.q0c(0x1b));
            set(&mut w, 1, 2, p.q0c(0x1c));
            set(&mut w, 1, 3, p.q14(0x1d));
            set(&mut w, 1, 4, p.q14(0x23));
        }
        0xf => {
            *slope = true;
            set(&mut w, 0x3f, 5, six(p.axis(4)));
            set(&mut w, 0x3f, 11, six(p.axis(1)));
            set(&mut w, 0x3f, 17, six(p.axis(7)));
            set(&mut w, 1, 1, p.q1c(0x1b));
            set(&mut w, 1, 2, p.q0c(0x1b));
            set(&mut w, 1, 0, p.q1c(0x25));
            set(&mut w, 1, 3, p.q14(0x22));
            set(&mut w, 1, 4, p.q14(0x1d));
        }
        0x10 => {
            set(&mut w, 1, 8, p.q1c(0x25));
            set(&mut w, 1, 9, p.q14(0x20));
            set(&mut w, 1, 10, p.q14(0x21));
            for (shift, axis) in [(12, 2), (14, 3), (16, 8), (18, 9), (20, 10), (22, 11)] {
                set(&mut w, 3, shift, dir(p.axis(axis)));
            }
            w = w & !0xff | 0x1a;
            set(&mut w, 1, 11, p.q0c(0x24));
            let d = if p.q0c(0x26) != 0 {
                3
            } else if p.q0c(0x27) != 0 {
                1
            } else {
                dir(p.axis(5))
            };
            set(&mut w, 3, 24, d);
            let before = w;
            set(&mut w, 3, 26, dir(p.axis(6)));
            if before & 0x800 == 0 {
                // the grab buttons: the last of 0..0x19 held goes in the low byte
                for b in 0..=0x19 {
                    if p.q0c(b) != 0 {
                        w = w & !0xff | b;
                    }
                }
            }
        }
        0x13 => {
            set(&mut w, 0x3f, 1, six(p.axis(10)));
            set(&mut w, 0x3f, 7, six(p.axis(11)));
            set(&mut w, 1, 0, p.q1c(0x25));
        }
        0x14 => set(&mut w, 1, 0, p.q1c(0x25)),
        0x15 => set(&mut w, 1, 0, p.q24(0x28)),
        _ => {}
    }
    if c.state == 0x10 {
        if w & 0x800 == 0 {
            *trick = 0x1a;
        } else {
            if *trick == 0x1a {
                *trick = random_easy_trick();
            }
            w = w & !0xff | (*trick & 0xff);
        }
        w &= !0x800;
    }
    w
}

// ---- the pad mapper (`cInput`, board row G1a5b2)

/// The game's button bits (`Pad_ParseButtonNames`' table at 0x337a90), as the pad device sets them.
pub const DPAD_UP: u32 = 0x1;
pub const DPAD_DOWN: u32 = 0x2;
pub const DPAD_RIGHT: u32 = 0x4;
pub const DPAD_LEFT: u32 = 0x8;
pub const SQUARE: u32 = 0x10;
pub const TRIANGLE: u32 = 0x20;
pub const CIRCLE: u32 = 0x40;
pub const START: u32 = 0x80;
pub const SELECT: u32 = 0x100;
pub const LSHIFT: u32 = 0x200;
pub const RSHIFT: u32 = 0x400;
pub const X: u32 = 0x800;
pub const LSHIFT2: u32 = 0x1000;
pub const RSHIFT2: u32 = 0x2000;
pub const LJOY: u32 = 0x4000;
pub const RJOY: u32 = 0x8000;
pub const LJOY_UP: u32 = 0x20000;
pub const LJOY_DOWN: u32 = 0x40000;
pub const LJOY_RIGHT: u32 = 0x80000;
pub const LJOY_LEFT: u32 = 0x100000;
pub const RJOY_UP: u32 = 0x200000;
pub const RJOY_DOWN: u32 = 0x400000;
pub const RJOY_RIGHT: u32 = 0x800000;
pub const RJOY_LEFT: u32 = 0x1000000;
/// `NONE` in a button map: never.
pub const NONE: u32 = 0xffff_ffff;
const SHIFTS: u32 = LSHIFT | RSHIFT | LSHIFT2 | RSHIFT2;

/// The libpad button bits (pressed = 1) each game bit 0..15 is read from (0x337c08).
pub const PS2_BITS: [u32; 16] = [0x1000, 0x4000, 0x2000, 0x8000, 0x80, 0x10, 0x20, 0x800, 0x100, 0x4, 0x8, 0x40, 0x1, 0x2, 0x200, 0x400];

/// The pad as the game keeps it (the device the mapper reads): this tick's and last tick's button bits, the d-pad
/// as an axis pair (+0x14, +0x18: left and up are -1) and the sticks (+0x1c..+0x28: (byte - 127.5) / 127.5, right
/// and down positive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadDevice {
    pub prev: u32,
    pub buttons: u32,
    pub dpad: [f32; 2],
    pub left: [f32; 2],
    pub right: [f32; 2],
}

impl PadDevice {
    /// `cPS2Device` vf10 0x17b278 for a connected pad: last tick's buttons kept, this tick's from the libpad word
    /// `ps2` (pressed = 1), the sticks from their bytes (`analog`: left x, y, right x, y; `None` for a digital pad,
    /// sticks 0), the d-pad as +-1 and the sticks past half as their direction bits. (The key repeat after it,
    /// `PadDevice_UpdateRepeat` 0x17a958, is not ported.)
    pub fn read(&mut self, ps2: u16, analog: Option<[u8; 4]>) {
        self.prev = self.buttons;
        let mut b = 0;
        for (i, m) in PS2_BITS.iter().enumerate() {
            if ps2 as u32 & m != 0 {
                b |= 1 << i;
            }
        }
        match analog {
            Some([lx, ly, rx, ry]) => {
                let n = |v: u8| (v as i32 as f32 - 127.5) / 127.5;
                self.left = [n(lx), n(ly)];
                self.right = [n(rx), n(ry)];
                self.dpad = dpad_axes(b);
                if self.left[0] < -0.5 {
                    b |= LJOY_LEFT;
                }
                if 0.5 < self.left[0] {
                    b |= LJOY_RIGHT;
                }
                if self.left[1] < -0.5 {
                    b |= LJOY_UP;
                }
                if 0.5 < self.left[1] {
                    b |= LJOY_DOWN;
                }
                if self.right[0] < -0.5 {
                    b |= RJOY_LEFT;
                }
                if 0.5 < self.right[0] {
                    b |= RJOY_RIGHT;
                }
                if self.right[1] < -0.5 {
                    b |= RJOY_UP;
                }
                if 0.5 < self.right[1] {
                    b |= RJOY_DOWN;
                }
            }
            None => {
                self.left = [0.0; 2];
                self.right = [0.0; 2];
                self.dpad = dpad_axes(b);
            }
        }
        self.buttons = b;
    }
}

/// The d-pad as an axis pair: left (else right) on x, up (else down) on y.
fn dpad_axes(b: u32) -> [f32; 2] {
    let x = if b & DPAD_LEFT != 0 { -1.0 } else if b & DPAD_RIGHT != 0 { 1.0 } else { 0.0 };
    let y = if b & DPAD_UP != 0 { -1.0 } else if b & DPAD_DOWN != 0 { 1.0 } else { 0.0 };
    [x, y]
}

/// The analog mask bits of a button map (`ANALOG_*`).
pub const ANALOG_JOY_L_X: u32 = 0x1;
pub const ANALOG_JOY_L_Y: u32 = 0x8;
pub const ANALOG_JOY_L_POS_Y: u32 = 0x10;
pub const ANALOG_JOY_L_NEG_Y: u32 = 0x20;
pub const ANALOG_JOY_R_X: u32 = 0x40;
pub const ANALOG_JOY_R_Y: u32 = 0x200;
pub const ANALOG_PAD_X: u32 = 0x1000;
pub const ANALOG_PAD_Y: u32 = 0x8000;
pub const ANALOG_PAD_POS_Y: u32 = 0x10000;
pub const ANALOG_PAD_NEG_Y: u32 = 0x20000;

/// `Pad_ReadAxis` 0x179940: an axis by its mask and negate flag: the d-pad if the mask has it and it is pressed,
/// else the left stick, else the right (each negated, x or y by the mask), kept only on the side a POS / NEG mask
/// allows and past the dead zone 0.38046828, then negated again if the map says so.
pub fn read_axis(d: &PadDevice, mask: u32, negate: bool) -> f32 {
    let x = mask & 0x71c7 != 0;
    let mut v = 0.0f32;
    if mask & 0x3f000 != 0 {
        v = -(if x { d.dpad[0] } else { d.dpad[1] });
    }
    let mut zero = v == 0.0;
    if zero && mask & 0x3f != 0 {
        v = -(if x { d.left[0] } else { d.left[1] });
        zero = v == 0.0;
    }
    if zero && mask & 0xfc0 != 0 {
        v = -(if x { d.right[0] } else { d.right[1] });
    }
    if (mask & 0x12492 == 0 || 0.0 <= v) && (mask & 0x24924 == 0 || v <= 0.0) && 0.380_468_28 <= v.abs() {
        if negate {
            -v
        } else {
            v
        }
    } else {
        0.0
    }
}

/// A button map as the mapper holds it (`cInput` +4, +0xa8, +0x14c): per action id 0..0x28 the buttons that must
/// be down and the buttons looked at (the map's other set with them), and per axis its mask and negate flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonMap {
    pub pressed: [u32; 0x29],
    pub looked_at: [u32; 0x29],
    pub axes: [(u32, bool); 16],
}

/// The button map's grab combos in file order (`IS_BOOLGRAB` easyA..hardE), the same in both maps.
const GRAB_COMBOS: [u32; 15] = [
    LSHIFT2,
    RSHIFT,
    RSHIFT2,
    LSHIFT,
    RSHIFT | RSHIFT2,
    LSHIFT | LSHIFT2,
    RSHIFT2 | LSHIFT,
    RSHIFT2 | LSHIFT2,
    RSHIFT | LSHIFT,
    LSHIFT2 | RSHIFT,
    LSHIFT | RSHIFT | RSHIFT2,
    RSHIFT | RSHIFT2 | LSHIFT2,
    RSHIFT | LSHIFT | LSHIFT2,
    RSHIFT2 | LSHIFT | LSHIFT2,
    LSHIFT | LSHIFT2 | RSHIFT | RSHIFT2,
];

impl ButtonMap {
    /// `DATA/CONFIG/BTNMAP<n>.DAT` (n = stats+0x6c, 0 or 1) as `Pad_SetupVibration` 0x152770 binds it for a
    /// rider: the 14 buttons (`IS_BOOL`, actions 0x1b..0x28), the 15 grab combos to the rider's trick slots
    /// (`slots`: column 0 of the rider's and board's `tricky_data::trick_slots::TRICK_SLOTS`), and the 16 axes
    /// (`IS_ANALOG`). Action ids no grab is bound to keep 0 and 0 (always "down").
    pub fn btnmap(n: u32, slots: [u16; 15]) -> ButtonMap {
        let mut m = ButtonMap { pressed: [0; 0x29], looked_at: [0; 0x29], axes: [(0, false); 16] };
        let mut bind = |a: usize, pressed: u32, other: u32| {
            m.pressed[a] = pressed;
            m.looked_at[a] = other | pressed;
        };
        for (k, s) in slots.iter().enumerate() {
            bind(*s as usize, GRAB_COMBOS[k], SHIFTS);
        }
        let one = n != 0;
        // Antic, AnticAbort, Boost, CameraToggle, CameraReverse, Tweak, LateSpinMode, RailSpinMode,
        // PrewindTurnMode, RandomGrab, Reset, ShoveLeft, ShoveRight, LessonWait (the maps' ALL is 0)
        let bools: [(u32, u32); 14] = [
            (X, 0),
            (if one { NONE } else { LJOY }, 0),
            (SQUARE, 0),
            (NONE, 0),
            (if one { TRIANGLE } else { NONE }, 0),
            (SQUARE, 0),
            (NONE, 0),
            (NONE, 0),
            (NONE, 0),
            (NONE, SHIFTS),
            (SELECT, 0),
            (NONE, 0),
            (NONE, 0),
            (X, 0),
        ];
        for (k, (p, other)) in bools.iter().enumerate() {
            bind(0x1b + k, *p, *other);
        }
        let xy = ANALOG_PAD_X | ANALOG_JOY_L_X;
        let yy = ANALOG_PAD_Y | ANALOG_JOY_L_Y;
        // Brake, Crouch, Spin, Flip, Turn, ShoveSide, ShoveFwd, RailSpin, LateSpin, LateFlip, AirTurn, AirTilt,
        // PrewindTurn, PrewindSpin, PrewindFlip, GateRock
        m.axes = [
            (ANALOG_PAD_NEG_Y | ANALOG_JOY_L_NEG_Y, true),
            (ANALOG_PAD_POS_Y | ANALOG_JOY_L_POS_Y, false),
            (if one { xy } else { ANALOG_PAD_X }, true),
            (if one { yy } else { ANALOG_PAD_Y }, true),
            (xy, false),
            (ANALOG_JOY_R_X, false),
            (ANALOG_JOY_R_Y, false),
            (ANALOG_PAD_X, false),
            (ANALOG_PAD_X, true),
            (ANALOG_PAD_Y, true),
            (ANALOG_JOY_L_X, true),
            (ANALOG_JOY_L_Y, true),
            (if one { 0 } else { ANALOG_JOY_L_X }, false),
            (if one { xy } else { ANALOG_PAD_X }, true),
            (if one { yy } else { ANALOG_PAD_Y }, true),
            (if one { yy } else { ANALOG_JOY_L_Y }, false),
        ];
        m
    }
}

/// The mapper (`cInput`) on a pad: what `GetControls` asks.
pub struct Mapper<'a> {
    pub map: &'a ButtonMap,
    pub pad: &'a PadDevice,
}

impl PadMapper for Mapper<'_> {
    /// `Pad_ReadAxis` 0x179940
    fn axis(&mut self, axis: u32) -> f32 {
        let (m, n) = self.map.axes[axis as usize];
        read_axis(self.pad, m, n)
    }
    /// vf0 0x179860: the combo is down (of the buttons looked at, exactly those)
    fn q0c(&mut self, a: u32) -> u32 {
        (self.pad.buttons & self.map.looked_at[a as usize] == self.map.pressed[a as usize]) as u32
    }
    /// vf1 0x179890: any of its buttons is down
    fn q14(&mut self, a: u32) -> u32 {
        let p = self.map.pressed[a as usize];
        (p != NONE && self.pad.buttons & p != 0) as u32
    }
    /// vf2 0x1798c8: the combo, counting only buttons that went down this tick
    fn q1c(&mut self, a: u32) -> u32 {
        (self.pad.buttons & !self.pad.prev & self.map.looked_at[a as usize] == self.map.pressed[a as usize]) as u32
    }
    /// vf3 0x179900: any of its buttons went down this tick
    fn q24(&mut self, a: u32) -> u32 {
        let p = self.map.pressed[a as usize];
        (p != NONE && self.pad.buttons & !self.pad.prev & p != 0) as u32
    }
}
