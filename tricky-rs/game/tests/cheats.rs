//! The button cheats (board row E10) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `cCheatCodes::Update` 0x26cf60 frame after frame on a front-end player with random presses, chords held and
//! let go, and the real codes typed (right, wrong, too long), the pad's rumble and the cheat sound logged; the
//! player, the four cheat switches and the calls compared with `tricky_game::cheats` each frame.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::cheats::{CheatEnv, CheatPad, Cheats, FePlayer, CHARACTERS, CHORD, MALLORA, MAX_ATTRIBUTES, UNLOCK_EVERYTHING};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    RumbleOn,
    Rumble(u32),
    Sound,
}

struct Env {
    pad: CheatPad,
    log: Vec<Ev>,
}

impl CheatEnv for Env {
    fn pad(&self, _: u32) -> CheatPad {
        self.pad
    }
    fn rumble_on(&mut self, _: u32) {
        self.log.push(Ev::RumbleOn);
    }
    fn rumble(&mut self, _: u32, s: u32) {
        self.log.push(Ev::Rumble(s));
    }
    fn cheat_sound(&mut self) {
        self.log.push(Ev::Sound);
    }
}

const GLOBALS: [u32; 4] = [0x3352a0, 0x3352ac, 0x3352b0, 0x3352b8];

#[test]
fn cheats_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x26cf60, 0x26cef8, 0x26d1c8, 0x26cdc8, 0x16a998, 0x2fb220] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let dev = r.mem.alloc(0x40);
    let dvt = r.mem.alloc(0x40);
    let fns = r.mem.alloc(0x20);
    r.mem.write_u32(dev + 0x38, dvt);
    r.mem.write_u32(dvt + 0x30, 0);
    r.mem.write_u32(dvt + 0x34, fns);
    r.mem.write_u32(dvt + 0x20, 0);
    r.mem.write_u32(dvt + 0x24, fns + 8);
    let l = log.clone();
    r.stub(fns, move |c, _| {
        assert_eq!(c.gpr(5), 1);
        l.borrow_mut().push(Ev::RumbleOn)
    });
    let l = log.clone();
    r.stub(fns + 8, move |c, _| {
        assert_eq!(c.gpr(5), 1);
        l.borrow_mut().push(Ev::Rumble(c.gpr(6) as u32))
    });
    r.stub(0x17a830, move |c, _| c.set_gpr(2, dev as u64));
    r.stub(0x23d5d0, |_, _| {});
    let app = r.mem.alloc(0x800);
    let fe = r.mem.alloc(0x100);
    let snd = r.mem.alloc(0x20);
    let svt = r.mem.alloc(0x60);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x734, fe);
    r.mem.write_u32(fe + 0xa4, snd);
    r.mem.write_u32(snd, svt);
    r.mem.write_u32(svt + 0x50, 0);
    r.mem.write_u32(svt + 0x54, fns + 0x10);
    let l = log.clone();
    r.stub(fns + 0x10, move |c, _| {
        assert_eq!(c.gpr(5), 3);
        l.borrow_mut().push(Ev::Sound)
    });
    let pl = r.mem.alloc(0x60);
    let mut g = Rng(0xc4ea_7000_0000_0001);
    let mut cheats = Cheats::default();
    for (k, a) in GLOBALS.iter().enumerate() {
        r.mem.write_u32(*a, 0);
        let _ = k;
    }
    let mut p = FePlayer { device: 1, rumble: 1, rumble_strength: 0x55, takes_codes: 1, ..Default::default() };
    let put = |m: &mut r5900::Mem, p: &FePlayer| {
        m.write_u32(pl, p.input);
        for k in 0..12 {
            m.write_u32(pl + 4 + 4 * k as u32, p.entered[k]);
        }
        m.write(pl + 0x34, &[p.count]).unwrap();
        m.write_u32(pl + 0x38, p.takes_codes);
        m.write_u32(pl + 0x40, p.device);
        m.write_u32(pl + 0x48, p.rumble);
        m.write_u32(pl + 0x4c, p.rumble_strength);
    };
    put(&mut r.mem, &p);
    let codes: Vec<[u32; 12]> = vec![UNLOCK_EVERYTHING, MAX_ATTRIBUTES, MALLORA, CHARACTERS[0]];
    let mut pad = CheatPad { connected: 1, ..Default::default() };
    let mut plan: Vec<u32> = vec![];
    let mut sounds = 0;
    for frame in 0..200000 {
        // the next frame's buttons: typing a code (each press a frame, a frame up between), or noise
        if plan.is_empty() {
            let mut code: Vec<u32> = match g.next() % 5 {
                0..=2 => codes[g.next() as usize % codes.len()].to_vec(),
                _ => (0..(g.next() % 15)).map(|_| 1 << (g.next() % 12)).collect(),
            };
            if g.next() % 6 == 0 && !code.is_empty() {
                code.pop();
            }
            if g.next() % 8 == 0 {
                code.push(0x40);
            }
            for b in code {
                plan.push(CHORD | b);
                plan.push(CHORD);
            }
            plan.push(if g.next() % 4 == 0 { 0x200 } else { 0 });
            plan.push(0);
            plan.reverse();
        }
        pad.prev = pad.buttons;
        pad.buttons = plan.pop().unwrap();
        pad.connected = (g.next() % 50 != 0) as u32;
        pad.input = if g.next() % 7 == 0 { 0x4800 } else { 0 };
        p.takes_codes = (g.next() % 100 != 0) as u32;
        p.rumble = (g.next() % 3 != 0) as u32;
        r.mem.write_u32(pl + 0x38, p.takes_codes);
        r.mem.write_u32(pl + 0x48, p.rumble);
        r.mem.write_u32(dev + 4, pad.connected);
        r.mem.write_u32(dev + 0xc, pad.prev);
        r.mem.write_u32(dev + 0x10, pad.buttons);
        r.mem.write_u32(dev + 0x2c, pad.input);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, pl as u64);
        r.call(0x26cf60).unwrap();
        let mut env = Env { pad, log: vec![] };
        p.update_cheats(&mut cheats, &mut env);
        let m = &mut r.mem;
        let mut theirs = FePlayer { input: m.read_u32(pl), takes_codes: m.read_u32(pl + 0x38), device: m.read_u32(pl + 0x40), rumble: m.read_u32(pl + 0x48), rumble_strength: m.read_u32(pl + 0x4c), ..Default::default() };
        for k in 0..12 {
            theirs.entered[k] = m.read_u32(pl + 4 + 4 * k as u32);
        }
        let mut b = [0u8; 1];
        m.read(pl + 0x34, &mut b).unwrap();
        theirs.count = b[0];
        assert_eq!(theirs, p, "frame {frame}: the player");
        let tc = Cheats { unlock_everything: m.read_u32(GLOBALS[0]), max_attrib: m.read_u32(GLOBALS[1]), mallora: m.read_u32(GLOBALS[2]), character: [m.read_u32(GLOBALS[3])] };
        assert_eq!(tc, cheats, "frame {frame}: the cheats");
        assert_eq!(*log.borrow(), env.log, "frame {frame}: calls");
        sounds += env.log.iter().filter(|e| **e == Ev::Sound).count();
    }
    eprintln!("{sounds} codes taken; cheats now {cheats:?}");
}
