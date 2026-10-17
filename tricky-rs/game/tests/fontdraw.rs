//! `tricky_game::fontdraw` (board row G7o) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is one of the row's functions on fresh random memory: strings wide and narrow (with
//! spaces, sign bits, and empty), counts below, at and past their lengths and −1, the four alignments and one
//! past, shadow and cursor on and off, the blink at 0 and above, styles that sound or not, fonts with a texture
//! or none. The glyph drawing, measuring and formatting are stand-ins on both sides that do their work (measure
//! by a made-up width per character, copy a format as its result) and log what they are handed, strings by
//! their contents (the draws hand copies on the game's stack). The calls, the answer and the memory (bar the
//! port's scratch) are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fontdraw as fd;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::{TextBoxEnv, DRAWER, DRAW_STRING, MEASURE};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const WIDE: u32 = ARENA;
const NARROW: u32 = ARENA + 0x400;
const DEST: u32 = ARENA + 0x800;
const STYLE: u32 = ARENA + 0x1000;
const FONT: u32 = ARENA + 0x1100;
const GLYPH: u32 = ARENA + 0x1200;
const APPOBJ: u32 = ARENA + 0x1400;
const RENDER: u32 = ARENA + 0x2000;
const RVT: u32 = ARENA + 0x2400;
const SCRATCH: u32 = 0x0180_0000;
/// The app, the drawer and the blink (0x337c58..0x338004), the sound's word, the narrow copy's first byte
const DATA: [(u32, u32); 3] = [(0x337c00, 0x500), (0x343000, 0x100), (0x38ca00, 0x40)];
fn vslot() -> u32 {
    0x01ff_01f0
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// What the stand-ins need of a memory
trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn r16(&mut self, a: u32) -> u16;
    fn w16(&mut self, a: u32, v: u16);
    fn w8(&mut self, a: u32, v: u8);
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn r16(&mut self, a: u32) -> u16 {
        Mem::r16(self, a)
    }
    fn w16(&mut self, a: u32, v: u16) {
        Mem::w16(self, a, v)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8; 1];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn r16(&mut self, a: u32) -> u16 {
        let mut b = [0u8; 2];
        self.read(a, &mut b).unwrap();
        u16::from_le_bytes(b)
    }
    fn w16(&mut self, a: u32, v: u16) {
        self.write_u16(a, v)
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write_u32(a, v)
    }
}

const STANDINS: [u32; 15] = [
    fd::DELETE, fd::MEM_FREE, fd::PARSE_SFN, fd::FIND_GLYPH, fd::DRAW_RAW, DRAW_STRING, fd::SHADOWED, fd::SHADOWED_CENTRED,
    fd::SHADOWED_RIGHT, MEASURE, fd::MEASURE_ASCII, fd::FILLED_RECT, fd::TYPE_SOUND, fd::VSPRINTF, fd::VSWPRINTF,
];

fn wide(io: &mut dyn Io, a: u32) -> Vec<u32> {
    let mut v = Vec::new();
    for k in 0..64 {
        let c = io.r16(a + 2 * k);
        v.push(c as u32);
        if c == 0 {
            break;
        }
    }
    v
}
fn narrow(io: &mut dyn Io, a: u32) -> Vec<u32> {
    let mut v = Vec::new();
    for k in 0..64 {
        let c = io.r8(a + k);
        v.push(c as u32);
        if c == 0 {
            break;
        }
    }
    v
}

