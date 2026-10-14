//! The memory card overlay's state machine (board row G9m): `MCOverlay_RunState` 0x1cf040 and the thirty states it
//! runs, `MemCardOp_Update` 0x1ceee0 and the small op functions. The overlay is an object in the game's memory
//! (`o`: +4 mode, +8 and +0xc and +0x10..+0x1c flags, +0x24 / +0x28 / +0x30 byte counts, +0x38 the slot asked for,
//! +0x44 a timer, +0x48 / +0x4c / +0x50 floats, +0x54 the state, +0x15c / +0x160 the wait for the card, +0x21c, +0x220
//! the slot, +0x224 the card system, +0x428 the overlay's own second vtable); what it asks of the card system and of
//! itself are virtual calls by vtable offset ([`CardEnv`]: `card` on the card system, `owner` through +0x428; the
//! state is set by `owner(0x1bc)`, the pad is `owner(0x1c)`). The one-shot flags that carry a button press to the next
//! tick are statics (0x33efbc..0x33f00c).

use crate::ps2f::F;
use crate::speechlib::Mem;

pub trait CardEnv {
    /// The virtual call at vtable offset `off` of the card system (`o + 0x224`), with its arguments after `this`.
    fn card(&mut self, m: &mut dyn Mem, off: u32, args: &[u32]) -> i64;
    /// The virtual call at vtable offset `off` of the overlay's second vtable (`o + 0x428`).
    fn owner(&mut self, m: &mut dyn Mem, off: u32, args: &[u32]) -> i64;
}

const PAD_A: u64 = 0x20;
const PAD_B: u64 = 0x800;

struct Cx<'a> {
    m: &'a mut dyn Mem,
    e: &'a mut dyn CardEnv,
    o: u32,
}

impl Cx<'_> {
    fn card(&mut self, off: u32) -> i64 {
        self.e.card(self.m, off, &[])
    }
    fn card1(&mut self, off: u32, a: u32) -> i64 {
        self.e.card(self.m, off, &[a])
    }
    fn owner(&mut self, off: u32) -> i64 {
        self.e.owner(self.m, off, &[])
    }
    fn set(&mut self, state: u32) {
        self.e.owner(self.m, 0x1bc, &[state]);
    }
    fn pad(&mut self) -> u64 {
        self.e.owner(self.m, 0x1c, &[]) as u64
    }
    fn r(&self, off: u32) -> u32 {
        self.m.r32(self.o + off)
    }
    fn w(&mut self, off: u32, v: u32) {
        self.m.w32(self.o + off, v);
    }
    fn state(&self) -> u32 {
        self.r(0x54)
    }
    fn slot_arg(&self) -> u32 {
        self.r(0x38)
    }
    fn flag(&self, a: u32) -> bool {
        self.m.r32(a) != 0
    }
    fn setflag(&mut self, a: u32, v: u32) {
        self.m.w32(a, v);
    }
}

// card system offsets
const BUSY: u32 = 0x5c;
const HAS_CARD: u32 = 0x174;
const CARD_ERR: u32 = 0x154;
const FORMATTED: u32 = 0x17c;
const C194: u32 = 0x194;
const C164: u32 = 0x164;
const OP_DONE: u32 = 0x1ec;
const C1CC: u32 = 0x1cc;
const C1BC: u32 = 0x1bc;
const C1C4: u32 = 0x1c4;
const C64: u32 = 0x64;
const C2C: u32 = 0x2c;
const C6C: u32 = 0x6c;
const FREE: u32 = 0xa4;
const NEEDED: u32 = 0x8c;
const C74: u32 = 0x74;
// owner offsets
const O_CONFIRM: u32 = 0x1ac;
const O_CANCEL: u32 = 0x1b4;

