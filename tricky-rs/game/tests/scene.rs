//! The race scenes' set-up (board row G11b) against the game in the function runner (TRICKY_ELF; skipped
//! without it): `RaceScene_Reset`, `RaceScene_SetupFromPicks`, `RaceScene_Select`, `RaceScene_GetCast`,
//! `FUN_00169948` and `RaceScene_GetScriptName` on random selections and scene objects, with `Rand_Fx`, the race's
//! rider and the clip markers scripted the same: the whole object and every call compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::scene::{RaceScene, SceneEnv, Selection, Slot, NO_RIDER, SCENES, SCRIPTS, SLOTS};

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

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Rand(u32),
    Rider,
    Marker(u32, i32),
}

#[derive(Clone)]
struct World {
    rand: u64,
    rider: u32,
    /// the first marker frame by clip (none: −1)
    frames: Vec<(i32, i32)>,
    log: Vec<Ev>,
}

impl World {
    fn draw(&mut self) -> u32 {
        self.rand ^= self.rand << 13;
        self.rand ^= self.rand >> 7;
        self.rand ^= self.rand << 17;
        let r = (self.rand >> 20) as u32;
        self.log.push(Ev::Rand(r));
        r
    }
    fn frame(&self, clip: i32) -> i32 {
        self.frames.iter().find(|(c, _)| *c == clip).map_or(-1, |(_, f)| *f)
    }
}

impl SceneEnv for World {
    fn race_rider0(&mut self) -> u32 {
        self.log.push(Ev::Rider);
        self.rider
    }
    fn first_marker_frame(&mut self, rider: u32, clip: i32) -> i32 {
        self.log.push(Ev::Marker(rider, clip));
        self.frame(clip)
    }
}

const OBJ_SIZE: u32 = 0x458 + 90 * 4;

fn write_obj(r: &mut Runner, at: u32, o: &RaceScene, sel_at: u32) {
    r.mem.write_u32(at, o.current as u32);
    for i in 0..SCENES {
        for j in 0..SLOTS {
            let s = o.slots[i][j];
            let p = at + 4 + 0x48 * i as u32 + 12 * j as u32;
            r.mem.write_u32(p, s.a as u32);
            r.mem.write_u32(p + 4, s.b as u32);
            r.mem.write_u32(p + 8, s.pick as u32);
        }
    }
    r.mem.write_u32(at + 0x43c, if o.has_selection { sel_at + 0x33c } else { 0 });
    for (k, c) in o.cast.iter().enumerate() {
        r.mem.write_u32(at + 0x440 + 4 * k as u32, *c as u32);
    }
    for (k, c) in o.riders.iter().enumerate() {
        r.mem.write_u32(at + 0x458 + 4 * k as u32, *c as u32);
    }
}

fn read_obj(r: &mut Runner, at: u32, sel_at: u32) -> RaceScene {
    let mut o = RaceScene { current: r.mem.read_u32(at) as i32, ..Default::default() };
    for i in 0..SCENES {
        for j in 0..SLOTS {
            let p = at + 4 + 0x48 * i as u32 + 12 * j as u32;
            o.slots[i][j] = Slot { a: r.mem.read_u32(p) as i32, b: r.mem.read_u32(p + 4) as i32, pick: r.mem.read_u32(p + 8) as i32 };
        }
    }
    let s = r.mem.read_u32(at + 0x43c);
    assert!(s == 0 || s == sel_at + 0x33c, "the selection pointer {s:#x}");
    o.has_selection = s != 0;
    for k in 0..SLOTS {
        o.cast[k] = r.mem.read_u32(at + 0x440 + 4 * k as u32) as i32;
    }
    for k in 0..90 {
        o.riders[k] = r.mem.read_u32(at + 0x458 + 4 * k as u32) as i32;
    }
    o
}

fn call(r: &mut Runner, f: u32, args: &[u64]) -> u64 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *a);
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e}"));
    r.cpu.gpr(2)
}

