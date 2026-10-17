//! `tricky_game::fe_widget` (board row G9b2c) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random widgets and texts (their tween, flags, colours, position and size; the text's font, string,
//! scale and border), every function run on both sides with the same answers from the calls out (the virtuals, the
//! renderer, the drawer and the font's draws and widths, their float arguments logged from f12..f15), the calls
//! logged in order (vectors by value) and the memory compared. The colour update 0x2ad988 is stubbed where the others
//! call it and runs on its own in a second runner.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_venuesel::LOC_STRING;
use tricky_game::fe_widget::*;
use tricky_game::fe_widgets::UPDATE_COLOR;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) | 1
}

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const W: u32 = ARENA + 0x1000;
const REND: u32 = ARENA + 0x2000;
const FONT: u32 = ARENA + 0x2400;
const STRS: u32 = ARENA + 0x2800;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const V_BASE: u32 = 0x7f_0000;
const SCRATCH: u32 = 0x5c5c_5c5c;

fn slot(off: u32) -> u32 {
    V_BASE + off + 4
}

/// How a call's arguments are logged: the integer registers, the float registers from f12, the words read through
/// its second argument
fn shape(t: u32) -> (usize, usize, usize) {
    match t {
        DRAW_WIDE_BORDER | DRAW_WIDE | DRAW_NARROW_BORDER | DRAW_NARROW => (2, 3, 0),
        WIDTH_WIDE | WIDTH_NARROW => (3, 1, 0),
        DRAWER_COLOUR => (2, 0, 4),
        THING_DTOR | LOC_STRING => (2, 0, 0),
        UPDATE_COLOR => (1, 0, 0),
        t if t == slot(0x50) || t == slot(0x138) => (2, 0, 4),
        t if t == slot(0x70) => (2, 0, 3),
        t if t == slot(0xe8) => (2, 0, 0),
        t if t == slot(0x1d0) || t == slot(0x20) || t == slot(0x150) || t == slot(0x130) => (2, 0, 0),
        _ => (1, 0, 0),
    }
}

struct Calls {
    seed: u64,
    n: u32,
    log: Vec<(u32, Vec<u32>)>,
}
type A = Rc<RefCell<Calls>>;

fn log_call(a: &A, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, ints: Vec<u32>, floats: Vec<u32>) -> u32 {
    let (_, _, by) = shape(target);
    let mut v = ints;
    // (a size's w is the VU register's leftover: three words)
    let by = if target == slot(0x138) { 3 } else { by };
    if by != 0 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..by as u32).map(|k| read(at + 4 * k)));
    }
    if target == slot(0x130) {
        let at = v[1];
        let s = a.borrow().seed ^ a.borrow().n as u64;
        let mut g = Rng(s | 1);
        for k in 0..4 {
            write(at + 4 * k, (g.below(600) as f32 - 100.0).to_bits());
        }
        v.truncate(1);
    }
    v.extend(floats);
    for x in v.iter_mut() {
        if (0x01f0_0000..0x01ff_0000).contains(x) || (BUF..BUF + 0x100).contains(x) {
            *x = SCRATCH;
        }
    }
    let mut b = a.borrow_mut();
    b.log.push((target, v));
    b.n += 1;
    // (a multiplying mix: xorshift alone keeps successive answers of one call in a fixed relation)
    let mut g = Rng(mix(b.seed ^ (target as u64) << 16 ^ b.n as u64));
    g.next();
    match target {
        t if t == slot(0x1d8) => g.below(8),
        WIDTH_WIDE | WIDTH_NARROW => g.below(300),
        LOC_STRING => STRS + 0x40 * g.below(4),
        _ => g.below(4),
    }
}

struct Ours(A);
impl FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let (ni, nf, _) = shape(target);
        let ints: Vec<u32> = (0..ni).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let floats: Vec<u32> = (0..nf).map(|k| args.get(ni + k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &mut |at| cell.borrow().r32(at), &mut |at, x| cell.borrow_mut().w32(at, x), target, ints, floats)
    }
}