/// 0x1d3818: the slot's status record (0x58 bytes a slot from +0x168): four flags set, then the ones that apply
/// cleared, and a formatted card with room reads the slot's header into the record.
fn read_slot_status(c: &mut Cx) {
    let off = c.r(0x220).wrapping_mul(0x58);
    let o = c.o;
    for k in [0x168, 0x16c, 0x170, 0x174] {
        c.m.w32(o.wrapping_add(k).wrapping_add(off), 1);
    }
    for k in 0..0x42 {
        c.m.w8(off.wrapping_add(o).wrapping_add(0x17c).wrapping_add(k), 0);
    }
    if c.card(CARD_ERR) == 0 {
        if c.card(FORMATTED) == 0 {
            let at = c.r(0x220).wrapping_mul(0x58).wrapping_add(o).wrapping_add(0x16c);
            c.m.w32(at, 0);
        } else if c.card(C194) == 0 {
            let at = c.r(0x220).wrapping_mul(0x58).wrapping_add(o).wrapping_add(0x170);
            c.m.w32(at, 0);
        } else {
            let v = c.card(NEEDED) as u32;
            let at = c.r(0x220).wrapping_mul(0x58).wrapping_add(o).wrapping_add(0x178);
            c.m.w32(at, v);
            let slot = c.r(0x220);
            let buf = c.r(0x220).wrapping_mul(0x58).wrapping_add(o).wrapping_add(0x17c);
            c.e.card(c.m, 0x1e4, &[slot, buf, 0x21]);
        }
    } else {
        let at = c.r(0x220).wrapping_mul(0x58).wrapping_add(o).wrapping_add(0x168);
        c.m.w32(at, 0);
    }
}

/// 0x1cfbc8: choose the state from the card (`flag` allows the move to 6).
fn pick_state(c: &mut Cx, flag: bool) {
    if c.card(HAS_CARD) == 0 {
        if c.card(CARD_ERR) == 0 {
            if c.state() == 4 {
                return;
            }
            c.set(4);
        } else {
            c.set(5);
        }
        return;
    }
    if c.card(FORMATTED) != 0 && c.card(C194) != 0 {
        if c.card(C164) == 0 {
            if !flag {
                return;
            }
            c.set(6);
            return;
        }
    }
    if c.r(8) == 0 {
        c.set(0x1c);
    } else {
        c.set(7);
    }
}

/// The pad part the long states share: A or B pressed, the other one or the d-pad.
fn pad_long(c: &mut Cx, s_b: u32, s_x: u32, s_a: u32) {
    let p = c.pad();
    if p & PAD_A == 0 {
        if p & PAD_B == 0 {
            if p & 0x40 == 0 {
                if p & 0x20001 == 0 {
                    if p & 0x40002 != 0 {
                        c.owner(0x19c);
                    }
                } else {
                    c.owner(0x1a4);
                }
            } else {
                c.owner(O_CONFIRM);
                c.setflag(s_x, 1);
            }
        } else {
            c.owner(O_CONFIRM);
            c.setflag(s_b, 1);
        }
    } else {
        c.owner(O_CANCEL);
        c.setflag(s_a, 1);
    }
}

// state 2 (0x1cf8d0)
fn st_wait_timer(c: &mut Cx) {
    let t = c.r(0x44) as i32;
    c.w(0x44, t.wrapping_sub(1) as u32);
    if t < 1 {
        c.set(3);
    }
}

// state 3 (0x1cf918)
fn st_check_card(c: &mut Cx) {
    if c.card(BUSY) != 0 {
        return;
    }
    if c.card(HAS_CARD) == 0 {
        if c.card(CARD_ERR) == 0 {
            c.set(4);
        } else {
            c.set(5);
        }
        return;
    }
    if c.r(8) == 0 {
        if c.card(FORMATTED) == 0 || c.card(C194) == 0 || c.card(C164) != 0 {
            c.set(0x1c);
        } else {
            c.set(6);
        }
    } else if c.card(FORMATTED) == 0 || c.card(C194) == 0 || c.card(C164) != 0 {
        c.set(7);
    } else {
        c.set(6);
    }
}