#[test]
fn race_scene_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let w = Rc::new(RefCell::new(World { rand: 1, rider: 0, frames: vec![], log: vec![] }));
    let x = w.clone();
    r.stub(0x23d4a8, move |c, _| {
        let v = x.borrow_mut().draw();
        c.set_gpr(2, v as u64);
    });
    let x = w.clone();
    r.stub(0x1814a8, move |c, _| {
        x.borrow_mut().log.push(Ev::Rider);
        let v = x.borrow().rider;
        c.set_gpr(2, v as u64);
    });
    let x = w.clone();
    r.stub(0x15fb08, move |c, _| {
        let (rider, clip) = ((c.gpr(4) as u32).wrapping_sub(0x46c0), c.gpr(5) as i32);
        x.borrow_mut().log.push(Ev::Marker(rider, clip));
        let f = x.borrow().frame(clip);
        c.set_gpr(2, f as i64 as u64);
    });
    for a in [0x169688u32] {
        r.allow(a);
    }
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let obj = r.mem.alloc(OBJ_SIZE);
    let sel_at = r.mem.alloc(0x33c + 100 * SCENES as u32);
    let rider = r.mem.alloc(0x5000);
    let mut g = Rng(0x5ce0_e000_0000_0001);
    let mut seen = [0u32; 6];
    for case in 0..20_000 {
        // a selection: scripts, slots with riders from a small set (repeats), candidates or none
        let mut sel: Selection = [[0; 25]; SCENES];
        for scene in sel.iter_mut() {
            scene[0] = g.below(64) as i32;
            for j in 0..SLOTS {
                scene[1 + 4 * j] = g.next() as i32;
                scene[2 + 4 * j] = g.below(12) as i32;
                let spread = if g.below(8) == 0 { 200 } else { 20 };
                scene[3 + 4 * j] = if g.below(4) == 0 { NO_RIDER } else { 0x100 + g.below(spread) as i32 };
                scene[4 + 4 * j] = if g.below(3) == 0 { -1 - g.below(3) as i32 } else { g.below(500) as i32 };
            }
        }
        for (i, scene) in sel.iter().enumerate() {
            for (k, v) in scene.iter().enumerate() {
                r.mem.write_u32(sel_at + 0x33c + 100 * i as u32 + 4 * k as u32, *v as u32);
            }
        }
        // a scene object as some earlier step left it
        let mut ours = RaceScene { current: g.below(SCENES as u32) as i32, has_selection: g.below(2) == 0, ..Default::default() };
        for i in 0..SCENES {
            for j in 0..SLOTS {
                ours.slots[i][j] = Slot { a: g.next() as i32, b: g.below(12) as i32, pick: if g.below(3) == 0 { -1 } else { g.below(500) as i32 } };
            }
        }
        for c in ours.cast.iter_mut() {
            *c = if g.below(4) == 0 { -1 } else { g.below(500) as i32 };
        }
        for c in ours.riders.iter_mut() {
            *c = if g.below(30) == 0 { NO_RIDER } else { g.below(300) as i32 };
        }
        write_obj(&mut r, obj, &ours, sel_at);
        {
            let mut x = w.borrow_mut();
            x.rand = g.next() as u64 | 1;
            x.rider = rider;
            x.frames = (0..12).filter_map(|c| if g.below(4) != 0 { Some((c, g.below(5) as i32)) } else { None }).collect();
            x.log.clear();
        }
        let mut env = w.borrow().clone();
        let op = g.below(6) as usize;
        seen[op] += 1;
        match op {
            0 => {
                call(&mut r, 0x169688, &[obj as u64]);
                ours.reset();
            }
            1 => {
                call(&mut r, 0x169698, &[obj as u64, sel_at as u64]);
                ours.setup_from_picks(&sel, &mut || env.draw());
            }
            2 => {
                let k = g.below(SCENES as u32) as i32;
                call(&mut r, 0x1699a0, &[obj as u64, k as u64]);
                ours.select(&mut env, k);
            }
            3 => {
                let out = r.mem.alloc(4);
                let t = call(&mut r, 0x169b60, &[obj as u64, out as u64]) as u32;
                assert_eq!(t, obj + 0x440, "case {case}: cast");
                assert_eq!(r.mem.read_u32(out) as usize, ours.cast_count(), "case {case}: cast count");
            }
            4 => {
                let out = r.mem.alloc(4);
                let t = call(&mut r, 0x169948, &[obj as u64, out as u64]) as u32;
                assert_eq!(t, obj + 0x458, "case {case}: riders");
                assert_eq!(r.mem.read_u32(out) as usize, ours.rider_count(), "case {case}: rider count");
            }
            _ => {
                // the script name of the chosen scene (with the selection taken)
                r.mem.write_u32(obj + 0x43c, sel_at + 0x33c);
                ours.has_selection = true;
                let p = call(&mut r, 0x169bb8, &[obj as u64]) as u32;
                let theirs = (sel[ours.current as usize][0] < SCRIPTS.len() as i32).then(|| {
                    let mut s = String::new();
                    for k in 0.. {
                        let b = (r.mem.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
                        if b == 0 {
                            break;
                        }
                        s.push(b as char);
                    }
                    s
                });
                assert_eq!(theirs.as_deref(), ours.script_name(&sel), "case {case}: script");
            }
        }
        assert_eq!(read_obj(&mut r, obj, sel_at), ours, "case {case} op {op}: the object");
        assert_eq!(w.borrow().log, env.log, "case {case} op {op}: calls");
    }
    eprintln!("ops {seen:?}");
}
