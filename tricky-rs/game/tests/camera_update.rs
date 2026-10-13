//! `tricky_game::camera_update` against `Game_UpdateCameras` 0x17f390 in the function runner (TRICKY_ELF; skipped
//! without it), with the entry lookup `FUN_00173c18` stubbed to a table.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::camera_update::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 20001) as f32 / 10000.0 - 1.0
    }
    fn ang(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => 1.570_796_4,
            2 => -1.570_796_4,
            3 => 3.141_592_7,
            _ => self.unit() * 7.0,
        }
    }
}

struct Env(Vec<CamEntry>);
impl EntryEnv for Env {
    fn entry(&mut self, id: u32) -> CamEntry {
        self.0[id as usize]
    }
}

#[test]
fn camera_update_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x250c10u32, 0x102f80, 0x137140, 0x137108, 0x17f390] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0x400);
    let table = r.mem.alloc(0x40 * 8);
    let entries: Rc<RefCell<Vec<CamEntry>>> = Rc::new(RefCell::new(vec![]));
    let e2 = entries.clone();
    r.stub(0x173c18, move |c, _| {
        let id = c.gpr(4) as u32;
        assert!(id < 8);
        c.set_gpr(2, (table + 0x40 * id) as u64);
    });
    // the identity matrix at 0x346410 (zero in the file; the game's start-up code fills it)
    for (k, v) in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0].iter().enumerate() {
        r.mem.write_f32(0x346410 + 4 * k as u32, *v);
    }
    let mut g = Rng(0x17f3_90c5_2a64_b1d7);
    let mut slots = 0;
    for case in 0..30_000 {
        let n = (g.next() % 5) as i32;
        let es: Vec<CamEntry> = (0..8)
            .map(|_| CamEntry {
                pos: [g.unit() * 5000.0, g.unit() * 5000.0, g.unit() * 800.0],
                yaw: g.ang(),
                pitch: g.ang(),
                roll: g.ang(),
                fov: [0.0f32, 0.1, 1.0, 2.0, 3.0, 5.0, 0.2][(g.next() % 7) as usize] * (1.0 + g.unit() * 0.2),
                near: [0.0f32, 0.5, 1.0, 2.0, 30.0][(g.next() % 5) as usize] + (g.next() % 3) as f32 * 0.25,
                w20: g.next(),
            })
            .collect();
        *e2.borrow_mut() = es.clone();
        for (k, e) in es.iter().enumerate() {
            let a = table + 0x40 * k as u32;
            for (j, v) in [e.pos[0], e.pos[1], e.pos[2], e.yaw, e.pitch, e.roll, e.fov, e.near].iter().enumerate() {
                r.mem.write_f32(a + 4 * j as u32, *v);
            }
            r.mem.write_u32(a + 0x20, e.w20);
        }
        let mut o: Vec<u32> = (0..0x2a0 / 4 + 1).map(|_| g.next()).collect();
        o[0x298 / 4] = n as u32;
        for s in 0..4 {
            o[(0xa0 + 0x80 * s) / 4] = g.next() % 8;
        }
        for (i, v) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *v);
        }
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x17f390).unwrap();
        let mut env = Env(es);
        update_cameras(&mut o, &mut env);
        for (i, v) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *v, "case {case} word {i:#x} ({:#x} in a slot)", (i * 4).wrapping_sub(0xa0) % 0x80);
        }
        slots += n;
    }
    eprintln!("camera update: {slots} slots");
}