/// A stand-in's work and answer, and its log
fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32]) -> (u32, Call) {
    let mut log = Vec::new();
    let answer = match func {
        fd::DELETE | fd::MEM_FREE | fd::TYPE_SOUND => {
            log.push(w[0]);
            0
        }
        fd::PARSE_SFN => {
            log.extend([w[0], w[1], w[2]]);
            0
        }
        fd::FIND_GLYPH => {
            log.extend([w[0], w[1]]);
            // some characters have no glyph; the others one whose +8 depends on it
            if w[1] % 3 == 0 {
                0
            } else {
                io.w8(GLYPH + 8, (w[1] as u8).wrapping_mul(37));
                GLYPH
            }
        }
        fd::DRAW_RAW => {
            log.push(w[0]);
            log.extend(narrow(io, w[1]));
            log.extend(&f[..3]);
            0
        }
        DRAW_STRING | fd::SHADOWED | fd::SHADOWED_CENTRED | fd::SHADOWED_RIGHT => {
            log.push(w[0]);
            log.extend(wide(io, w[1]));
            log.extend(&f[..3]);
            // (the alpha it draws with)
            log.push(io.r32(DRAWER));
            if func != DRAW_STRING {
                // (the shadowed draws leave the drawer in their own colour)
                io.w32(DRAWER, 0.25f32.to_bits());
            }
            0
        }
        MEASURE | fd::MEASURE_ASCII => {
            log.push(w[0]);
            let s = if func == MEASURE { wide(io, w[1]) } else { narrow(io, w[1]) };
            log.extend(&s);
            log.push((w[2] != 0) as u32);
            log.push(f[0]);
            let n: u32 = s.iter().filter(|&&c| c != 0).map(|&c| 3 + (c * 5) % 8).sum();
            if w[2] != 0 {
                io.w32(w[2] + 8, n);
            }
            n
        }
        fd::FILLED_RECT => {
            log.push(w[0]);
            log.extend(&f[..4]);
            0
        }
        fd::VSPRINTF | fd::VSWPRINTF => {
            let fmt = if func == fd::VSPRINTF { narrow(io, w[1]) } else { wide(io, w[1]) };
            log.extend(&fmt);
            for k in 0..12 {
                log.push(io.r32(w[2] + 4 * k));
            }
            // the result: the format as it is
            for (k, &c) in fmt.iter().enumerate() {
                if func == fd::VSPRINTF {
                    io.w8(w[0] + k as u32, c as u8);
                } else {
                    io.w16(w[0] + 2 * k as u32, c as u16);
                }
            }
            0
        }
        _ => {
            // the renderer's +0x1f0
            log.extend([w[0], w[1], w[2]]);
            0
        }
    };
    log.push(answer);
    (answer, Call(func, log))
}

