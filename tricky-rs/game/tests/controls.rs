//! The pad word against the original's (board row G1a5b1): `cPlayer::GetControls` 0x151250 run in the function
//! runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with the pad mapper's queries, the overlay
//! check, the slope time scale and the random easy trick answered and recorded, against
//! `tricky_game::controls::get_controls` on the same answers: the word, the trick slot and the calls.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controls::{get_controls, ControlsIn, PadMapper};

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
}

/// One case's answers: per query kind and action, and per axis.
#[derive(Clone)]
struct Pad {
    axes: [f32; 16],
    q: [[u32; 0x30]; 4],
}
impl Default for Pad {
    fn default() -> Self {
        Pad { axes: [0.0; 16], q: [[0; 0x30]; 4] }
    }
}
impl PadMapper for Pad {
    fn axis(&mut self, a: u32) -> f32 {
        self.axes[a as usize]
    }
    fn q0c(&mut self, a: u32) -> u32 {
        self.q[0][a as usize]
    }
    fn q14(&mut self, a: u32) -> u32 {
        self.q[1][a as usize]
    }
    fn q1c(&mut self, a: u32) -> u32 {
        self.q[2][a as usize]
    }
    fn q24(&mut self, a: u32) -> u32 {
        self.q[3][a as usize]
    }
}

const GAPP: u32 = 0x337c58;
const VCALL: u32 = 0x1000;

#[test]
fn pad_word_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x2cc808); // EAMem_Set
    let pad: Rc<RefCell<Pad>> = Rc::default();
    for (addr, k) in [(0x179758u32, 0usize), (0x179780, 1), (0x1797a8, 2), (0x1797d0, 3)] {
        let p = pad.clone();
        r.stub(addr, move |c, _| {
            let a = c.gpr(5) as usize;
            c.set_gpr(2, p.borrow().q[k][a] as u64);
        });
    }
    let p = pad.clone();
    r.stub(0x1797f8, move |c, _| {
        let a = c.gpr(5) as usize;
        c.set_f(0, p.borrow().axes[a]);
    });
    let overlay = Rc::new(RefCell::new(0u64));
    let o = overlay.clone();
    r.stub(0x1a77a8, move |c, _| c.set_gpr(2, *o.borrow()));
    let slope = Rc::new(RefCell::new(0u32));
    let s = slope.clone();
    r.stub(0x153130, move |_, _| *s.borrow_mut() += 1);
    let easy = Rc::new(RefCell::new((0u32, 0u32)));
    let e = easy.clone();
    r.stub(0x11d3d8, move |c, _| {
        e.borrow_mut().1 += 1;
        c.set_gpr(2, e.borrow().0 as u64);
    });
    // the boarder's debug-record virtual answers none
    r.stub(VCALL, |c, _| c.set_gpr(2, 0));
    let player = r.mem.alloc(0x900);
    let boarder = r.mem.alloc(0x500);
    let vt = r.mem.alloc(0x100);
    let mapper = r.mem.alloc(0x300);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let race = r.mem.alloc(0x100);
    let word = r.mem.alloc(8);
    r.mem.write_u32(player, boarder);
    r.mem.write_u32(player + 0x870, mapper);
    r.mem.write_u32(player + 0x878, 0);
    r.mem.write_u32(boarder, vt);
    r.mem.write_u16(vt + 0x38, 0);
    r.mem.write_u32(vt + 0x3c, VCALL);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x34, 0);

    let mut g = Rng(0x6a3d_51c0_9e27_4f13);
    let mut seen = [0u32; 0x18];
    for case in 0..40_000 {
        let state = if g.next() % 8 == 0 { g.next() % 0x18 } else { [2, 3, 0xb, 0xd, 0xd, 0xe, 0xf, 0x10, 0x10, 0x10, 8, 10, 0x13, 0x14, 0x15][g.next() as usize % 15] };
        let mut pd = Pad::default();
        for a in pd.axes.iter_mut() {
            *a = match g.next() % 6 {
                0 => 0.0,
                1 => [0.5, -0.5, 1.0, -1.0, 0.50001, -0.50001][g.next() as usize % 6],
                _ => g.f(-1.0, 1.0),
            };
        }
        let dense = g.next() % 3;
        for k in 0..4 {
            for a in 0..0x30 {
                pd.q[k][a] = if g.next() % (3 + 10 * dense) == 0 { [1, 2, 3, 0xff][g.next() as usize % 4] } else { 0 };
            }
        }
        let vis = g.next() % 6 == 0;
        let race_ptr = if g.next() % 2 == 0 { 0 } else { race };
        let running = g.next() % 2;
        let trick0 = if g.next() % 2 == 0 { 0x1a } else { g.next() % 0x1a };
        let easy_answer = g.next() % 0x1a;
        *pad.borrow_mut() = pd.clone();
        *overlay.borrow_mut() = vis as u64;
        *slope.borrow_mut() = 0;
        *easy.borrow_mut() = (easy_answer, 0);
        r.mem.write_u32(boarder + 0x428, state);
        r.mem.write_u32(boarder + 0x424, g.next() % 8);
        r.mem.write_u32(game + 0x40, race_ptr);
        r.mem.write_u32(race + 0x54, running);
        r.mem.write_u32(player + 0x868, trick0);
        r.mem.write_u32(word, 0xdead_beef);
        r.cpu.set_gpr(4, player as u64);
        r.cpu.set_gpr(5, word as u64);
        r.call(0x151250).unwrap_or_else(|e| panic!("case {case} state {state:#x}: {e}"));

        let paused = vis && (race_ptr == 0 || running == 0);
        let mut trick = trick0;
        let mut easy_calls = 0;
        let mut sl = false;
        let ours = get_controls(ControlsIn { state, paused }, &mut pd, &mut trick, &mut || { easy_calls += 1; easy_answer }, &mut sl);
        let theirs = r.mem.read_u32(word);
        assert_eq!(theirs, ours, "case {case} state {state:#x}: word {theirs:#010x} vs ours {ours:#010x}");
        assert_eq!(r.mem.read_u32(player + 0x868), trick, "case {case} state {state:#x}: trick slot");
        assert_eq!(easy.borrow().1, easy_calls, "case {case} state {state:#x}: random easy trick calls");
        assert_eq!(*slope.borrow(), sl as u32, "case {case} state {state:#x}: slope time scale");
        seen[state as usize] += (!paused) as u32;
    }
    for st in [2, 3, 0xb, 0xd, 0xe, 0xf, 0x10, 8, 10, 0x13, 0x14, 0x15] {
        assert!(seen[st] > 20, "state {st:#x} seen {} times", seen[st]);
    }
}
