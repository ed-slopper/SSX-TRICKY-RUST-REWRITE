//! `tricky_game::renderstate` (board row G7k) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case one of the renderer's state virtuals on a fresh random renderer (its texture state
//! set to what is asked or not, its stack, its view records, camera and clip matrix, its screen words) with
//! random words. The matrix update, the texture manager's functions (which move the packet place on), the submit
//! and the virtuals called are logging stand-ins; the box test runs the game's VU0 micro program. The memory,
//! the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::renderstate as rs;
use tricky_game::riderdraw::RiderDrawEnv;

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
const R: u32 = ARENA;
const RVT: u32 = ARENA + 0x2000;
const TS: u32 = ARENA + 0x2400;
const CAM: u32 = ARENA + 0x2600;
const BZ: u32 = ARENA + 0x2700;
const BZVT: u32 = ARENA + 0x2780;
const PLACE: u32 = ARENA + 0x2900;
const BOX: u32 = ARENA + 0x2a00;
const PKT: u32 = ARENA + 0x3000;
const ANSWER: u32 = 0x77;
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}
const CALLS: [u32; 5] = [rs::MATRIX_UPDATE, rs::LOCK_TEXTURE, rs::TARGET_PACKET, rs::CLEAR_PACKET, rs::SUBMIT];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

/// A call's log and what it does: the texture manager's packet makers move the packet place (their last word)
/// on 0x30
fn act(func: u32, w: &[u32], read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32)) -> (Vec<u32>, u32) {
    match func {
        rs::TARGET_PACKET => {
            let p = read(w[2]);
            write(w[2], p + 0x30);
            (w[..3].to_vec(), 0)
        }
        rs::CLEAR_PACKET => {
            let p = read(w[3]);
            write(w[3], p + 0x30);
            (w[..4].to_vec(), 0)
        }
        rs::LOCK_TEXTURE => (w[..4].to_vec(), ANSWER),
        rs::MATRIX_UPDATE => (w[..1].to_vec(), 0),
        rs::SUBMIT => (w[..4].to_vec(), 0),
        f if f == vaddr(0x1f0) => (w[..3].to_vec(), 0),
        _ => (w[..5].to_vec(), if func == vaddr(0x90) { ANSWER + 1 } else { 0 }),
    }
}

