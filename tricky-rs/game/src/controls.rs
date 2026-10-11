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