// state 4 (0x1cfaa8)
fn st_02(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if !c.flag(0x33efc0) {
            if !c.flag(0x33efc4) {
                pick_state(c, true);
            } else {
                c.setflag(0x33efc4, 0);
                c.card(C64);
                c.set(2);
            }
        } else {
            c.setflag(0x33efc0, 0);
            c.set(1);
        }
    }
    let p = c.pad();
    if p & PAD_A == 0 {
        if p & PAD_B != 0 {
            c.setflag(0x33efc4, 1);
            c.owner(O_CONFIRM);
        }
    } else {
        c.setflag(0x33efc0, 1);
        c.owner(O_CANCEL);
    }
}

// state 5 (0x1cfcf8)
fn st_03(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if !c.flag(0x33efc8) {
            if !c.flag(0x33efcc) {
                if c.card(HAS_CARD) == 0 {
                    if c.card(CARD_ERR) == 0 {
                        c.set(4);
                    }
                    // an error on a card that is not there: nothing to do but read the pad
                } else {
                    if c.card(FORMATTED) != 0 && c.card(C194) != 0 && c.card(C164) == 0 {
                        c.set(6);
                    } else if c.r(8) == 0 {
                        c.set(0x1c);
                    } else {
                        c.set(7);
                    }
                }
            } else {
                c.setflag(0x33efcc, 0);
                c.card(C64);
                c.set(2);
            }
        } else {
            c.setflag(0x33efc8, 0);
            c.set(1);
        }
    }
    let p = c.pad();
    if p & PAD_A == 0 {
        if p & PAD_B != 0 {
            c.owner(O_CONFIRM);
            c.setflag(0x33efcc, 1);
        }
    } else {
        c.owner(O_CANCEL);
        c.setflag(0x33efc8, 1);
    }
}

// state 6 (0x1cfee0)
fn st_04(c: &mut Cx) {
    let t = c.r(0x44) as i32;
    c.w(0x44, t.wrapping_sub(1) as u32);
    if 0 < t {
        return;
    }
    if c.card(BUSY) != 0 {
        return;
    }
    let a;
    if c.r(0xc) == 0 {
        a = c.r(8);
    } else if c.card(FORMATTED) == 0 {
        a = c.r(8);
    } else if c.card(C194) == 0 {
        a = c.r(8);
    } else if c.card(C164) == 0 {
        c.set(6);
        return;
    } else {
        a = c.r(8);
    }
    if a == 0 {
        if c.card(HAS_CARD) == 0 {
            c.set(4);
        } else if c.card(CARD_ERR) == 0 {
            read_slot_status(c);
            c.set(0x1c);
        } else {
            c.set(4);
        }
    } else if c.card(HAS_CARD) == 0 {
        c.set(4);
    } else if c.card(CARD_ERR) == 0 {
        if c.card(FORMATTED) != 0 && c.card(C194) != 0 && c.card(C164) == 0 && c.card(C6C) == 0 {
            let free = c.card(FREE);
            let needed = c.card(NEEDED);
            if needed < free {
                c.set(0x16);
                return;
            }
        }
        read_slot_status(c);
        c.set(7);
    } else {
        c.set(4);
    }
}

