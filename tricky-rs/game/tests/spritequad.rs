//! `hudsprites::draw_rect_scaled` / `draw_anchored` (board row G8j) against `Sprite_DrawRectScaled` 0x14eed0 and
//! `Sprite_DrawAnchored` 0x1fc178 in the function runner (TRICKY_ELF; skipped without it): random rectangles, flags
//! and placements, the texture bind and the quad submitted logged on both sides (floats bit for bit).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::hudsprites::*;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;

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

type L = Rc<RefCell<Vec<(u32, Vec<u32>)>>>;
const R_FN: u32 = 0x7e0000;

struct Ours(L);
impl SpritesEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let mut v = ints.to_vec();
        v.extend(floats.iter().map(|f| f.to_bits()));
        self.0.borrow_mut().push((func, v));
        (0, 0.0)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, ints: &[u32], _floats: &[f32]) -> u32 {
        self.0.borrow_mut().push((R_FN + slot, ints.to_vec()));
        0
    }
}

const F: [f32; 12] = [0.0, 1.0, 0.5, 2.25, 12.0, -3.5, 100.0, 0.75, 37.5, 0.1, 0.3, 1.3];

#[test]
fn sprite_quads_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(0x400);
    let (app, ren, vt, rect) = (base, base + 0x100, base + 0x200, base + 0x300);
    let l: L = Rc::new(RefCell::new(vec![]));
    for f in [0x19b730u32, 0x19bdf8] {
        let l2 = l.clone();
        r.stub(f, move |c, _| {
            let mut v = vec![c.gpr(4) as u32];
            v.extend((0..8).map(|k| c.f(12 + k).to_bits()));
            l2.borrow_mut().push((f, v));
        });
    }
    let l2 = l.clone();
    r.stub(R_FN + 0x1f4, move |c, _| l2.borrow_mut().push((R_FN + 0x1f4, vec![c.gpr(5) as u32, c.gpr(6) as u32])));
    let mut g = Rng(0x1fc1_78ee_8e8e_0001);
    for case in 0..60_000u32 {
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x724, ren);
        r.mem.write_u32(ren + 0x10, vt);
        r.mem.write_u32(vt + 0x1f4, R_FN + 0x1f4);
        r.mem.write_u16(vt + 0x1f0, 0);
        r.mem.write_u32(rect, g.next());
        for k in 1..7 {
            r.mem.write_u32(rect + 4 * k, F[g.below(F.len() as u32) as usize].to_bits());
        }
        let flags = g.next() & 0x7f;
        let v: Vec<f32> = (0..4).map(|_| F[g.below(F.len() as u32) as usize]).collect();
        let scaled = g.below(2) == 0;
        for (k, x) in v.iter().enumerate() {
            r.cpu.set_f(12 + k, *x);
        }
        r.cpu.set_gpr(4, rect as u64);
        r.cpu.set_gpr(5, flags as u64);
        l.borrow_mut().clear();
        r.call(if scaled { 0x14eed0 } else { 0x1fc178 }).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        let mut m = Mem::default();
        let mut b = vec![0u8; 0x400];
        r.mem.read(base, &mut b).unwrap();
        m.regions.push((base, b));
        let mut gb = vec![0u8; 4];
        r.mem.read(GAPP, &mut gb).unwrap();
        m.regions.push((GAPP, gb));
        let e = &mut Ours(l.clone());
        if scaled {
            draw_rect_scaled(&mut m, e, rect, flags, v[0], v[1], v[2], v[3]);
        } else {
            draw_anchored(&mut m, e, rect, flags, v[0], v[1], v[2], v[3]);
        }
        let mine = std::mem::take(&mut *l.borrow_mut());
        assert_eq!(theirs, mine, "case {case} (flags {flags:#x}, scaled {scaled})");
    }
}
