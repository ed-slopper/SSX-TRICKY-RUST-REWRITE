//! The rail normal from the game's own inputs (board row G20c2c): the real `Rail_ClosestPoint` 0x259330 and the real
//! `RailSlideMotion_Update` 0x10b0d0 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped
//! without it), over a world grid built in the runner's memory (cells, patches, curves, owners as the original reads
//! them), and `ground_tick::RailWorld::closest_point` and `rail_slide_normal` must equal them bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::rider::ground_tick as gt;
use gt::{RailCell, RailCurve, RailOwner, RailPatch, RailSlideIn, RailWorld};

type V4 = [f32; 4];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    /// -1 ..= 1
    fn unit(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 10_000.0 - 1.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.unit() * 0.5 + 0.5) * (hi - lo)
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

const GWORLD: u32 = 0x346488;

fn put_v(r: &mut Runner, at: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * k as u32, *x);
    }
}

fn get_v(r: &mut Runner, at: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| r.mem.read_u32(at + 4 * k))
}

fn bits(v: V4) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn put_box(r: &mut Runner, at: u32, b: &[f32; 6]) {
    for (k, x) in b.iter().enumerate() {
        r.mem.write_f32(at + 4 * k as u32, *x);
    }
}

fn union(boxes: impl Iterator<Item = [f32; 6]>) -> [f32; 6] {
    let mut u = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
    for b in boxes {
        for k in 0..3 {
            u[k] = u[k].min(b[k]);
            u[k + 3] = u[k + 3].max(b[k + 3]);
        }
    }
    u
}

/// The grid in the runner's memory, as the original reads it; returns the world's address.
fn put_world(r: &mut Runner, w: &RailWorld, per_cell: u32) -> u32 {
    let wp = r.mem.alloc(0x60);
    r.mem.write_f32(wp + 4, w.x0);
    r.mem.write_f32(wp + 8, w.y0);
    r.mem.write_f32(wp + 0x28, w.cell);
    r.mem.write_u32(wp + 0x2c, w.nx as u32);
    r.mem.write_u32(wp + 0x30, w.ny as u32);
    r.mem.write_u32(wp + 0x48, per_cell);
    let table = r.mem.alloc(4 * w.cells.len() as u32 + 4);
    r.mem.write_u32(wp + 0x4c, table);
    for (i, cell) in w.cells.iter().enumerate() {
        let Some(cell) = cell else { continue };
        let cp = r.mem.alloc(0x40);
        r.mem.write_u32(table + 4 * i as u32, cp);
        put_box(r, cp, &cell.bbox);
        assert_eq!(cell.patches.len() as u32, per_cell);
        let patches = r.mem.alloc(0x4c * per_cell + 4);
        r.mem.write_u32(cp + 0x38, patches);
        for (j, patch) in cell.patches.iter().enumerate() {
            let pp = patches + 0x4c * j as u32;
            put_box(r, pp, &patch.bbox);
            r.mem.write_u32(pp + 0x24, patch.x24);
            r.mem.write_u16(pp + 0x28, patch.x28);
            r.mem.write_u16(pp + 0x2a, patch.curves.len() as u16);
            r.mem.write_u16(pp + 0x2c, patch.x2c as u16);
            r.mem.write_u32(pp + 0x48, patch.x48);
            let list = r.mem.alloc(4 * patch.curves.len() as u32 + 4);
            r.mem.write_u32(pp + 0x3c, list);
            for (k, c) in patch.curves.iter().enumerate() {
                let cv = r.mem.alloc(0x80);
                r.mem.write_u32(list + 4 * k as u32, cv);
                for (row, m) in c.m.iter().enumerate() {
                    put_v(r, cv + 0x10 * row as u32, *m);
                }
                let owner = c.owner.expect("the test world gives every curve an owner");
                let op = r.mem.alloc(0x30);
                r.mem.write_u32(op + 0x18, owner.flags);
                r.mem.write_u32(op + 0x24, owner.tag);
                r.mem.write_u32(cv + 0x58, op);
                put_box(r, cv + 0x5c, &c.bbox);
            }
        }
    }
    wp
}