// state 7 (0x1d0a38)
fn st_05(c: &mut Cx) {
    if c.r(0x15c) != 0 {
        pad_long(c, 0x33efe4, 0x33efe8, 0x33efec);
        return;
    }
    if c.card(HAS_CARD) == 0 {
        c.set(2);
        return;
    }
    if c.card(CARD_ERR) != 0 {
        c.set(5);
        return;
    }
    if c.r(0xc) != 0 && c.card(FORMATTED) != 0 && c.card(C194) != 0 && c.card(C164) == 0 {
        c.set(6);
        return;
    }
    if c.card(C6C) == 0 && c.card(FORMATTED) != 0 && c.card(C194) != 0 && c.card(C164) == 0 {
        let free = c.card(FREE);
        let needed = c.card(NEEDED);
        if needed < free {
            c.set(0x16);
            return;
        }
    }
    if !c.flag(0x33efe4) {
        if !c.flag(0x33efe8) {
            if c.flag(0x33efec) {
                c.setflag(0x33efec, 0);
                c.set(1);
                return;
            }
        } else {
            c.setflag(0x33efe8, 0);
            if c.r(0x18) != 0 {
                let (s, d) = (c.slot_arg(), c.o + 0xd8);
                c.e.card(c.m, C74, &[s, d]);
                c.set(0x17);
                return;
            }
        }
    } else {
        c.setflag(0x33efe4, 0);
        c.w(0x4c, 0x3f80_0000);
        c.w(0x50, 0x3c88_8f86);
        if c.r(0x10) != 0 {
            if c.card(FORMATTED) == 0 || c.card(C194) == 0 || c.card(C164) != 0 {
                c.set(10);
                return;
            }
            let s = c.slot_arg();
            if c.card1(C1CC, s) == 0 {
                let free = c.card(FREE);
                let needed = c.card(NEEDED);
                if needed < free {
                    c.set(0x15);
                    return;
                }
            }
            let s = c.slot_arg();
            if c.card1(C1CC, s) == 0 {
                let d = c.o + 0xd8;
                c.e.owner(c.m, 0x1bc, &[0xd, d]);
            } else {
                let s = c.slot_arg();
                c.card1(C74, s);
                c.set(9);
                return;
            }
        }
    }
    pad_long(c, 0x33efe4, 0x33efe8, 0x33efec);
}

// state 9 (0x1d1c30)
fn st_07(c: &mut Cx) {
    if c.r(0x15c) == 0 {
        if c.card(HAS_CARD) == 0 {
            c.set(4);
            return;
        }
        if c.card(OP_DONE) != 0 {
            c.owner(0x1c);
            c.set(0x14);
            return;
        }
        if c.flag(0x33f00c) {
            c.setflag(0x33f00c, 0);
            c.set(0xd);
        }
    }
    let p = c.pad();
    if p & PAD_A == 0 {
        if p & PAD_B == 0 {
            return;
        }
        c.owner(O_CANCEL);
        c.setflag(0x33f00c, 1);
        return;
    }
    c.owner(O_CONFIRM);
    c.set(7);
}

// state 10 (0x1d0150)
fn st_08(c: &mut Cx) {
    if c.r(0x15c) == 0 {
        if c.card(HAS_CARD) == 0 {
            c.set(4);
            return;
        }
        if c.flag(0x33efd0) {
            c.setflag(0x33efd0, 0);
            c.set(0xb);
            return;
        }
    }
    let p = c.pad();
    if p & PAD_B == 0 {
        if p & PAD_A == 0 {
            return;
        }
        c.owner(O_CONFIRM);
        c.setflag(0x33efd0, 1);
        return;
    }
    c.owner(O_CANCEL);
    c.set(7);
}

// state 11 (0x1d0240)
fn st_09(c: &mut Cx) {
    let t = c.r(0x44) as i32;
    c.w(0x44, t.wrapping_sub(1) as u32);
    if t < 1 && c.card(BUSY) == 0 {
        if c.card(0x4c) == 0 {
            c.set(0xd);
        } else {
            c.set(0xc);
        }
    }
}

// state 12 (0x1d02e8)
fn st_10(c: &mut Cx) {
    let p = c.pad();
    if p & 0x880 != 0 {
        c.owner(O_CONFIRM);
        c.set(2);
    }
}

// state 13 (0x1d06a8)
fn st_11(c: &mut Cx) {
    let s = c.slot_arg();
    if c.card1(C1CC, s) == 0 {
        let s = c.slot_arg();
        if c.card1(C1BC, s) == 0 {
            c.set(0xf);
            return;
        }
    }
    c.set(0xe);
}

// state 14 and 15 (0x1d0740, 0x1d07d8)
fn st_done_or(c: &mut Cx, next: u32) {
    if c.card(BUSY) == 0 {
        if c.card(OP_DONE) == 0 {
            c.set(next);
        } else {
            c.set(0x14);
        }
    }
}

fn progress(c: &mut Cx) {
    let a = c.r(0x24) as i32 as f32;
    let b = c.r(0x30) as i32 as f32;
    c.w(0x48, (F(a) / F(b)).0.to_bits());
}

// state 16 (0x1d0870)
fn st_14(c: &mut Cx) {
    progress(c);
    c.set(0x11);
}

