//! `tricky_game::animcurve` (board row G5c3a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random curves of every kind (and runs of them) at unaligned addresses in a random arena, evaluated at
//! random frames (often whole ones and the ends); random clip files for the root offset; random Euler triples (with
//! whole turns added) for the delta and the lerps; random angles for the sine and cosine. The answers and the memory
//! compared.

use r5900::Runner;
use tricky_game::animcurve as c;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const CURVE: u32 = ARENA + 0x100;
const PTR: u32 = ARENA + 0x40;
const OUT: u32 = ARENA + 0x80;

/// A 24-bit float's three bytes for a value
fn b24(v: f32) -> [u8; 3] {
    let b = v.to_bits();
    [(b >> 8) as u8, (b >> 16) as u8, (b >> 24) as u8]
}

/// A random piece at `at` of length `len`: its bytes
fn piece(g: &mut Rng, kind: u32, len: u32) -> Vec<u8> {
    let h = (len << 4 | kind) as u16;
    let mut v = h.to_le_bytes().to_vec();
    let fl = |g: &mut Rng, v: &mut Vec<u8>| v.extend_from_slice(&b24(g.f(-3.0, 3.0)));
    match kind {
        0 => fl(g, &mut v),
        1 => (0..2).for_each(|_| fl(g, &mut v)),
        2 => (0..3).for_each(|_| fl(g, &mut v)),
        3 => (0..4).for_each(|_| fl(g, &mut v)),
        4 => (0..len).for_each(|_| v.extend_from_slice(&g.f(-3.0, 3.0).to_le_bytes())),
        6 => {
            (0..2).for_each(|_| fl(g, &mut v));
            (0..len).for_each(|_| v.push(g.next() as u8));
        }
        7 => {
            (0..2).for_each(|_| fl(g, &mut v));
            (0..len).for_each(|_| v.extend_from_slice(&(g.next() as u16).to_le_bytes()));
        }
        _ => {}
    }
    v
}

