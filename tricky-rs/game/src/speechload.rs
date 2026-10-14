//! `Speech_LoadEvents` 0x227d70 (board row G10j): the sections of `data/config/speech.inf` that match the level, read
//! token by token. The tokens it knows (their strings are at 0x3a03c8, 0x3a03d0 and 0x3a0410) set the streaming file,
//! open a BIG archive of speech banks (or a plain file when the archive is not there) and load the banks in it, or load
//! one named bank. Everything it touches is a call (the ini reader, `strcmp`, `sprintf`, the archive and bank loaders),
//! so [`LoadEnv::call`] answers them by address; its own work is the frame (`sp`: the ini reader at +0, the file name
//! buffers) and the speech system object (`sys`: +0x68 the level's name, +0xc8 two words a bank (the file or the BIG's
//! entry, then a flag), +0x358 the open archives, +0xe8 the count).

use crate::speechlib::Mem;

pub trait LoadEnv {
    /// The call to the function at `target` with up to four arguments; its answer.
    fn call(&mut self, m: &mut dyn Mem, target: u32, args: &[u32]) -> u32;
}

pub const INI_OPEN: u32 = 0x221668;
pub const FIND_SECTION: u32 = 0x221b60;
pub const SECTION_START: u32 = 0x221918;
pub const READ_TOKEN: u32 = 0x221c90;
pub const READ_STRING: u32 = 0x221ef8;
pub const AT_END: u32 = 0x221e90;
pub const INI_CLOSE: u32 = 0x221788;
pub const STRCMP: u32 = 0x2fcc98;
pub const SPRINTF: u32 = 0x2fca28;
pub const STRLEN: u32 = 0x2fd260;
pub const STRNCPY: u32 = 0x2fd630;
pub const SET_FILE: u32 = 0x229d18;
pub const OPEN_BIG: u32 = 0x2c8068;
pub const BIG_FIND: u32 = 0x2cc0d8;
pub const BIG_COUNT: u32 = 0x2cc1b0;
pub const BIG_LOOKUP: u32 = 0x2cbdc0;
pub const FREE: u32 = 0x2ccfc0;
pub const REGISTER: u32 = 0x2bad70;
pub const LOAD_BANK: u32 = 0x2283a8;

const T_FILE: u32 = 0x3a03c8;
const T_LEVEL: u32 = 0x3a03d0;
const T_BANK: u32 = 0x3a0410;
const F_BIG: u32 = 0x3a03e0;
const F_NAME: u32 = 0x3a03f0;
const F_PATH: u32 = 0x3a0408;