// 0x1d1f60
fn fail_save(c: &mut Cx) {
    c.setflag(0x33efbc, 0);
    c.card(0x12c);
    c.set(0x14);
    c.owner(0x3c);
}

// state 17 (0x1d08b8)
fn st_15(c: &mut Cx) {
    let t = c.r(0x44) as i32;
    c.w(0x44, t.wrapping_sub(1) as u32);
    progress(c);
    if c.card(OP_DONE) == 0 {
        if c.card(0x11c) == 1 {
            if (c.r(0x24) as i32) < c.r(0x30) as i32 {
                if c.owner(0x2c) == 0 {
                    fail_save(c);
                }
            } else {
                c.card(0x124);
                c.owner(0x34);
                c.set(0x12);
            }
        } else if c.card(OP_DONE) != 0 {
            c.owner(0x3c);
            c.set(0x14);
        }
    } else {
        c.set(0x14);
        c.owner(0x3c);
    }
}

// state 18 (0x1d0f20)
fn st_16(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if (c.r(0x44) as i32) < 1 {
            if c.card(OP_DONE) == 0 {
                c.set(0x13);
                return;
            }
            c.set(0x14);
        } else if c.card(HAS_CARD) != 0 {
            let t = c.r(0x44);
            c.w(0x44, t.wrapping_sub(1));
        } else {
            c.set(4);
        }
    }
}

// state 19 (0x1d05f8)
fn st_17(c: &mut Cx) {
    let p = c.pad();
    if p & 0x80 == 0 {
        if p & PAD_B != 0 {
            c.owner(O_CONFIRM);
            c.set(1);
        }
    } else {
        c.owner(O_CONFIRM);
        c.set(1);
    }
}

// state 20 and 34 (0x1d0358, 0x1d0ff0): wait for the card, then back to 6 after a press
fn st_wait_press(c: &mut Cx, flag: u32) {
    if c.card(BUSY) == 0 && c.flag(flag) {
        c.setflag(flag, 0);
        c.card(C64);
        c.set(6);
        return;
    }
    let p = c.pad();
    if p & 0x880 != 0 {
        c.owner(O_CONFIRM);
        c.setflag(flag, 1);
    }
}

// state 21 (0x1d0420)
fn st_19(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        c.setflag(0x33efd8, 0);
        c.card(C64);
        c.set(7);
    }
    let p = c.pad();
    if p & 0x880 != 0 {
        c.owner(O_CONFIRM);
        c.setflag(0x33efd8, 1);
    }
}

// state 22 (0x1d04d8)
fn st_20(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if !c.flag(0x33efe0) {
            if !c.flag(0x33efdc) {
                pick_state(c, false);
            } else {
                c.setflag(0x33efdc, 0);
                c.set(1);
            }
        } else {
            c.setflag(0x33efe0, 0);
            c.card(C64);
            c.set(2);
        }
    }
    let p = c.pad();
    if p & PAD_B == 0 {
        if p & PAD_A != 0 {
            c.owner(O_CANCEL);
            c.setflag(0x33efdc, 1);
        }
    } else {
        c.owner(O_CONFIRM);
        c.setflag(0x33efe0, 1);
    }
}

// state 23 (0x1d17c8)
fn st_21(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if c.card(HAS_CARD) == 0 {
            c.set(4);
            return;
        }
        if c.card(OP_DONE) != 0 {
            c.owner(0x1c);
            c.set(0x1b);
            return;
        }
        if c.flag(0x33f000) {
            c.setflag(0x33f000, 0);
            c.set(0x18);
            return;
        }
        if c.flag(0x33f004) {
            c.setflag(0x33f004, 0);
            if c.r(8) == 0 {
                c.set(0x1c);
            } else {
                c.set(7);
            }
        }
    }
    let p = c.pad();
    if p & PAD_A == 0 {
        if p & PAD_B != 0 {
            c.owner(O_CONFIRM);
            c.setflag(0x33f000, 1);
        }
    } else {
        c.owner(O_CANCEL);
        c.setflag(0x33f004, 1);
    }
}