struct Ours(Vec<Call>);
impl RiderDrawEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let m2 = std::cell::RefCell::new(m);
        let (l, a) = act(func, &w, &mut |at| m2.borrow().r32(at), &mut |at, v| m2.borrow_mut().w32(at, v));
        self.0.push(Call(func, l));
        a
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        let mut w = vec![this];
        w.extend(args);
        w.resize(8, 0);
        let (l, a) = act(func, &w, &mut |_| 0, &mut |_, _| {});
        self.0.push(Call(func, l));
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn renderer_state_virtuals_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.load_vu0_chain(0x31cac0).unwrap();
    for f in [0x1dcc10u32, 0x1e8958, 0x1e9970, 0x1e1648, 0x1e14b8, 0x1e1510, 0x1e1668, 0x1e9698, 0x1e9670, 0x1e94e8, 0x1e9478, 0x1e9458, 0x1e9448, 0x1e9418, 0x1e9378, 0x1e9338, 0x1e91e8, 0x1e9098, 0x1e1e10, 0x1e9ef0, 0x1e8fd8] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in CALLS.into_iter().chain([vaddr(0x1f0), vaddr(0x90)]) {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let m2 = std::cell::RefCell::new(mem);
            let (lg, a) = act(func, &w, &mut |at| m2.borrow_mut().read_u32(at), &mut |at, v| m2.borrow_mut().write_u32(at, v));
            l.borrow_mut().push(Call(func, lg));
            c.set_gpr(2, a as u64);
        });
    }
    let mut g = Rng(0x57a7_0000_0000_0001);
    let mut seen = [0usize; 21];
    for case in 0..21 * 600u32 {
        let which = case % 21;
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(R + 0x10, RVT);
        m.w16(RVT + 0x1f0, [0u16, 0x10, 0xfff0][g.below(3) as usize]);
        m.w32(RVT + 0x1f4, vaddr(0x1f0));
        m.w32(R + 0x484, BZ);
        m.w32(BZ, BZVT);
        m.w16(BZVT + 0x90, [0u16, 8][g.below(2) as usize]);
        m.w32(BZVT + 0x94, vaddr(0x90));
        m.w32(R + 0x470, TS + 0x1c * g.below(3));
        m.w32(R + 0x46c, g.below(4));
        m.w32(R + 0x680, PKT + 0x10 * g.below(8));
        m.w32(R + 0xe4, g.below(3));
        m.w32(R + 0x3f4, CAM);
        m.w32(R + 0x6a4, [0x31u32, 0, 0x30][g.below(3) as usize]);
        for k in 0..3u32 {
            let rec = R + 0x14 + 0x34 * k;
            wf(&mut m, rec + 0x20, [0.0f32, 1.5, g.f(-3.0, 3.0)][g.below(3) as usize]);
            wf(&mut m, rec + 0x1c, g.f(0.3, 1.5));
        }
        // the clip matrix and camera: a perspective with depth −1..1 over a near and far
        for i in 0..4u32 {
            for j in 0..4u32 {
                let v = match (i, j) {
                    (0, 0) | (1, 1) => g.f(0.5, 2.0),
                    (2, 2) => g.f(0.9, 1.1),
                    (2, 3) => 1.0,
                    (3, 2) => g.f(-20.0, -5.0),
                    _ => if g.below(2) == 0 { 0.0 } else { g.f(-0.1, 0.1) },
                };
                wf(&mut m, R + 0x5b0 + 0x10 * i + 4 * j, v);
                wf(&mut m, CAM + 0x10 * i + 4 * j, if i == j { 1.0 } else { g.f(-0.5, 0.5) });
            }
        }
        wf(&mut m, R + 0x5f8, g.f(1000.0, 30000.0));
        for k in 0..4 {
            wf(&mut m, PLACE + 4 * k, if k == 3 { 1.0 } else { g.f(-40.0, 40.0) });
            wf(&mut m, BOX + 4 * k, if k == 3 { 1.0 } else { g.f(-60.0, 20.0) });
            wf(&mut m, BOX + 0x10 + 4 * k, if k == 3 { 1.0 } else { g.f(-20.0, 60.0) });
        }
        wf(&mut m, PLACE + 8, [g.f(5.0, 200.0), 0.5, 20.0, -3.0][g.below(4) as usize]);
        // the texture state: sometimes already what is asked
        let t = m.r32(R + 0x470);
        let (a, b, c) = (g.below(4), g.below(4), g.next());
        if g.below(2) == 0 {
            m.w8(t + 0x11, a as u8);
            m.w8(t + 0xf, a as u8);
            m.w8(t + 0x12, a as u8);
            m.w8(t + 0x13, b as u8);
            m.w16(t + 6, 10);
            m.w8(t + 0x15, a as u8);
            m.w8(t + 0x17, a as u8);
            m.w16(t + 8, [a as u16, 0xffff][g.below(2) as usize]);
            m.w16(t + 0xa, [a as u16, 0xffff][g.below(2) as usize]);
            m.w8(t + 0x14, a as u8);
            m.w8(t + 0x16, a as u8);
            m.w32(t + 0x14, [0x0100_0300u32, 0x0300_0000, 0x0600_0300, 0x0a00_0300, 0x0100_0000, 0x0300_0300][g.below(6) as usize] | (g.next() & 0x00ff_00ff));
        }
        let mode = [u32::MAX, 0, 1, 2, 3, 4, 5, 6, 0x1234][g.below(9) as usize];
        let k = [0.0f32, -35.022, 10.0, g.f(-100.0, 100.0)][g.below(4) as usize];
        let reference = [0.08f32, 0.0, 0.01, g.f(0.0, 1.0)][g.below(4) as usize];
        for (at, bytes) in &m.regions {
            r.mem.write(*at, bytes).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let mut ans = 0u32;
        let (func, regs, f12): (u32, Vec<u32>, f32) = match which {
            0 => {
                rs::depth_offset(&mut m, &mut ours, R, PLACE, F(k));
                (0x1dcc10, vec![R, PLACE], k)
            }
            1 => {
                ans = rs::fov(&m, R).0.to_bits();
                (0x1e8958, vec![R], 0.0)
            }
            2 => {
                ans = rs::camera(&m, R);
                (0x1e9970, vec![R], 0.0)
            }
            3 => {
                ans = rs::lock_texture(&mut m, &mut ours, R, a, b, c);
                (0x1e1648, vec![R, a, b, c], 0.0)
            }
            4 => {
                ans = rs::draw_into(&mut m, &mut ours, R, c);
                (0x1e14b8, vec![R, c], 0.0)
            }
            5 => {
                ans = rs::screen_back(&mut m, &mut ours, R);
                (0x1e1510, vec![R], 0.0)
            }
            6 => {
                rs::clear_texture(&mut m, &mut ours, R, a, c);
                (0x1e1668, vec![R, a, c], 0.0)
            }
            7 => {
                rs::push_state(&mut m, R);
                (0x1e9698, vec![R], 0.0)
            }
            8 => {
                rs::pop_state(&mut m, R);
                (0x1e9670, vec![R], 0.0)
            }
            9 => {
                rs::set_11(&mut m, R, a);
                (0x1e94e8, vec![R, a], 0.0)
            }
            10 => {
                rs::set_test(&mut m, R, a, b, F(reference));
                (0x1e9478, vec![R, a, b], reference)
            }
            11 => {
                rs::set_0f(&mut m, R, a);
                (0x1e9458, vec![R, a], 0.0)
            }
            12 => {
                ans = rs::get_0f(&m, R);
                (0x1e9448, vec![R], 0.0)
            }
            13 => {
                rs::set_blend(&mut m, R, a);
                (0x1e9418, vec![R, a], 0.0)
            }
            14 => {
                let tex = [a, c, 0xffff_ffff, 0x8000][g.below(4) as usize];
                rs::set_texture(&mut m, R, tex, b & 1);
                (0x1e9378, vec![R, tex, b & 1], 0.0)
            }
            15 => {
                rs::set_14(&mut m, R, a, b & 1);
                (0x1e9338, vec![R, a, b & 1], 0.0)
            }
            16 => {
                rs::set_blend_mode(&mut m, R, mode);
                (0x1e91e8, vec![R, mode], 0.0)
            }
            17 => {
                rs::set_lights(&mut m, R, PLACE);
                (0x1e9098, vec![R, PLACE], 0.0)
            }
            18 => {
                ans = rs::box_test(&m, BOX, BOX + 0x10, R + 0x5b0);
                (0x1e1e10, vec![R, BOX, BOX + 0x10, R + 0x5b0], 0.0)
            }
            19 => {
                rs::set_1d90(&mut m, R, c);
                (0x1e9ef0, vec![R, c], 0.0)
            }
            _ => {
                ans = rs::bezier_forward(&mut m, &mut ours, R, &[a, b, c, mode]);
                (0x1e8fd8, vec![R, a, b, c, mode], 0.0)
            }
        };
        // (each word as the EE holds a 32-bit value: sign-extended to its 64-bit register)
        for (i, v) in regs.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f12);
        r.call(func).unwrap_or_else(|e| panic!("case {case} ({func:#x}): {e}"));
        let theirs = log.borrow().clone();
        assert_eq!(theirs, ours.0, "case {case} ({func:#x}): the calls");
        let answers_a_word = matches!(which, 2 | 3 | 4 | 5 | 12 | 18 | 20);
        if answers_a_word {
            let got = r.cpu.gpr(2) as u32;
            let got = if which == 12 { got & 0xff } else { got };
            assert_eq!(got, ans, "case {case} ({func:#x}): the answer");
        }
        if which == 1 {
            assert_eq!(r.cpu.fpr[0], ans, "case {case}: the field of view");
        }
        for (at, bytes) in &m.regions {
            let mut th = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != bytes {
                let i = th.iter().zip(bytes).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} ({func:#x}): memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(bytes));
            }
        }
        seen[which as usize] += (ans != 0 || !theirs.is_empty()) as usize + 1;
    }
    assert!(seen.iter().all(|&n| n > 0));
}