/// 0x227d70, with `sp` the frame after the prologue, `sys` the speech system (`a0`), `arg` its `a1` and `level` the
/// section to match (`a2`).
pub fn load_events(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, sys: u32, arg: u32, level: u32) {
    let (file, big, short, tmp) = (sp + 0x910, sp + 0x950, sp + 0x990, sp + 0x9b0);
    let name = sys + 0x68;
    m.w32(sp + 0xa14, arg);
    let ini = m.r32(0x3436fc);
    e.call(m, INI_OPEN, &[sp, ini, 0x10]);
    if e.call(m, FIND_SECTION, &[sp, level]) != 0 {
        e.call(m, SECTION_START, &[sp]);
        m.w32(sp + 0xa1c, file);
        m.w32(sp + 0xa20, big);
        m.w32(sp + 0xa24, tmp);
        m.w32(sp + 0xa28, short);
        'sec: while e.call(m, AT_END, &[sp]) == 0 {
            let tok = e.call(m, READ_TOKEN, &[sp]);
            m.w32(sp + 0xa18, tok);
            if e.call(m, STRCMP, &[T_FILE, tok]) == 0 {
                e.call(m, READ_STRING, &[sp, T_FILE, file]);
                let a0 = m.r32(0x343700);
                let a1 = m.r32(sys);
                e.call(m, SET_FILE, &[a0, a1, file]);
            }
            if e.call(m, STRCMP, &[T_LEVEL, tok]) == 0 {
                e.call(m, READ_STRING, &[sp, T_LEVEL, name]);
                let buf = sp + 0x810;
                e.call(m, SPRINTF, &[buf, F_BIG, name]);
                let h = e.call(m, OPEN_BIG, &[buf, 0]);
                let n = m.r32(sys + 0xe8);
                m.w32(sys + 0x358 + n.wrapping_mul(4), h);
                if h == 0 {
                    e.call(m, SPRINTF, &[buf, F_NAME, name]);
                    let h2 = e.call(m, OPEN_BIG, &[buf, 0]);
                    let n = m.r32(sys + 0xe8);
                    m.w32(sys + 0xc8 + n.wrapping_mul(8), h2);
                    if h2 == 0 {
                        break 'sec;
                    }
                    m.w32(sys + 0xcc + n.wrapping_mul(8), 1);
                    let a = m.r32(sp + 0xa14);
                    e.call(m, REGISTER, &[h2, a]);
                    let n = m.r32(sys + 0xe8);
                    m.w32(sys + 0xe8, n.wrapping_add(1));
                } else {
                    m.w8(big, 0);
                    e.call(m, SPRINTF, &[buf, F_NAME, name]);
                    let len = e.call(m, STRLEN, &[buf]) as i32;
                    let mut at = buf;
                    let mut i = 0i32;
                    while i < len {
                        let c = m.r8(buf.wrapping_add(i as u32)) as i8;
                        i += 1;
                        if c == 0x7c {
                            let dst = m.r32(sp + 0xa20);
                            e.call(m, STRNCPY, &[dst, buf, i as u32]);
                            at = buf.wrapping_add(i as u32);
                            let dst = m.r32(sp + 0xa20);
                            m.w8(dst.wrapping_add(i as u32), 0);
                            break;
                        }
                    }
                    let n = m.r32(sys + 0xe8);
                    let h = m.r32(sys + 0x358 + n.wrapping_mul(4));
                    let f = e.call(m, BIG_FIND, &[h, at]);
                    let n = m.r32(sys + 0xe8);
                    m.w32(sys + 0xcc + n.wrapping_mul(8), 0);
                    m.w32(sys + 0xc8 + n.wrapping_mul(8), f);
                    if f == 0 {
                        let h = m.r32(sys + 0x358 + n.wrapping_mul(4));
                        e.call(m, FREE, &[h]);
                        let n = m.r32(sys + 0xe8);
                        m.w32(sys + 0x358 + n.wrapping_mul(4), 0);
                        break 'sec;
                    }
                    let a = m.r32(sp + 0xa14);
                    e.call(m, REGISTER, &[f, a]);
                    let n = m.r32(sys + 0xe8);
                    let h = m.r32(sys + 0x358 + n.wrapping_mul(4));
                    let count = e.call(m, BIG_COUNT, &[h]) as i32;
                    let mut i = 0i32;
                    while i < count {
                        let n = m.r32(sys + 0xe8);
                        let h = m.r32(sys + 0x358 + n.wrapping_mul(4));
                        let r = e.call(m, BIG_LOOKUP, &[h, 0, i as u32, sp + 0xa10, 0]);
                        let n = m.r32(sys + 0xe8);
                        let off = m.r32(sp + 0xa10);
                        let h = m.r32(sys + 0x358 + n.wrapping_mul(4));
                        let want = m.r32(sys + 0xc8 + n.wrapping_mul(8));
                        let at = h.wrapping_add(off);
                        if at != want {
                            let out = m.r32(sp + 0xa24);
                            e.call(m, SPRINTF, &[out, F_PATH, big, r]);
                            let l = e.call(m, STRLEN, &[out]);
                            let out = m.r32(sp + 0xa24);
                            m.w8(out.wrapping_add(l).wrapping_sub(4), 0);
                            let out = m.r32(sp + 0xa24);
                            e.call(m, LOAD_BANK, &[sys, out, at]);
                        }
                        i += 1;
                    }
                    let n = m.r32(sys + 0xe8);
                    m.w32(sys + 0xe8, n.wrapping_add(1));
                }
            }
            if e.call(m, STRCMP, &[T_BANK, tok]) == 0 {
                e.call(m, READ_STRING, &[sp, T_BANK, file]);
                let out = m.r32(sp + 0xa28);
                e.call(m, SPRINTF, &[out, F_PATH, name, file]);
                let out = m.r32(sp + 0xa28);
                e.call(m, LOAD_BANK, &[sys, out, 0]);
            }
        }
    }
    e.call(m, INI_CLOSE, &[sp, 2]);
}