// state 24 (0x1d1978)
fn st_22(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if c.card(OP_DONE) == 0 {
            c.set(0x19);
        } else {
            c.set(0x1b);
        }
    }
}

// state 25 (0x1d1ac0)
fn st_23(c: &mut Cx) {
    if (c.r(0x44) as i32) < 1 {
        if c.card(BUSY) != 0 {
            return;
        }
        if c.card(OP_DONE) == 0 {
            c.set(0x1a);
        } else {
            c.set(0x1b);
        }
    } else {
        if c.card(BUSY) == 0 {
            if c.card(OP_DONE) != 0 {
                c.set(0x1b);
                return;
            }
            let a = c.r(0x21c);
            c.card1(C2C, a);
        }
        let t = c.r(0x44);
        c.w(0x44, t.wrapping_sub(1));
    }
}

// state 26 (0x1d1bc0)
fn st_24(c: &mut Cx) {
    let p = c.pad();
    if p & 0x880 != 0 {
        c.owner(O_CONFIRM);
        c.set(6);
    }
}

// state 27 (0x1d1a10)
fn st_25(c: &mut Cx) {
    if c.card(BUSY) == 0 && c.flag(0x33f008) {
        c.setflag(0x33f008, 0);
        c.set(6);
        return;
    }
    let p = c.pad();
    if p & 0x880 != 0 {
        c.owner(O_CONFIRM);
        c.setflag(0x33f008, 1);
    }
}

// state 28 (0x1d1498)
fn st_26(c: &mut Cx) {
    if c.r(0x15c) != 0 {
        pad_long(c, 0x33eff4, 0x33eff8, 0x33effc);
        return;
    }
    if c.card(HAS_CARD) == 0 {
        c.set(2);
        return;
    }
    if c.card(CARD_ERR) != 0 {
        c.set(5);
        return;
    }
    let cond = c.r(0xc) == 0 || c.card(FORMATTED) == 0 || c.card(C194) == 0 || c.card(C164) != 0;
    if !cond {
        c.set(6);
        return;
    }
    if !c.flag(0x33eff4) {
        if !c.flag(0x33eff8) {
            if c.flag(0x33effc) {
                c.setflag(0x33effc, 0);
                c.set(1);
            }
        } else {
            c.setflag(0x33eff8, 0);
            if c.r(0x18) != 0 {
                let (s, d) = (c.slot_arg(), c.o + 0xd8);
                c.e.card(c.m, C74, &[s, d]);
                c.set(0x17);
                return;
            }
        }
    } else {
        c.setflag(0x33eff4, 0);
        if c.r(0x14) != 0 {
            c.set(0x1d);
            return;
        }
    }
    pad_long(c, 0x33eff4, 0x33eff8, 0x33effc);
}

// state 29 (0x1d1268)
fn st_27(c: &mut Cx) {
    if c.card(OP_DONE) == 0 {
        c.set(0x1e);
    } else {
        c.set(0x22);
        c.owner(C64);
    }
}

// 0x1d2010
fn fail_load(c: &mut Cx) {
    c.card(0xdc);
    c.set(0x22);
    c.owner(C64);
}

// state 30 (0x1d12f0)
fn st_28(c: &mut Cx) {
    let n = c.card(0x7c) as i32 as f32;
    let d = c.r(0x30) as i32 as f32;
    c.w(0x48, (F(n) / F(d)).0.to_bits());
    if c.card(OP_DONE) == 0 {
        if c.card(0xcc) == 1 {
            if (c.r(0x28) as i32) < c.r(0x30) as i32 {
                if c.owner(0x54) == 0 {
                    fail_load(c);
                }
            } else {
                let rest = c.r(0x30).wrapping_sub(c.r(0x28));
                if c.e.owner(c.m, 0x5c, &[rest]) == 0 {
                    fail_load(c);
                    return;
                }
                c.card(0xd4);
                c.set(0x1f);
            }
        } else if c.card(OP_DONE) != 0 {
            c.owner(C64);
            c.set(0x22);
        }
    } else {
        c.set(0x22);
        c.owner(C64);
    }
}