/// A random world: a few control polygons (cubic Béziers as the curve matrices), their boxes, the patches and cells
/// round them, some boxes shrunk so that they miss, some owners off, some records empty, some cells missing.
/// Returns the world and the control points of each curve (to aim queries at them).
fn random_world(g: &mut Rng) -> (RailWorld, Vec<[V4; 4]>, u32) {
    let cell = g.range(300.0, 700.0);
    let (nx, ny) = (2 + g.below(4) as i32, 2 + g.below(4) as i32);
    let (x0, y0) = (g.unit() * 2000.0, g.unit() * 2000.0);
    let per_cell = 1 + g.below(3);
    let mut cps = Vec::new();
    let mut cells = Vec::new();
    for _ in 0..nx * ny {
        if g.below(8) == 0 {
            cells.push(None);
            continue;
        }
        let mut patches = Vec::new();
        for _ in 0..per_cell {
            let mut curves = Vec::new();
            let n = if g.below(5) == 0 { 0 } else { 1 + g.below(3) };
            for _ in 0..n {
                let base = [x0 + g.range(0.0, nx as f32 * cell), y0 + g.range(0.0, ny as f32 * cell), g.unit() * 400.0, 0.0];
                let mut p = [[0.0f32; 4]; 4];
                for (k, pk) in p.iter_mut().enumerate() {
                    *pk = [base[0] + g.unit() * 350.0, base[1] + g.unit() * 350.0, base[2] + g.unit() * 150.0, if g.below(8) == 0 { g.unit() * 5.0 } else { 0.0 }];
                    let _ = k;
                }
                let mut m = [[0.0f32; 4]; 4];
                for k in 0..4 {
                    m[0][k] = -p[0][k] + 3.0 * p[1][k] - 3.0 * p[2][k] + p[3][k];
                    m[1][k] = 3.0 * p[0][k] - 6.0 * p[1][k] + 3.0 * p[2][k];
                    m[2][k] = -3.0 * p[0][k] + 3.0 * p[1][k];
                    m[3][k] = p[0][k];
                }
                let mut bbox = union(std::iter::once([
                    p.iter().map(|q| q[0]).fold(f32::MAX, f32::min),
                    p.iter().map(|q| q[1]).fold(f32::MAX, f32::min),
                    p.iter().map(|q| q[2]).fold(f32::MAX, f32::min),
                    p.iter().map(|q| q[0]).fold(f32::MIN, f32::max),
                    p.iter().map(|q| q[1]).fold(f32::MIN, f32::max),
                    p.iter().map(|q| q[2]).fold(f32::MIN, f32::max),
                ]));
                if g.below(10) == 0 {
                    for k in 0..3 {
                        bbox[k] += (bbox[k + 3] - bbox[k]) * 0.9;
                    }
                }
                let flags = if g.below(6) == 0 { g.next() & !1 } else { g.next() | 1 };
                curves.push(RailCurve { m, owner: Some(RailOwner { flags, tag: g.next() }), bbox });
                cps.push(p);
            }
            let mut bbox = if curves.is_empty() { [0.0; 6] } else { union(curves.iter().map(|c| c.bbox)) };
            if g.below(12) == 0 {
                for k in 0..3 {
                    bbox[k] += (bbox[k + 3] - bbox[k]) * 0.8;
                }
            }
            let used = g.below(6) != 0;
            let (x24, x28, x2c, x48) = if used { (g.below(3) * 7, 0, 0, 0) } else { (0, 0, 0, if g.below(3) == 0 { 5 } else { 0 }) };
            patches.push(RailPatch { bbox, x24, x28, x2c, x48, curves });
        }
        let mut bbox = union(patches.iter().map(|p| p.bbox));
        if g.below(12) == 0 {
            for k in 0..3 {
                bbox[k] += (bbox[k + 3] - bbox[k]) * 0.8;
            }
        }
        // a cell box that holds the whole map, sometimes, so that the curves are reached
        if g.below(2) == 0 {
            bbox = [x0 - 1.0e4, y0 - 1.0e4, -1.0e4, x0 + 1.0e4, y0 + 1.0e4, 1.0e4];
        }
        cells.push(Some(RailCell { bbox, patches }));
    }
    (RailWorld { x0, y0, cell, nx, ny, cells }, cps, per_cell)
}

