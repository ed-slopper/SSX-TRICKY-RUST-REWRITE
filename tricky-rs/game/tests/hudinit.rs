//! `tricky_game::hudstate::hud_overlay_new` (board row G8j) against `HudOverlay_InitTables` 0x19e888 in the function
//! runner (TRICKY_ELF; skipped without it): random game modes, courses, player counts, source tables and starting
//! memory, every call out logged on both sides and all memory compared (the bands, the three view layouts, the HUD).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::hudstate::*;
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

struct Ours(L);
impl HudEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.borrow_mut().push((func, args.to_vec()));
        0
    }
    fn multiplier(&mut self, _m: &mut Mem, _score: u32) -> f32 {
        unreachable!()
    }
    fn progress(&mut self, _m: &mut Mem, _rider: u32) -> f32 {
        unreachable!()
    }
    fn renderer(&mut self, _m: &mut Mem, _slot: u32, _args: &[u32]) -> u32 {
        unreachable!()
    }
    fn draw_player(&mut self, _m: &mut Mem, _func: u32, _x: f32, _y: f32, _w: f32, _h: f32, _hud: u32, _i: u32) {
        unreachable!()
    }
}

/// The bands, the view layouts, the static source tables, gApp, the course / game mode, the players
const GLOBALS: [(u32, u32); 6] =
    [(BANDS, 14 * 20), (LAYOUTS, 3 * LAYOUT_SIZE), (0x38d060, 0x978), (GAPP, 4), (LAP_MODE, 8), (PLAYERS, 4)];
const ARENA: u32 = 0x1000;

fn snapshot(r: &mut Runner, base: u32) -> Mem {
    let mut m = Mem::default();
    for (at, n) in GLOBALS.iter().copied().chain([(base, ARENA)]) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

#[test]
fn hud_overlay_new_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(ARENA);
    let (hud, app, race, world) = (base, base + 0x700, base + 0xe80, base + 0xf00);
    let l: L = Rc::new(RefCell::new(vec![]));
    for f in [0x1a7d78u32, 0x1a1968] {
        let l2 = l.clone();
        r.stub(f, move |c, _| l2.borrow_mut().push((f, vec![c.gpr(4) as u32])));
    }
    // memset (its own code uses PCPYH, not in the runner): only the frame's temporary, which is not compared
    r.stub(0x2fb468, |c, m| {
        let (at, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
        for k in 0..n {
            m.write_u8(at + k, v);
        }
    });
    let mut g = Rng(0x19e8_88ee_8e8e_0001);
    for case in 0..20_000u32 {
        for (at, n) in GLOBALS.iter().copied().chain([(base, ARENA)]) {
            for k in 0..n / 4 {
                r.mem.write_u32(at + 4 * k, if g.below(2) == 0 { g.next() } else { [1.0f32, 0.5, 300.0][g.below(3) as usize].to_bits() });
            }
        }
        r.mem.write_u32(GAPP, app);
        r.mem.write_u32(app + 0x730, race);
        r.mem.write_u32(race + 0x1c, world);
        r.mem.write_u32(LAP_MODE, g.below(13));
        r.mem.write_u32(GAME_MODE, g.below(11));
        r.mem.write_u32(PLAYERS, [0, 1, 2, 4, u32::MAX][g.below(5) as usize]);
        r.mem.write_u32(world + 0x7c, g.below(3));
        r.mem.write_u32(world + 0x84, g.below(3));
        let mut m = snapshot(&mut r, base);
        r.cpu.set_gpr(4, hud as u64);
        l.borrow_mut().clear();
        r.call(0x19e888).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut *l.borrow_mut());
        assert_eq!(r.cpu.gpr(2) as u32, hud);
        let back = hud_overlay_new(&mut m, &mut Ours(l.clone()), hud);
        assert_eq!(back, hud);
        let mine = std::mem::take(&mut *l.borrow_mut());
        assert_eq!(theirs, mine, "case {case}");
        let them = snapshot(&mut r, base);
        for ((at, x), (_, y)) in them.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                let i = i & !3;
                panic!("case {case}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..i + 4], &y[i..i + 4]);
            }
        }
    }
}