// state 31 (0x1d1720)
fn st_29(c: &mut Cx) {
    if c.card(BUSY) == 0 {
        if c.card(OP_DONE) == 0 {
            c.set(0x20);
        } else {
            c.set(0x22);
            c.owner(C64);
        }
    }
}

// state 32 (0x1d10b8)
fn st_30(c: &mut Cx) {
    if (c.r(0x44) as i32) < 1 {
        if c.card(BUSY) == 0 {
            if c.card(OP_DONE) == 0 {
                c.set(0x21);
            } else {
                c.set(0x22);
                c.owner(C64);
            }
        }
    } else {
        if c.card(BUSY) == 0 {
            let a = c.r(0x21c);
            c.card1(C2C, a);
        }
        let t = c.r(0x44);
        c.w(0x44, t.wrapping_sub(1));
    }
}

/// 0x1cf040: one tick of the overlay: ask the card system for its state (the flags at +0x10..+0x1c are rebuilt in
/// states 7 and 0x1c), count the wait down, run the state's function, then tell the overlay what the state means
/// (0x15c / 0x164).
pub fn run_state(m: &mut dyn Mem, e: &mut dyn CardEnv, o: u32) {
    let mut c = Cx { m, e, o };
    if c.state() != 1 {
        c.w(0x10, 0);
        c.w(0x14, 0);
        c.w(0x18, 0);
        c.w(0x1c, 0);
        let busy = c.card(BUSY);
        let mut update = true;
        if busy == 0 {
            let s = c.state();
            if s == 0x14 || s == 0x22 || s == 0x1b {
                update = false;
            } else if s == 0xc {
                update = false;
            }
        }
        if update {
            c.card(0x1c);
            let s = c.state();
            if s == 7 || s == 0x1c {
                c.w(0x1c, 1);
                let mut a = 0;
                if c.card(HAS_CARD) != 0 {
                    let sl = c.slot_arg();
                    if c.card1(C1BC, sl) == 0 {
                        let sl = c.slot_arg();
                        a = (c.card1(C1C4, sl) == 0) as u32;
                    }
                }
                c.w(0x10, a);
                let mut b = 0;
                if c.card(FORMATTED) != 0 && c.card(C194) != 0 {
                    if c.card(C164) == 0 {
                        let sl = c.slot_arg();
                        if c.card1(C1CC, sl) != 0 {
                            let sl = c.slot_arg();
                            if c.card1(C1BC, sl) == 0 {
                                let sl = c.slot_arg();
                                b = (c.card1(C1C4, sl) == 0) as u32;
                            }
                        }
                    }
                }
                c.w(0x14, b);
                if c.state() == 0x1c {
                    c.w(0x18, 0);
                } else {
                    let sl = c.slot_arg();
                    let v = c.card1(C1CC, sl) as u32;
                    c.w(0x18, v);
                }
            }
            if c.card(0x1ac) != 0 {
                c.w(0xc, 1);
                c.card(0x144);
            }
        }
        if c.r(0x15c) != 0 && c.card(BUSY) == 0 {
            c.w(0x15c, 0);
            c.w(0x160, 5);
        }
        let mut s = c.state();
        if matches!(s, 7 | 0x1c | 4 | 0x16 | 0x17 | 9 | 10 | 0x1b) {
            let n = (c.r(0x160) as i32).wrapping_sub(1);
            c.w(0x160, n as u32);
            if c.r(0x15c) == 0 && !(0 < n) {
                let a = c.r(0x21c);
                c.card1(C2C, a);
                s = c.state();
                c.w(0x15c, 1);
            }
        }
        let idx = s.wrapping_sub(2);
        match idx {
            0 => st_wait_timer(&mut c),
            1 => st_check_card(&mut c),
            2 => st_02(&mut c),
            3 => st_03(&mut c),
            4 => st_04(&mut c),
            5 => st_05(&mut c),
            7 => st_07(&mut c),
            8 => st_08(&mut c),
            9 => st_09(&mut c),
            10 => st_10(&mut c),
            0xb => st_11(&mut c),
            0xc => st_done_or(&mut c, 0xf),
            0xd => st_done_or(&mut c, 0x10),
            0xe => st_14(&mut c),
            0xf => st_15(&mut c),
            0x10 => st_16(&mut c),
            0x11 => st_17(&mut c),
            0x12 => st_wait_press(&mut c, 0x33efd4),
            0x13 => st_19(&mut c),
            0x14 => st_20(&mut c),
            0x15 => st_21(&mut c),
            0x16 => st_22(&mut c),
            0x17 => st_23(&mut c),
            0x18 => st_24(&mut c),
            0x19 => st_25(&mut c),
            0x1a => st_26(&mut c),
            0x1b => st_27(&mut c),
            0x1c => st_28(&mut c),
            0x1d => st_29(&mut c),
            0x1e => st_30(&mut c),
            0x1f => {
                c.owner(0x6c);
            }
            0x20 => st_wait_press(&mut c, 0x33eff0),
            _ => {}
        }
        match c.state() {
            1 | 7 | 8 | 0x12 | 0x1c | 0x1f => {
                c.owner(0x15c);
                c.owner(0x164);
            }
            2 | 3 | 6 => {
                c.owner(0x164);
                c.owner(0x15c);
            }
            4 | 5 | 9 | 10 | 0xc | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 | 0x19 | 0x1a | 0x1b | 0x21 | 0x22 => {
                c.owner(0x15c);
            }
            0xb | 0x11 | 0x18 | 0x1e => {
                c.owner(0x164);
            }
            _ => {}
        }
    } else {
        c.owner(0x15c);
        c.owner(0x164);
    }
}