fn bezier(p: &[V4; 4], t: f32) -> V4 {
    let s = 1.0 - t;
    let mut o = [0.0; 4];
    for k in 0..4 {
        o[k] = s * s * s * p[0][k] + 3.0 * s * s * t * p[1][k] + 3.0 * s * t * t * p[2][k] + t * t * t * p[3][k];
    }
    o
}

#[test]
fn closest_point_is_the_originals() {
    // `Rail_ClosestPoint` 0x259330 (its only call, `Vec4_Scale` 0x102f50, running as the original): the point, the
    // unit tangent and the owner's word it writes, and whether it finds anything, equal `RailWorld::closest_point`
    let Some(mut r) = runner() else { return };
    r.allow(0x102f50);
    let mut g = Rng(0x5eed_0259_3300);
    let pos_at = r.mem.alloc(0x10);
    let out = r.mem.alloc(0x40);
    let unused = r.mem.alloc(0x10);
    let (mut found, mut missed, mut moved, mut wrong) = (0, 0, 0, Vec::new());
    for round in 0..60 {
        let (w, cps, per_cell) = random_world(&mut g);
        let wp = put_world(&mut r, &w, per_cell);
        r.mem.write_u32(GWORLD, wp);
        for q in 0..150 {
            // a query near a curve (most), or anywhere in the grid
            let pos: V4 = if !cps.is_empty() && g.below(4) != 0 {
                let c = &cps[g.below(cps.len() as u32) as usize];
                let p = bezier(c, g.range(0.0, 1.0));
                let spread = if g.below(3) == 0 { 450.0 } else { 40.0 };
                [p[0] + g.unit() * spread, p[1] + g.unit() * spread, p[2] + g.unit() * spread, if g.below(8) == 0 { g.unit() } else { 0.0 }]
            } else {
                [w.x0 + g.range(-800.0, w.nx as f32 * w.cell + 800.0), w.y0 + g.range(-800.0, w.ny as f32 * w.cell + 800.0), g.unit() * 500.0, 0.0]
            };
            put_v(&mut r, pos_at, pos);
            for k in 0..0x10 {
                r.mem.write_u32(out + 4 * k, 0xdead_beef);
            }
            r.cpu.set_gpr(4, GWORLD as u64);
            r.cpu.set_gpr(5, pos_at as u64);
            r.cpu.set_gpr(6, unused as u64);
            r.cpu.set_gpr(7, out as u64);
            r.call(0x259330).unwrap_or_else(|e| panic!("round {round} query {q}: pos {pos:?} {e:?}"));
            let game_found = r.cpu.gpr(2) as u32 != 0;
            let (gp, gt_, gw) = (get_v(&mut r, out + 0x10), get_v(&mut r, out + 0x20), r.mem.read_u32(out + 0x34));
            let ours = w.closest_point(pos);
            let ok = match ours {
                None => !game_found && gp == [0xdead_beef; 4] && gt_ == [0xdead_beef; 4] && gw == 0xdead_beef,
                Some(h) => game_found && gp == bits(h.point) && gt_ == bits(h.tangent) && gw == h.word,
            };
            if ours.is_some() {
                found += 1;
            } else {
                missed += 1;
            }
            if let Some(h) = ours {
                moved += (h.point != [0.0; 4]) as u32;
            }
            if !ok {
                wrong.push(format!("round {round} query {q}: pos {pos:?}: game found {game_found} point {gp:x?} tangent {gt_:x?} word {gw:#x}; ours {ours:?}"));
                if wrong.len() > 6 {
                    break;
                }
            }
        }
        if wrong.len() > 6 {
            break;
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(found > 2000 && missed > 300 && moved > 2000, "found {found}, missed {missed}, moved {moved}");
}

fn unit3(g: &mut Rng) -> V4 {
    let v = [g.unit(), g.unit(), g.unit() * 0.6, 0.0];
    let q = 1.0 / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] * q, v[1] * q, v[2] * q, 0.0]
}