struct Ours(Vec<Call>);
impl TextBoxEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(4, 0);
        let (a, c) = standin(m, func, &w, &f);
        self.0.push(c);
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn font_draws_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x17cc48u32, 0x17cc80, 0x17ccb8, 0x17ccf8, 0x17cd30, 0x17cd70, 0x17cdb8, 0x17ce00, 0x19dec0, 0x19e388, 0x19e380, 0x19ded0,
        0x19c788, 0x19c7b8, 0x19c8d0, 0x19c808, 0x19d9e0, 0x19da08, 0x19d278, 0x19d210, 0x19d370, 0x19d308, 0x19d8e0, 0x19d960,
        0x19df90, 0x19def0, 0x19e460, 0x19e3b8,
    ];
    for f in funcs {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in STANDINS.into_iter().chain([vslot()]) {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..16).map(|k| c.fpr[k]).collect();
            let (a, call) = standin(mem, func, &w, &f);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let data: Vec<Vec<u8>> = DATA
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            b
        })
        .collect();
    let mut g = Rng(0xf047_0000_0000_0001);
    let mut seen = vec![0usize; funcs.len()];
    let (mut cursors, mut sounds, mut blinks) = (0usize, 0usize, 0usize);
    for case in 0..28 * 600u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x1100])] };
        for (k, &(at, _)) in DATA.iter().enumerate() {
            m.regions.push((at, data[k].clone()));
        }
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the strings: letters, spaces, some with the sign bit, of 0 to 40 characters
        let len = g.pick(&[0u32, 1, 3, 10, 40]);
        for k in 0..len {
            let c = match g.below(9) {
                0 => 0x20,
                8 => 1,
                1 => 0x80 + g.below(0x7f),
                _ => 0x61 + g.below(26),
            };
            m.w16(WIDE + 2 * k, c as u16 | if g.below(16) == 0 { 0x100 } else { 0 });
            m.w8(NARROW + k, c as u8);
        }
        m.w16(WIDE + 2 * len, 0);
        m.w8(NARROW + len, 0);
        m.w16(DEST + 2 * g.pick(&[0u32, 0, 2, 7]), 0);
        // the style, its font, the app's renderer
        m.w32(STYLE, FONT);
        m.w32(STYLE + 4, g.pick(&[0u32, 1]));
        m.w32(FONT + 8, g.pick(&[0u32, ARENA + 0x3000]));
        m.w32(FONT + 0xc, g.pick(&[8u32, 12, 16]));
        m.w32(0x337c58, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        m.w16(RVT + 0x1f0, g.pick(&[0u16, 0x10, 0xfff0]));
        m.w32(RVT + 0x1f4, vslot());
        m.w32(fd::BLINK, g.pick(&[0u32, 1, 30]));
        m.w32(fd::BLINK_ON, g.below(2));
        wf(&mut m, DRAWER, g.f(0.0, 1.0));
        let count = g.pick(&[-2i32, -1, -1, 0, 1, 3, len as i32, len as i32 + 2]);
        let (align, shadow, cursor) = (g.pick(&[0u32, 1, 2, 3]), g.below(2), g.below(2));
        let (scale, x, y) = (g.pick(&[1.0f32, 0.5, 1.5]), g.f(-50.0, 600.0), g.f(-50.0, 450.0));
        let va: [u64; 6] = [0, 1, 2, 3, 4, 5].map(|_| (g.next() as u64) << 32 | g.next() as u64);
        let c = g.next();
        let flags = g.below(4);
        for (at, b) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, b).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut o = Ours(Vec::new());
        let which = (case % funcs.len() as u32) as usize;
        seen[which] += 1;
        let func = funcs[which];
        let (sf, xf, yf) = (F(scale), F(x), F(y));
        // the registers each takes: words from a0, floats from f12
        let (regs, fregs, ours): (Vec<u64>, Vec<f32>, u32) = match func {
            0x17cc48 => (vec![WIDE as u64], vec![], fd::wstr_count(&m, WIDE)),
            0x17cc80 => (vec![DEST as u64, WIDE as u64], vec![], fd::wstr_copy(&mut m, DEST, WIDE)),
            0x17ccb8 => (vec![DEST as u64, NARROW as u64], vec![], fd::wstr_set_ascii(&mut m, DEST, NARROW)),
            0x17ccf8 => (vec![DEST as u64, WIDE as u64], vec![], fd::wstr_to_ascii(&mut m, DEST, WIDE)),
            0x17cd30 => (vec![DEST as u64, WIDE as u64, count as i64 as u64], vec![], fd::wstr_copy_n(&mut m, DEST, WIDE, count)),
            0x17cd70 => (vec![DEST as u64, NARROW as u64, count as i64 as u64], vec![], fd::wstr_set_ascii_n(&mut m, DEST, NARROW, count)),
            0x17cdb8 => (vec![DEST as u64, WIDE as u64, count as i64 as u64], vec![], fd::wstr_to_ascii_n(&mut m, DEST, WIDE, count)),
            0x17ce00 => (vec![DEST as u64, WIDE as u64], vec![], fd::wstr_append(&mut m, DEST, WIDE)),
            0x19dec0 => (vec![STYLE as u64], vec![], fd::style_init(&mut m, STYLE)),
            0x19e388 => (vec![STYLE as u64], vec![], fd::style_init(&mut m, STYLE)),
            0x19e380 => {
                fd::style_set_font(&mut m, STYLE, FONT + 0x40);
                (vec![STYLE as u64, (FONT + 0x40) as u64], vec![], 0)
            }
            0x19ded0 => {
                fd::style_destruct(&mut m, &mut o, STYLE, flags);
                (vec![STYLE as u64, flags as u64], vec![], 0)
            }
            0x19c788 => (vec![FONT as u64, NARROW as u64], vec![], fd::font_init(&mut m, FONT, NARROW)),
            0x19c7b8 => (vec![FONT as u64, NARROW as u64], vec![], fd::font_construct(&mut m, &mut o, FONT, NARROW)),
            0x19c8d0 => {
                fd::font_free(&mut m, &mut o, FONT);
                (vec![FONT as u64], vec![], 0)
            }
            0x19c808 => {
                fd::font_delete(&mut m, &mut o, FONT, flags);
                (vec![FONT as u64, flags as u64], vec![], 0)
            }
            0x19d9e0 => (vec![FONT as u64, c as u64], vec![], fd::glyph_width(&mut m, &mut o, FONT, c)),
            0x19da08 => (vec![FONT as u64, c as u64], vec![], fd::glyph_advance(&mut m, &mut o, FONT, c)),
            0x19d278 => {
                fd::draw_centered(&mut m, &mut o, FONT, WIDE, xf, yf, sf, SCRATCH);
                (vec![FONT as u64, WIDE as u64], vec![x, y, scale], 0)
            }
            0x19d210 => {
                fd::draw_centered_ascii(&mut m, &mut o, FONT, NARROW, xf, yf, sf, SCRATCH);
                (vec![FONT as u64, NARROW as u64], vec![x, y, scale], 0)
            }
            0x19d370 => {
                fd::draw_right(&mut m, &mut o, FONT, WIDE, xf, yf, sf);
                (vec![FONT as u64, WIDE as u64], vec![x, y, scale], 0)
            }
            0x19d308 => {
                fd::draw_right_ascii(&mut m, &mut o, FONT, NARROW, xf, yf, sf, SCRATCH);
                (vec![FONT as u64, NARROW as u64], vec![x, y, scale], 0)
            }
            0x19d8e0 => {
                fd::drawf_raw(&mut m, &mut o, FONT, NARROW, va, sf, xf, yf, SCRATCH);
                let mut v = vec![FONT as u64, NARROW as u64];
                v.extend(va);
                (v, vec![scale, x, y], 0)
            }
            0x19d960 => {
                fd::drawf_wide(&mut m, &mut o, FONT, WIDE, va, sf, xf, yf, SCRATCH);
                let mut v = vec![FONT as u64, WIDE as u64];
                v.extend(va);
                (v, vec![scale, x, y], 0)
            }
            0x19df90 => {
                fd::draw_aligned(&mut m, &mut o, STYLE, align, WIDE, count, shadow, sf, xf, yf, SCRATCH);
                (vec![STYLE as u64, align as u64, WIDE as u64, count as i64 as u64, shadow as u64, 1], vec![scale, x, y], 0)
            }
            0x19def0 => {
                fd::draw_aligned_ascii(&mut m, &mut o, STYLE, align, NARROW, count, shadow, sf, xf, yf, SCRATCH);
                (vec![STYLE as u64, align as u64, NARROW as u64, count as i64 as u64, shadow as u64], vec![scale, x, y], 0)
            }
            0x19e460 => {
                fd::draw_with_cursor(&mut m, &mut o, STYLE, align, WIDE, count, shadow, cursor, sf, xf, yf, SCRATCH);
                (vec![STYLE as u64, align as u64, WIDE as u64, count as i64 as u64, shadow as u64, cursor as u64], vec![scale, x, y], 0)
            }
            _ => {
                fd::draw_with_cursor_ascii(&mut m, &mut o, STYLE, align, NARROW, count, shadow, cursor, sf, xf, yf, SCRATCH);
                (vec![STYLE as u64, align as u64, NARROW as u64, count as i64 as u64, shadow as u64, cursor as u64], vec![scale, x, y], 0)
            }
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v);
        }
        for (k, v) in fregs.into_iter().enumerate() {
            r.cpu.set_f(12 + k, v);
        }
        // (the stack cleared, as the port's scratch is fresh: with a count past the string, the aligned draw
        // reads its copy past the copy's end, where earlier calls left their words)
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != o.0 {
            let i = theirs.iter().zip(&o.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.0.len()));
            let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(16)])).unwrap_or_default();
            panic!("case {case} {func:#x}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.0.len(), show(theirs.get(i)), show(o.0.get(i)));
        }
        let answer = r.cpu.gpr(2) as u32;
        let returns = matches!(func, 0x17cc48..=0x17ce00 | 0x19dec0 | 0x19e388 | 0x19c788 | 0x19c7b8 | 0x19d9e0 | 0x19da08);
        assert!(!returns || answer == ours, "case {case} {func:#x}: answer theirs {answer:#x} ours {ours:#x}");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        cursors += o.0.iter().filter(|c| c.0 == fd::FILLED_RECT).count();
        sounds += o.0.iter().filter(|c| c.0 == fd::TYPE_SOUND).count();
        blinks += (matches!(func, 0x19e460 | 0x19e3b8) && count != -1 && m.r32(fd::BLINK) == 70) as usize;
    }
    println!("{cursors} cursors, {sounds} sounds, {blinks} blinks");
    assert!(cursors > 50 && sounds > 50 && blinks > 50);
}