const REGIONS: [(u32, u32); 4] = [(ARENA, 0x8000), (0x337c40, 0x40), (0x347e00, 0x60), (0x3a9e00, 0xe400)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn run(direct: bool) {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Calls { seed: 0, n: 0, log: vec![] }));
    let mut leaves = vec![THING_DTOR, DRAWER_COLOUR, DRAW_WIDE_BORDER, DRAW_WIDE, DRAW_NARROW_BORDER, DRAW_NARROW, WIDTH_WIDE, WIDTH_NARROW, LOC_STRING];
    if !direct {
        leaves.push(UPDATE_COLOR);
    }
    for off in (0..0x200).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let (ni, nf, _) = shape(f);
            let ints: Vec<u32> = (0..ni).map(|k| c.gpr(4 + k) as u32).collect();
            let floats: Vec<u32> = (0..nf).map(|k| c.f(12 + k).to_bits()).collect();
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, ints, floats);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    // the helpers they call run as the original: the tween's start (0x2ac348), the thing's start from where it is
    // (0x2ac288), the vector scale (0x102f50), the widget's destructor (0x2adaa8)
    for f in [0x2ac348u32, 0x2ac288, 0x102f50, 0x2adaa8] {
        r.allow(f);
    }
    let methods: Vec<u32> = if direct {
        vec![0x2ad988]
    } else {
        vec![
            0x2ac288, 0x2ad250, 0x2ad2c0, 0x2ad380, 0x2ad7d8, 0x2ad7e0, 0x2ad7e8, 0x2ad828, 0x2ad838, 0x2ad848, 0x2ad960,
            0x2ad938, 0x2ad910, 0x2ad8e8, 0x2ada20, 0x2ada28, 0x2ada30, 0x2ada38, 0x2ada40, 0x2ada48, 0x2ada50, 0x2ada58,
            0x2adaa0, 0x2adaa8, 0x2b1138, 0x2b1328, 0x2b1340, 0x2b1388, 0x2b14b8, 0x2b14e0, 0x2b15d0, 0x2b15e8, 0x2b1638,
            0x2b4a28,
        ]
    };
    let mut g = Rng(if direct { 0x2ad9_0000_0000_0002 } else { 0x2ad9_0000_0000_0001 });
    let mut counts = vec![0u32; methods.len()];
    let fl = |g: &mut Rng| -> u32 {
        match g.below(10) {
            0 => 0,
            1 => 1.0f32.to_bits(),
            2 => (-0.5f32).to_bits(),
            3 => 1.5f32.to_bits(),
            _ => ((g.below(2000) as f32 - 500.0) / 64.0).to_bits(),
        }
    };
    for case in 0..(if direct { 4_000u32 } else { 40_000 }) {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, g.next());
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x724, REND);
        r.mem.write_u32(APP + 0x50, g.next());
        r.mem.write_u32(REND + 0x10, VT);
        for off in (0..0x200).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let w = W;
        r.mem.write_u32(w + 0xc, VT);
        for o in [0x10u32, 0x14, 0x18, 0x1c, 0x30, 0xa0, 0xa4, 0xa8, 0x114, 0x11c] {
            r.mem.write_u32(w + o, g.below(2));
        }
        r.mem.write_u32(w + 0x20, g.below(8));
        r.mem.write_u32(w + 0x24, g.below(6));
        r.mem.write_u32(w + 0x28, [0u32, 1, 3, 5][g.below(4) as usize]);
        r.mem.write_u32(w + 0x2c, g.below(4));
        for o in (0x40u32..0x110).step_by(4) {
            let v = fl(&mut g);
            r.mem.write_u32(w + o, v);
        }
        r.mem.write_u32(w + 0x110, FONT);
        r.mem.write_u32(FONT + 0xc, g.below(30));
        r.mem.write_u32(w + 0x118, fl(&mut g));
        r.mem.write_u32(w + 0x120, [u32::MAX, 0, 5][g.below(3) as usize]);
        r.mem.write_u32(w + 0x124, [0, STRS, STRS + 0x40, STRS + 0x81][g.below(4) as usize]);
        r.mem.write_u32(STRS, 0);
        r.mem.write_u32(STRS + 0x40, [0u32, 0x41, 0x4100][g.below(3) as usize]);
        for k in 0..4 {
            r.mem.write_u32(BUF + 4 * k, fl(&mut g));
        }
        let which = g.below(methods.len() as u32) as usize;
        let addr = methods[which];
        counts[which] += 1;
        let fs = [fl(&mut g), fl(&mut g), fl(&mut g), fl(&mut g)];
        let (a1, a2, a3) = match addr {
            0x2ad828 | 0x2ad838 | 0x2ad960 | 0x2ad938 | 0x2ad910 | 0x2ad8e8 => (BUF, 0, 0),
            0x2ad848 => (BUF + 0x40, BUF + 0x60, 0),
            0x2b1138 => (g.next(), g.next(), g.next()),
            0x2b14e0 => (BUF + 0x80, 0, 0),
            0x2b15d0 | 0x2b1638 => (STRS + 0x40, 0, 0),
            0x2ada58 => (g.next(), g.next(), 0),
            _ => (g.below(3), g.next(), 0),
        };
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.n = 0;
            b.log.clear();
        }
        let mut m = snap(&mut r);
        r.cpu.set_gpr(4, w as u64);
        r.cpu.set_gpr(5, a1 as i32 as i64 as u64);
        r.cpu.set_gpr(6, a2 as i32 as i64 as u64);
        r.cpu.set_gpr(7, a3 as i32 as i64 as u64);
        for (k, v) in fs.iter().enumerate() {
            r.cpu.set_f(12 + k, f32::from_bits(*v));
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().n = 0;
        let e = &mut Ours(a.clone());
        let mine_ret: Option<u32> = match addr {
            0x2ac288 => {
                thing_start_from_current(&mut m, w, BUF + 0x100);
                None
            }
            0x2ad250 => {
                vf5(&mut m, e, w, BUF + 0x100);
                None
            }
            0x2ad2c0 => {
                vf7(&mut m, e, w);
                None
            }
            0x2ad380 => {
                vf8(&mut m, e, w, BUF + 0x100);
                None
            }
            0x2ad7d8 => Some(vf15()),
            0x2ad7e0 => Some(owner(&m, w)),
            0x2ad7e8 => {
                size_from(&mut m, e, w, fs, BUF + 0x100);
                None
            }
            0x2ad828 => {
                vf38(&mut m, w, a1);
                None
            }
            0x2ad838 => {
                vf37(&mut m, w, a1);
                None
            }
            0x2ad848 => {
                vf36(&mut m, e, w, a1, a2, BUF + 0x100);
                None
            }
            0x2ad960 | 0x2ad938 | 0x2ad910 | 0x2ad8e8 => {
                let at = match addr {
                    0x2ad960 => 0xac,
                    0x2ad938 => 0xbc,
                    0x2ad910 => 0xcc,
                    _ => 0xdc,
                };
                set_colour_set(&mut m, w, at, a1);
                None
            }
            0x2ad988 => {
                update_colour(&mut m, e, w);
                None
            }
            0x2ada20 | 0x2ada28 | 0x2ada30 => {
                let at = match addr {
                    0x2ada20 => 0xa8,
                    0x2ada28 => 0xa4,
                    _ => 0xa0,
                };
                set_flag(&mut m, e, w, at, a1);
                None
            }
            0x2ada38 => Some(flag(&m, w, 0xa8)),
            0x2ada40 => Some(flag(&m, w, 0xa4)),
            0x2ada48 => Some(flag(&m, w, 0xa0)),
            0x2ada50 => Some(flag(&m, w, 0xf0)),
            0x2ada58 => {
                post_event(&mut m, e, w, a1, a2);
                None
            }
            0x2adaa0 => Some(vf28()),
            0x2adaa8 => Some(destroy(&mut m, e, w, a1)),
            0x2b1138 => Some(text_construct(&mut m, e, w, a1, a2, a3)),
            0x2b1328 => Some(text_destroy(&mut m, e, w, a1)),
            0x2b1340 => {
                text_vf39(&mut m, e, w);
                None
            }
            0x2b1388 => {
                text_draw(&mut m, e, w);
                None
            }
            0x2b14b8 => {
                text_vf12(&mut m, e, w);
                None
            }
            0x2b14e0 => {
                text_size(&mut m, e, w, a1);
                None
            }
            0x2b15d0 => {
                text_set_wide(&mut m, w, a1);
                None
            }
            0x2b15e8 => {
                text_set_id(&mut m, e, w, a1);
                None
            }
            0x2b1638 => {
                text_set_narrow(&mut m, w, a1);
                None
            }
            _ => Some(text_vf43()),
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        // (track 2's end w is the VU register's leftover)
        let skip = |at: u32| (BUF + 0x100..BUF + 0x200).contains(&at) || (addr == 0x2ad2c0 && at >= w + 0x5c && at < w + 0x60);
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !skip(at + i as u32)) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by function: {counts:?}");
}

#[test]
fn widget_and_text_match_the_original() {
    run(false);
}

#[test]
fn colour_update_matches_the_original() {
    run(true);
}