fn cross3(a: V4, b: V4) -> V4 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0], 0.0]
}

#[test]
fn rail_slide_normal_is_the_originals() {
    // `RailSlideMotion_Update` 0x10b0d0 with the real `Rail_ClosestPoint`: the normal (+0x2a0) and lean angle
    // (object +8) it leaves inside the fall-off limits, the hit it records (+0x290, +0x2d0..), the blend it leaves
    // (+0x284), and whether it takes the board off the rail (it calls `Boarder_SetState` 0x11f608) are
    // `ground_tick::rail_slide_normal`'s
    let Some(mut r) = runner() else { return };
    let real = [0x102f50, 0x102f80, 0x250c10, 0x259330, 0x2508b8, 0x250a48, 0x102e80, 0x10c120, 0x250fd0, 0x250d60, 0x129008, 0x2510f8];
    for a in real {
        r.allow(a);
    }
    // every other call of the function (the vector helpers it only fills stack with, the quaternions, the collision
    // calls, the take-off) does nothing; the state change is recorded
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..((0x10c060 - 0x10b0d0) / 4) {
        let w = r.mem.read_u32(0x10b0d0 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    for a in real {
        jals.remove(&a);
    }
    let off_rail = Rc::new(RefCell::new(0u32));
    for a in jals {
        if a == 0x11f608 {
            let flag = off_rail.clone();
            r.stub(a, move |_, _| *flag.borrow_mut() += 1);
        } else {
            r.stub(a, |cpu, _| cpu.set_gpr(2, 0));
        }
    }
    let rider = r.mem.alloc(0x6000);
    let obj = r.mem.alloc(0x40);
    let ctl = r.mem.alloc(0x100);
    let other = r.mem.alloc(0x1000);
    r.mem.write_u32(obj + 0x10, rider);
    r.mem.write_u32(rider + 0x464, ctl);
    r.mem.write_u32(rider + 0x5ae0, other);
    let mut g = Rng(0x5eed_010b_0d01);
    let (mut inside, mut outside, mut none, mut blended, mut flipped, mut wrong) = (0, 0, 0, 0, 0, Vec::new());
    let mut attitude_wrong: Vec<String> = Vec::new();
    for round in 0..40 {
        let (w, cps, per_cell) = random_world(&mut g);
        let wp = put_world(&mut r, &w, per_cell);
        r.mem.write_u32(GWORLD, wp);
        for q in 0..300 {
            // the board's frame: a rotation (rows x, y, z) and its position, near a curve (most)
            let z = unit3(&mut g);
            let x = {
                let t = cross3([0.0, 0.0, 1.0, 0.0], z);
                let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
                if l < 0.2 { [1.0, 0.0, 0.0, 0.0] } else { [t[0] / l, t[1] / l, t[2] / l, 0.0] }
            };
            let y = cross3(z, x);
            let pos: V4 = if !cps.is_empty() && g.below(6) != 0 {
                let c = &cps[g.below(cps.len() as u32) as usize];
                let p = bezier(c, g.range(0.0, 1.0));
                // the offset is given in the frame, so the fall-off limits are met from both sides
                let (a, b, d) = (g.unit() * 170.0, g.unit() * 60.0 - 30.0, g.unit() * 60.0);
                [p[0] + x[0] * a + y[0] * b + z[0] * d, p[1] + x[1] * a + y[1] * b + z[1] * d, p[2] + x[2] * a + y[2] * b + z[2] * d, if g.below(8) == 0 { g.unit() } else { 0.0 }]
            } else {
                [w.x0 + g.range(-300.0, w.nx as f32 * w.cell + 300.0), w.y0 + g.range(-300.0, w.ny as f32 * w.cell + 300.0), g.unit() * 500.0, 0.0]
            };
            let input = RailSlideIn {
                pos,
                frame: [x, y, z],
                vel: [g.unit() * 800.0 + 1.0, g.unit() * 800.0, g.unit() * 800.0, 0.0],
                side: unit3(&mut g),
                up: unit3(&mut g),
                kind: [1u32, 2, 3, 4][g.below(4) as usize],
                time: if g.below(3) == 0 { g.range(0.0, 0.6) } else { g.range(0.6, 4.0) },
                blend: if g.below(2) == 0 { 0.0 } else { g.range(0.0, 1.0) },
                lean: g.unit() * 12.0,
            };
            for (at, v) in [(0x4a60, input.frame[0]), (0x4a70, input.frame[1]), (0x4a80, input.frame[2]), (0x4a90, input.pos), (0x150, input.vel), (0x190, input.side), (0x1a0, input.up)] {
                put_v(&mut r, rider + at, v);
            }
            r.mem.write_u32(rider + 0x1dc, input.kind);
            r.mem.write_f32(rider + 0x284, input.blend);
            r.mem.write_f32(rider + 0x12c, 0.0);
            r.mem.write_f32(rider + 0x130, 0.0);
            r.mem.write_f32(obj + 0xc, input.time);
            r.mem.write_f32(obj + 8, input.lean);
            let q0 = {
                let v = [g.unit(), g.unit(), g.unit(), g.unit()];
                let l = (((v[1] * v[1] + v[2] * v[2]) + v[3] * v[3]) + v[0] * v[0]).sqrt().max(1e-3);
                [v[0] / l, v[1] / l, v[2] / l, v[3] / l]
            };
            put_v(&mut r, rider + 0x170, q0);
            r.mem.write_f32(rider + 0x1b0, 9.0);
            put_v(&mut r, rider + 0x180, [9.0; 4]);
            put_v(&mut r, rider + 0x2a0, [7.0; 4]);
            put_v(&mut r, rider + 0x2d0, [8.0; 4]);
            r.mem.write_u32(rider + 0x290, 0x1234_5678);
            *off_rail.borrow_mut() = 0;
            r.cpu.set_gpr(4, obj as u64);
            r.call(0x10b0d0).unwrap_or_else(|e| panic!("round {round} query {q}: {input:?} {e:?}"));
            let (gn, gl, gh, gw, gb, gset) = (
                get_v(&mut r, rider + 0x2a0),
                r.mem.read_u32(obj + 8),
                get_v(&mut r, rider + 0x2d0),
                r.mem.read_u32(rider + 0x290),
                r.mem.read_u32(rider + 0x284),
                *off_rail.borrow(),
            );
            let ours = gt::rail_slide_normal(&w, &input);
            let ok = match ours {
                None => gset == 1 && gn == [7.0f32.to_bits(); 4] && gh == [8.0f32.to_bits(); 4] && gw == 0x1234_5678 && gl == input.lean.to_bits(),
                Some(s) => {
                    let common = gh == bits(s.hit.point) && gw == s.hit.word && gb == s.blend.to_bits();
                    match s.on {
                        None => common && gset == 1 && gn == [7.0f32.to_bits(); 4] && gl == input.lean.to_bits(),
                        Some(n) => {
                            let mut o = tricky_game::orient::Orient { q: gt::rail_attitude(q0, n.normal, gt::rail_travel(s.hit.tangent, input.vel), n.lean), ..Default::default() };
                            tricky_game::orient::update_orientation(&mut o);
                            let att = (get_v(&mut r, rider + 0x170), get_v(&mut r, rider + 0x180), get_v(&mut r, rider + 0x190), get_v(&mut r, rider + 0x1a0), r.mem.read_u32(rider + 0x1b0));
                            let same = att == (bits(o.q), bits(o.fwd), bits(o.side), bits(o.up), o.yaw.to_bits());
                            if !same {
                                attitude_wrong.push(format!("round {round} query {q}: game {att:x?}
  ours {o:?}"));
                            }
                            common && gset == 0 && gn == bits(n.normal) && gl == n.lean.to_bits() && same
                        }
                    }
                }
            };
            match ours {
                None => none += 1,
                Some(s) if s.on.is_some() => inside += 1,
                Some(_) => outside += 1,
            }
            if let Some(s) = ours {
                blended += (s.blend != input.blend) as u32;
                if gt::rail_travel(s.hit.tangent, input.vel) != s.hit.tangent {
                    flipped += 1;
                }
            }
            if !ok {
                wrong.push(format!(
                    "round {round} query {q}: {input:?}\n  game: normal {gn:x?} lean {gl:#x} hit {gh:x?} word {gw:#x} blend {gb:#x} off-rail calls {gset}\n  ours: {ours:?}"
                ));
                if wrong.len() > 4 {
                    break;
                }
            }
        }
        if wrong.len() > 4 {
            break;
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(inside > 1500 && outside > 500 && none > 100 && blended > 500 && flipped > 500, "inside {inside}, outside {outside}, none {none}, blended {blended}, flipped {flipped}");
}

#[test]
fn quaternion_helpers_are_the_originals() {
    // `Quat_Multiply` 0x102e80, `Quat_RotateVecToVec` 0x2508b8 and `Quat_Slerp` 0x250a48 on their own, on random
    // and near-equal quaternions and on parallel, opposite and random unit vectors
    use tricky_game::orient::{quat_multiply, rotate_vec_to_vec, slerp};
    let Some(mut r) = runner() else { return };
    for a in [0x102e80, 0x2508b8, 0x250a48, 0x250c10, 0x250fd0, 0x250d60] {
        r.allow(a);
    }
    let (out, pa, pb, pc) = (r.mem.alloc(16), r.mem.alloc(16), r.mem.alloc(16), r.mem.alloc(16));
    let mut g = Rng(0x0907_aa31_5c4e_2d18);
    let quat = |g: &mut Rng| {
        let v = [g.unit(), g.unit(), g.unit(), g.unit()];
        let l = (((v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]) + v[3] * v[3]).sqrt().max(1e-3);
        [v[0] / l, v[1] / l, v[2] / l, v[3] / l]
    };
    let mut near = 0;
    for case in 0..30_000 {
        let p = quat(&mut g);
        let q = match g.below(4) {
            0 => [p[0] + g.unit() * 1e-3, p[1] + g.unit() * 1e-3, p[2], p[3] + g.unit() * 1e-3],
            1 => [-p[0], -p[1] + g.unit() * 1e-3, -p[2], -p[3]],
            _ => quat(&mut g),
        };
        put_v(&mut r, pa, p);
        put_v(&mut r, pb, q);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, pa as u64);
        r.cpu.set_gpr(6, pb as u64);
        r.call(0x102e80).unwrap();
        assert_eq!(get_v(&mut r, out), bits(quat_multiply(p, q)), "case {case}: multiply {p:?} {q:?}");
        let t = [0.0, 1.0, g.range(0.0, 1.0), 1.0 / 12.0][g.below(4) as usize];
        r.cpu.set_f(12, t);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, pa as u64);
        r.cpu.set_gpr(6, pb as u64);
        r.call(0x250a48).unwrap();
        assert_eq!(get_v(&mut r, out), bits(slerp(t, p, q)), "case {case}: slerp {t} {p:?} {q:?}");
        let d = ((p[1] * q[1] + p[2] * q[2]) + p[3] * q[3]) + p[0] * q[0];
        near += (1.0 - d.abs() <= 0.001) as u32;
        let a = unit3(&mut g);
        let b = match g.below(5) {
            0 => a,
            1 => [-a[0], -a[1], -a[2], 0.0],
            2 => { let v = [a[0] + g.unit() * 1e-3, a[1], a[2] + g.unit() * 1e-3, 0.0]; let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); [v[0] / l, v[1] / l, v[2] / l, 0.0] }
            _ => unit3(&mut g),
        };
        put_v(&mut r, out, p);
        put_v(&mut r, pc, a);
        put_v(&mut r, pb, b);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, pc as u64);
        r.cpu.set_gpr(6, pb as u64);
        r.call(0x2508b8).unwrap();
        assert_eq!(get_v(&mut r, out), bits(rotate_vec_to_vec(p, a, b)), "case {case}: rotate {p:?} by {a:?} to {b:?}");
    }
    assert!(near > 3000, "near-equal slerps: {near}");
}