fn curve(g: &mut Rng) -> (Vec<u8>, u32) {
    let kind = [0u32, 1, 2, 3, 4, 5, 5, 6, 7, 9][g.below(10) as usize];
    if kind != 5 {
        let len = 1 + g.below(20);
        return (piece(g, kind, len), len);
    }
    let n = 1 + g.below(4);
    // (sometimes a header whose count field is negative: no pieces)
    let neg = if g.below(8) == 0 { 0x800 } else { 0 };
    let mut v = (((n | neg) << 4 | 5) as u16).to_le_bytes().to_vec();
    let mut total = 0;
    for _ in 0..n {
        let k = [0u32, 1, 2, 3, 4, 6, 7][g.below(7) as usize];
        let len = 1 + g.below(12);
        v.extend(piece(g, k, len));
        total += len;
    }
    (v, total)
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn curves_and_angles_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [0x157ed8u32, 0x158030, 0x1586c0, 0x202520, 0x102f80, 0x202758, 0x2027f0, 0x250c10] {
        r.allow(f);
    }
    let mut g = Rng(0x1580_0000_0000_0001);
    let mut counts = [0usize; 8];
    for case in 0..60_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        let kind = g.below(8) as usize;
        let mut fl = None;
        // (the function, its integer arguments, our answer as bits)
        let (f, args, mine): (u32, Vec<u32>, Option<u32>) = match kind {
            0 | 1 => {
                let (bytes, len) = curve(&mut g);
                let at = CURVE + g.below(4);
                for (k, b) in bytes.iter().enumerate() {
                    m.w8(at + k as u32, *b);
                }
                let t = match g.below(4) {
                    0 => g.below(len + 1) as f32,
                    1 => (len as f32 - 1.0).max(0.0),
                    _ => g.f(0.0, len as f32),
                };
                fl = Some(t);
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                if kind == 0 {
                    // a single piece (kind 5 too: its header read as a piece)
                    (0x158030, vec![0, at], Some(c::eval_piece(&m, at, F(t)).0.to_bits()))
                } else {
                    m.w32(PTR, at);
                    r.mem.write(ARENA, &m.regions[0].1).unwrap();
                    (0x157ed8, vec![PTR], Some(c::eval(&m, PTR, F(t)).0.to_bits()))
                }
            }
            2 => {
                // a clip file: headers, offsets, three curves
                let file = ARENA + 0x20;
                let hdrs = ARENA + 0x400;
                let offs = ARENA + 0x800;
                let data = ARENA + 0x900;
                m.w32(file + 8, hdrs);
                m.w32(file + 0xc, offs);
                m.w32(file + 0x10, data);
                let clip = g.below(6);
                for h in 0..8 {
                    m.w8(hdrs + 0x24 * h + 9, if g.below(2) == 0 { 0 } else { 1 + g.below(3) as u8 });
                    m.w32(hdrs + 0x24 * h + 4, g.below(6));
                }
                let mut at = 0u32;
                for k in 0..10 {
                    let (bytes, _) = curve(&mut g);
                    m.w32(offs + 4 * k, at);
                    for (i, b) in bytes.iter().enumerate() {
                        m.w8(data + at + i as u32, *b);
                    }
                    at += bytes.len() as u32 + g.below(3);
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                c::root_offset(&mut m, file, clip, OUT);
                (0x1586c0, vec![file, clip, OUT], None)
            }
            3 | 4 | 5 => {
                let (a, b, o) = (ARENA + 0x20, ARENA + 0x30, ARENA + 0x80);
                for k in 0..3 {
                    let turns = [0.0, 6.2831855, -6.2831855, 12.566371][g.below(4) as usize];
                    wf(&mut m, a + 4 * k, g.f(-3.5, 3.5));
                    wf(&mut m, b + 4 * k, g.f(-3.5, 3.5) + turns);
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                match kind {
                    3 => {
                        c::euler_delta_into(&mut m, o, a, b);
                        (0x202520, vec![o, a, b], None)
                    }
                    4 => {
                        let t = g.f(-1.0, 2.0);
                        fl = Some(t);
                        c::vec3_scale(&mut m, o, a, F(t));
                        (0x102f80, vec![o, a], None)
                    }
                    _ => {
                        let t = [0.0, 1.0, 0.5, g.f(0.0, 1.0)][g.below(4) as usize];
                        fl = Some(t);
                        let out = if g.below(2) == 0 { a } else { o };
                        c::euler_lerp(&mut m, a, b, out, F(t));
                        (0x202758, vec![a, b, out], None)
                    }
                }
            }
            6 => {
                // n triples in place
                let n = g.below(6);
                let (asx, bsx, ts) = (ARENA + 0x200, ARENA + 0x220, ARENA + 0x240);
                for k in 0..n {
                    let (pa, pb) = (ARENA + 0x300 + 0x10 * k, ARENA + 0x380 + 0x10 * k);
                    m.w32(asx + 4 * k, pa);
                    m.w32(bsx + 4 * k, pb);
                    wf(&mut m, ts + 4 * k, g.f(0.0, 1.0));
                    for j in 0..3 {
                        wf(&mut m, pa + 4 * j, g.f(-3.5, 3.5));
                        wf(&mut m, pb + 4 * j, g.f(-9.0, 9.0));
                    }
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                c::euler_lerp_array(&mut m, asx, bsx, ts, n as i32);
                (0x2027f0, vec![asx, bsx, ts, n], None)
            }
            _ => {
                let x = match g.below(3) {
                    0 => g.f(-100.0, 100.0),
                    1 => g.below(9) as f32 * std::f32::consts::FRAC_PI_4 - 3.1415927,
                    _ => g.f(-7.0, 7.0),
                };
                fl = Some(x);
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let (s, co) = c::sin_cos(F(x));
                wf(&mut m, OUT, s.0);
                wf(&mut m, OUT + 4, co.0);
                // (the game writes them; ours into the same words before the copy below)
                (0x250c10, vec![OUT, OUT + 4], None)
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        if let Some(v) = fl {
            r.cpu.set_f(12, v);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        if let Some(v) = mine {
            assert_eq!(r.cpu.f(0).to_bits(), v, "case {case}: {f:#x} answer");
        }
        let mut theirs = vec![0u8; SIZE as usize];
        r.mem.read(ARENA, &mut theirs).unwrap();
        let b = &m.regions[0].1;
        if &theirs != b {
            let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
            panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", ARENA + i as u32, theirs[i], b[i]);
        }
    }
    println!("{counts:?}");
}