/// 0x1ceee0: the card op's tick: the blink (+0x4c eased between 0.2 and 1.0 at the rate +0x50, reversing at the
/// ends), then mode 0 runs the state machine and mode 1 waits for the card to finish.
pub fn update(m: &mut dyn Mem, e: &mut dyn CardEnv, o: u32) {
    let mut c = Cx { m, e, o };
    let rate = f32::from_bits(c.r(0x50));
    let v = (F(f32::from_bits(c.r(0x4c))) + F(rate)).0;
    c.w(0x4c, v.to_bits());
    let (limit, out) = if rate < 0.0 { (0.2f32, v < 0.2) } else { (1.0f32, 1.0 < v) };
    if out {
        c.w(0x4c, limit.to_bits());
        c.w(0x50, (-F(rate)).0.to_bits());
    }
    match c.r(4) {
        0 => {
            drop(c);
            run_state(m, e, o);
        }
        1 => {
            c.card(0x1c);
            if c.r(0x15c) != 0 {
                if c.card(BUSY) != 0 {
                    return;
                }
                c.w(0x15c, 0);
            }
            if c.card(BUSY) == 0 {
                c.owner(0x194);
                c.e.owner(c.m, 0x1c4, &[0]);
                c.set(2);
            }
        }
        _ => {}
    }
}

/// 0x1d2140: restart the op: waiting for the card, no result, state 0.
pub fn restart(m: &mut dyn Mem, e: &mut dyn CardEnv, o: u32) {
    let mut c = Cx { m, e, o };
    c.w(0x164, 1);
    c.w(0x20, 0);
    c.w(0x21c, 0);
    c.card1(C2C, 0);
    c.w(0x15c, 1);
    c.w(0x54, 0);
}

/// 0x1d21a0: the op has finished (state 1).
pub fn is_done(m: &dyn Mem, o: u32) -> bool {
    m.r32(o + 0x54) == 1
}

/// 0x1d21c0: the op's result.
pub fn result(m: &dyn Mem, o: u32) -> u32 {
    m.r32(o + 0x20)
}

/// 0x1ce8b0: begin an op of `size` bytes: the card system's call at 0x84 and the size at +0x30.
pub fn begin(m: &mut dyn Mem, e: &mut dyn CardEnv, o: u32, size: u32) {
    let mut c = Cx { m, e, o };
    c.card(0x84);
    c.w(0x30, size);
}
