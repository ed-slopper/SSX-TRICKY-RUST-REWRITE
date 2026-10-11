//! Our landing check against the original's, bit for bit (board row E8a1): the game's own `Landing_CheckAngles`
//! 0x12ba78 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), with
//! `Vec4_Scale`, `Math_Asin` and `Math_Atan` run as the original, the clip queries answering what we pick and
//! `Boarder_SetState` recorded, and `tricky_game::landing::check_angles` on the same rider.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::landing::{check_angles, LandingState};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    fn unit(&mut self) -> [f32; 4] {
        let (x, y, z) = (self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0));
        let l = (x * x + y * y + z * z).sqrt().max(1e-3);
        [x / l, y / l, z / l, 0.0]
    }
}

fn put(r: &mut Runner, at: u32, v: [f32; 4]) {
    for (i, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * i as u32, *x);
    }
}

fn get(r: &mut Runner, at: u32) -> Vec<u32> {
    (0..4).map(|i| r.mem.read_u32(at + 4 * i)).collect()
}

#[test]
fn landing_check_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x102f50, 0x250fd0, 0x2510f8] {
        r.allow(f); // Vec4_Scale, Math_Asin, Math_Atan
    }
    let clip = Rc::new(RefCell::new((0i64, false)));
    let state = Rc::new(RefCell::new(Vec::<u64>::new()));
    let (c1, c2, st) = (clip.clone(), clip.clone(), state.clone());
    r.stub(0x15eec0, move |cpu, _| cpu.set_gpr(2, c1.borrow().0 as u64));
    r.stub(0x15ffb8, move |cpu, _| cpu.set_gpr(2, c2.borrow().1 as u64));
    r.stub(0x11f608, move |cpu, _| st.borrow_mut().push(cpu.gpr(5)));
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let nrm = r.mem.alloc(16);
    let out = r.mem.alloc(8);
    r.mem.write_u32(rd + 0x464, stats);
    let mut g = Rng(0x1a4d_1a4d_1a4d_1a4d);
    let (mut landed, mut ok) = (0, 0);
    for case in 0..8000 {
        let s = LandingState {
            clip_type: [0x1, 0xe, 0xf, 0x10, 0x3][(g.next() % 5) as usize],
            clip_flag4: g.next() % 4 != 0,
            surface: [1, 2, 3, 0, 6, 10, 5, 18][(g.next() % 8) as usize],
            board: g.unit(),
            vel: [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-2000.0, 500.0), 0.0],
            up: g.unit(),
            stability: g.next() as u8,
        };
        let n = if case % 50 == 0 { [0.0, 0.0, 1.0, 0.0] } else { g.unit() };
        *clip.borrow_mut() = (s.clip_type, s.clip_flag4);
        state.borrow_mut().clear();
        r.mem.write_u32(rd + 0x290, s.surface);
        put(&mut r, rd + 0x180, s.board);
        put(&mut r, rd + 0x150, s.vel);
        put(&mut r, rd + 0x1a0, s.up);
        put(&mut r, nrm, n);
        r.mem.write_u8(stats + 0x16, s.stability);
        for o in [0x320u32, 0x330] {
            put(&mut r, rd + o, [f32::from_bits(0xdead_beef); 4]);
        }
        r.mem.write_u32(out, 0xdead_beef);
        r.mem.write_u32(out + 4, 0xdead_beef);
        r.cpu.set_gpr(4, rd as u64);
        r.cpu.set_gpr(5, nrm as u64);
        r.cpu.set_gpr(6, out as u64);
        r.cpu.set_gpr(7, (out + 4) as u64);
        r.call(0x12ba78).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let game_ok = r.cpu.gpr(2) != 0;
        match check_angles(&s, n) {
            None => {
                assert!(!game_ok, "case {case}: the game landed");
                assert!(state.borrow().is_empty(), "case {case}: no state change");
                assert_eq!(r.mem.read_u32(out), 0xdead_beef, "case {case}: pitch untouched");
            }
            Some(l) => {
                landed += 1;
                ok += l.ok as u32;
                assert_eq!(*state.borrow(), vec![1], "case {case}: state 1");
                assert_eq!(get(&mut r, rd + 0x320), l.forward.map(f32::to_bits).to_vec(), "case {case} {s:?} n {n:?}: forward");
                assert_eq!(get(&mut r, rd + 0x330), l.side.map(f32::to_bits).to_vec(), "case {case}: side");
                assert_eq!(r.mem.read_u32(out), l.pitch.to_bits(), "case {case}: pitch game {} ours {}", r.mem.read_f32(out), l.pitch);
                assert_eq!(r.mem.read_u32(out + 4), l.roll.to_bits(), "case {case}: roll game {} ours {}", r.mem.read_f32(out + 4), l.roll);
                assert_eq!(game_ok, l.ok, "case {case}: clean landing");
            }
        }
    }
    assert!(landed > 2000 && ok > 200 && ok < landed, "landed {landed}, clean {ok}");
}
