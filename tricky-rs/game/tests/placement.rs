//! The board's attitude at a placement (board row G20c3a): the real `Boarder_PlaceAt` 0x11cae8 runs in the function
//! runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) with `Boarder_ResetOrientation` 0x128e20,
//! `Boarder_RotateAboutAxis` 0x128e90, `Boarder_UpdateOrientation` 0x129008, `Vec4_Set` 0x10c120, `Math_SinCos`,
//! `Vec3_Scale`, `Math_Atan` and `Math_Asin` run as the original, the placement's ray cast given by the test (hit or no
//! hit, any normal) and every other call it makes recorded as doing nothing. The quaternion (+0x170), the three axis
//! vectors (+0x180 forward, +0x190 side, +0x1a0 up) and the heading (+0x1b0) it leaves must equal
//! `orient::placed_attitude`'s bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::orient::{placed_attitude, Orient};

type V4 = [f32; 4];

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
    fn unit3(&mut self) -> V4 {
        loop {
            let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.1 {
                return [v[0] / l, v[1] / l, v[2] / l, 0.0];
            }
        }
    }
}

struct W {
    r: Runner,
    rider: u32,
    pos: u32,
    vel: u32,
    hit: Rc<RefCell<Option<V4>>>,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // run as the original: the orientation routines and the maths they use
    let run: [u32; 8] = [0x128e20, 0x128e90, 0x129008, 0x10c120, 0x250c10, 0x102f80, 0x2510f8, 0x250fd0];
    for a in run {
        r.allow(a);
    }
    let hit: Rc<RefCell<Option<V4>>> = Rc::new(RefCell::new(None));
    {
        let hit = hit.clone();
        // `World_RayCast` 0x25aff8(world, line, out, ...): the hit's place at out+0x10, its normal at out+0x20
        r.stub(0x25aff8, move |cpu, mem| {
            let out = cpu.gpr(6) as u32;
            match *hit.borrow() {
                Some(n) => {
                    for k in 0..4u32 {
                        mem.write_f32(out + 0x10 + 4 * k, [1.0, 2.0, 3.0, 0.0][k as usize]);
                        mem.write_f32(out + 0x20 + 4 * k, n[k as usize]);
                    }
                    cpu.set_f(0, 1.0);
                }
                None => cpu.set_f(0, 0.0),
            }
        });
    }
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..(0x4a0 / 4) {
        let w = r.mem.read_u32(0x11cae8 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    jals.remove(&0x25aff8);
    for a in run {
        jals.remove(&a);
    }
    for a in jals {
        r.stub(a, |cpu, _| cpu.set_gpr(2, 0));
    }
    // the call through the vtable (rider+0, slot at +0x9c)
    r.stub(0x4000, |cpu, _| cpu.set_gpr(2, 0));
    let rider = r.mem.alloc(0x6000);
    let vt = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x100);
    let pos = r.mem.alloc(0x10);
    let vel = r.mem.alloc(0x10);
    r.mem.write_u32(vt + 0x9c, 0x4000);
    r.mem.write_u32(rider, vt);
    r.mem.write_u32(rider + 0x464, ctl);
    for (k, x) in [10.0f32, 20.0, 30.0, 0.0].iter().enumerate() {
        r.mem.write_f32(pos + 4 * k as u32, *x);
    }
    Some(W { r, rider, pos, vel, hit })
}

/// What the original leaves in +0x170..+0x1b4, as words.
fn game_attitude(w: &mut W, heading: f32, hit: Option<V4>, flag: bool) -> Vec<u32> {
    *w.hit.borrow_mut() = if flag { None } else { hit };
    // noise where the attitude lives, so a field the placement does not write cannot pass by luck
    for k in 0..0x14u32 {
        w.r.mem.write_f32(w.rider + 0x170 + 4 * k, 7.0 + k as f32);
    }
    for k in 0..4u32 {
        w.r.mem.write_f32(w.rider + 0x1a0 + 4 * k, 0.5);
        w.r.mem.write_f32(w.rider + 0x2a0 + 4 * k, 5.0);
    }
    w.r.cpu.set_gpr(4, w.rider as u64);
    w.r.cpu.set_gpr(5, w.pos as u64);
    w.r.cpu.set_gpr(6, w.vel as u64);
    w.r.cpu.set_gpr(7, flag as u64);
    w.r.cpu.set_f(12, heading);
    w.r.cpu.set_f(13, 0.0);
    w.r.call(0x11cae8).expect("Boarder_PlaceAt runs to its return");
    (0..0x11u32).map(|k| w.r.mem.read_u32(w.rider + 0x170 + 4 * k)).collect()
}

/// Our attitude in the same layout (+0x170 q, +0x180 forward, +0x190 side, +0x1a0 up, +0x1b0 heading).
fn ours(o: &Orient) -> Vec<u32> {
    let mut v = vec![0u32; 0x11];
    for k in 0..4 {
        v[k] = o.q[k].to_bits();
        v[4 + k] = o.fwd[k].to_bits();
        v[8 + k] = o.side[k].to_bits();
        v[12 + k] = o.up[k].to_bits();
    }
    v[16] = o.yaw.to_bits();
    v
}

/// Which branch a case took, for the coverage check.
#[derive(Default, Debug)]
struct Cover {
    turned: u32,
    skipped: u32,
    clamped: u32,
    no_ray: u32,
    flagged: u32,
    behind: u32,
}

fn headings(g: &mut Rng, n: usize) -> f32 {
    const SPECIAL: [f32; 9] = [0.0, 1.5707964, -1.5707964, 3.1415927, -3.1415927, 6.2831855, 1e-7, -1e-7, 0.7853982];
    if n % 4 == 0 {
        SPECIAL[(n / 4) % SPECIAL.len()]
    } else {
        g.f(-7.0, 7.0)
    }
}

/// The normals the ray can answer: anything, plus the ones that meet the branches' edges (straight up, down,
/// sideways: the cross product's length 0 or 1, and a hair either side of the 1e-5 limit).
fn normal(g: &mut Rng, n: usize, up: V4) -> V4 {
    match n % 8 {
        0 => [0.0, 0.0, 1.0, 0.0],
        1 => [0.0, 0.0, -1.0, 0.0],
        2 => {
            // horizontal, so the cross product has length 1 (held to 0.99999)
            let a = g.f(0.0, 6.2831855);
            [a.cos(), a.sin(), 0.0, 0.0]
        }
        3 => {
            // a hair off the board's up: the cross product's length near 1e-5
            let e = g.f(0.5e-5, 2.0e-5);
            let a = g.f(0.0, 6.2831855);
            let v = [up[0] + e * a.cos(), up[1] + e * a.sin(), up[2], 0.0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / l, v[1] / l, v[2] / l, 0.0]
        }
        4 => {
            // leaning, a few degrees
            let e = g.f(0.0, 0.4);
            let a = g.f(0.0, 6.2831855);
            let v = [e * a.cos(), e * a.sin(), 1.0, 0.0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / l, v[1] / l, v[2] / l, 0.0]
        }
        _ => g.unit3(),
    }
}

#[test]
fn placement_attitude_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x6_20c3_a000_0001);
    let mut cover = Cover::default();
    let mut wrong = Vec::new();
    for n in 0..6000usize {
        let heading = headings(&mut g, n);
        let flag = g.next() % 6 == 0;
        let ray = !flag && g.next() % 5 != 0;
        // the board's up after the heading's turn is (0, 0, 1) whatever the heading: the first case's rotation is about z
        let up = [0.0, 0.0, 1.0, 0.0];
        let hit = if ray { Some(normal(&mut g, n, up)) } else { None };
        let game = game_attitude(&mut w, heading, hit, flag);
        let o = placed_attitude(heading, if flag { None } else { hit });
        match hit {
            _ if flag => cover.flagged += 1,
            None => cover.no_ray += 1,
            Some(nrm) => {
                // normal × (0, 0, 1) = (n.y, −n.x, 0)
                let s = (nrm[1] * nrm[1] + nrm[0] * nrm[0]).sqrt();
                if s <= 1e-5 {
                    cover.skipped += 1
                } else {
                    cover.turned += 1;
                    if s > 0.99999 {
                        cover.clamped += 1
                    }
                    if nrm[2] < 0.0 {
                        cover.behind += 1
                    }
                }
            }
        }
        let mine = ours(&o);
        if game != mine {
            wrong.push(format!("case {n}: heading {heading} hit {hit:?} flag {flag}\n  game {game:x?}\n  ours {mine:x?}"));
            if wrong.len() > 5 {
                break;
            }
        }
    }
    eprintln!("Boarder_PlaceAt attitude: 6000 cases, {cover:?}");
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(cover.turned > 1500 && cover.skipped > 100 && cover.clamped > 100, "{cover:?}");
    assert!(cover.no_ray > 300 && cover.flagged > 300 && cover.behind > 300, "{cover:?}");
}

/// The comparison can tell: a one-bit error in the quaternion, or the turn left out, is caught.
#[test]
fn planted_errors_are_caught() {
    let Some(mut w) = world() else { return };
    let heading = 0.9;
    let hit = Some([0.3f32, -0.2, 0.9, 0.0]);
    let game = game_attitude(&mut w, heading, hit, false);
    let o = placed_attitude(heading, hit);
    assert_eq!(game, ours(&o));
    let mut bad = o;
    bad.q[1] = f32::from_bits(bad.q[1].to_bits() ^ 1);
    assert_ne!(game, ours(&bad));
    assert_ne!(game, ours(&placed_attitude(heading, None)), "the ray's turn must matter");
}
