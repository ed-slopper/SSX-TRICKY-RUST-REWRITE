//! The ground normal (rider+0x2a0) as the game writes it on a rail and at a placement (board row G20c2): the real
//! `RailSlideMotion_Update` 0x10b0d0 and `Boarder_PlaceAt` 0x11cae8 run in the function runner on the player's
//! SLUS_203.26 (TRICKY_ELF; skipped without it), the calls they make that are not the normal's recorded as doing
//! nothing, and the normal and the lean angle they leave equal `ground_tick`'s bit for bit.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::rider::ground_tick as gt;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 10_000.0 - 1.0
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn put_v(r: &mut Runner, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * k as u32, *x);
    }
}

fn get_v(r: &mut Runner, at: u32) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| r.mem.read_u32(at + 4 * k))
}

fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[test]
fn rail_normal_is_the_originals() {
    // `RailSlideMotion_Update` 0x10b0d0 with the rail's closest point (`Rail_ClosestPoint` 0x259330) given by the test and
    // the board inside the fall-off limits: the normal it stores (0x10bbb8) and the lean angle (object +8) are
    // `rail_normal` of the tangent (turned to the travel) and `rail_lean_angle` of the lateral offset it measured.
    let Some(mut r) = runner() else { return };
    let pick = Rc::new(RefCell::new(([0.0f32; 4], [0.0f32; 4])));
    {
        let pick = pick.clone();
        r.stub(0x259330, move |cpu, mem| {
            let (point, tangent) = *pick.borrow();
            let out = cpu.gpr(7) as u32;
            for k in 0..4 {
                mem.write_f32(out + 0x10 + 4 * k, point[k as usize]);
                mem.write_f32(out + 0x20 + 4 * k, tangent[k as usize]);
            }
            cpu.set_gpr(2, 1);
        });
    }
    // everything else it calls: the vector helpers run as the original, the rest does nothing
    for a in [0x102f50, 0x102f80, 0x250c10] {
        r.allow(a);
    }
    for a in [0x10c120, 0x2508b8, 0x250a48, 0x129008, 0x125088, 0x126250] {
        r.stub(a, |_, _| {});
    }
    let mut g = Rng(0x5eed_010b_0d00);
    let rider = r.mem.alloc(0x6000);
    let obj = r.mem.alloc(0x40);
    r.mem.write_u32(obj + 0x10, rider);
    let (mut steep, mut flat, mut flipped, mut eased) = (0, 0, 0, 0);
    let mut wrong = Vec::new();
    for n in 0..20_000u32 {
        // the board's frame: x, y, z axes and a position; the closest point is a small offset from the position
        put_v(&mut r, rider + 0x4a60, [1.0, 0.0, 0.0, 0.0]);
        put_v(&mut r, rider + 0x4a70, [0.0, 1.0, 0.0, 0.0]);
        put_v(&mut r, rider + 0x4a80, [0.0, 0.0, 1.0, 0.0]);
        let pos = [g.unit() * 5000.0, g.unit() * 5000.0, g.unit() * 5000.0, 0.0];
        put_v(&mut r, rider + 0x4a90, pos);
        let d = [g.unit() * 40.0, g.unit() * 19.0, g.unit() * 15.0, if g.next() % 8 == 0 { g.unit() } else { 0.0 }];
        let point = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2], pos[3] + d[3]];
        // the tangent: random directions, some near the vertical (steep above 0.92), some with a w
        let mut t = [g.unit(), g.unit(), g.unit() * if g.next() % 3 == 0 { 4.0 } else { 0.3 }, 0.0];
        if g.next() % 8 == 0 {
            t[3] = g.unit() * 0.01;
        }
        let t = probe_unit(t);
        *pick.borrow_mut() = (point, t);
        let vel = [g.unit() * 800.0 + 1.0, g.unit() * 800.0, g.unit() * 800.0, 0.0];
        let mut up = [g.unit(), g.unit(), g.unit(), 0.0];
        up = probe_unit(up);
        let axis = probe_unit([g.unit(), g.unit(), g.unit(), 0.0]);
        let kind = [1u32, 2, 3, 4][(g.next() % 4) as usize];
        let prev = g.unit() * 12.0;
        put_v(&mut r, rider + 0x150, vel);
        put_v(&mut r, rider + 0x1a0, up);
        put_v(&mut r, rider + 0x190, axis);
        r.mem.write_u32(rider + 0x1dc, kind);
        r.mem.write_u32(rider + 0x284, 0);
        r.mem.write_f32(rider + 0x12c, 0.0);
        r.mem.write_f32(rider + 0x130, 0.0);
        r.mem.write_f32(obj + 0xc, 1.0 + g.unit().abs());
        r.mem.write_f32(obj + 8, prev);
        put_v(&mut r, rider + 0x2a0, [7.0, 7.0, 7.0, 7.0]);
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x10b0d0).unwrap_or_else(|e| panic!("call {n}: point {point:?} pos {pos:?} {e:?}"));
        let game_n = get_v(&mut r, rider + 0x2a0);
        let game_a = r.mem.read_u32(obj + 8);
        // ours: the tangent turned to the travel, the lateral offset as the original measures it
        let travel = gt::rail_travel(t, vel);
        let towards = travel != t;
        let lateral = gt::rail_offset(axis, [point[0] - pos[0], point[1] - pos[1], point[2] - pos[2], point[3] - pos[3]]);
        let angle = gt::rail_lean_angle(prev, lateral, kind, up[2]);
        let normal = gt::rail_normal(travel, up, angle);
        flipped += towards as u32;
        if travel[2].abs() > 0.92 { steep += 1 } else { flat += 1 }
        eased += (angle != lateral_aim(lateral, kind, up[2])) as u32;
        if game_a != angle.to_bits() || game_n != bits(normal) {
            wrong.push(format!(
                "call {n}: tangent {travel:?} up {up:?} lateral {lateral} kind {kind} prev {prev}: game {game_n:x?} angle {game_a:#x}, ours {:x?} angle {:#x}",
                bits(normal),
                angle.to_bits()
            ));
            if wrong.len() > 10 {
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(steep > 1000 && flat > 1000 && flipped > 1000 && eased > 1000, "steep {steep} flat {flat} flipped {flipped} eased {eased}");
}

fn probe_unit(v: [f32; 4]) -> [f32; 4] {
    // only a direction for the inputs, not compared
    let q = 1.0 / v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.map(|x| x * q)
}

fn lateral_aim(lateral: f32, kind: u32, up_z: f32) -> f32 {
    let (a, b) = if kind.wrapping_sub(3) < 2 { (f32::from_bits(0x3f32_b8c2), f32::from_bits(0x3c4c_cccd)) } else { (f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3d08_8889)) };
    let aim = lateral * a * b;
    if up_z < 0.0 { -aim } else { aim }
}

#[test]
fn placement_normal_is_the_originals() {
    // `Boarder_PlaceAt` 0x11cae8: the normal (+0x2a0) is (0,0,1,0), or the ray's hit normal when the placement casts one
    // (`World_RayCast` 0x25aff8, given by the test) and it hits; the rider's up (+0x2c0) is a copy of it
    let Some(mut r) = runner() else { return };
    let hit: Rc<RefCell<Option<[f32; 4]>>> = Rc::new(RefCell::new(None));
    {
        let hit = hit.clone();
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
    // every other call is recorded as doing nothing, the one through the vtable too
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..(0x4a0 / 4) {
        let w = r.mem.read_u32(0x11cae8 + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    jals.remove(&0x25aff8);
    for a in jals {
        r.stub(a, |cpu, _| cpu.set_gpr(2, 0));
    }
    r.stub(0x4000, |cpu, _| cpu.set_gpr(2, 0));
    let mut g = Rng(0x5eed_011c_ae80);
    let rider = r.mem.alloc(0x6000);
    let vt = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x100);
    let pos = r.mem.alloc(0x10);
    let vel = r.mem.alloc(0x10);
    r.mem.write_u32(vt + 0x9c, 0x4000);
    r.mem.write_u32(rider, vt);
    r.mem.write_u32(rider + 0x464, ctl);
    put_v(&mut r, pos, [10.0, 20.0, 30.0, 0.0]);
    let mut wrong = Vec::new();
    let (mut hits, mut misses, mut flagged) = (0, 0, 0);
    for n in 0..300u32 {
        let flag = g.next() % 3 == 0;
        let ray = !flag && g.next() % 2 == 0;
        let normal = probe_unit([g.unit(), g.unit(), g.unit(), 0.0]);
        *hit.borrow_mut() = if ray { Some(normal) } else { None };
        put_v(&mut r, rider + 0x2a0, [5.0, 5.0, 5.0, 5.0]);
        put_v(&mut r, rider + 0x2c0, [6.0, 6.0, 6.0, 6.0]);
        put_v(&mut r, rider + 0x1a0, [0.0, 0.0, 1.0, 0.0]);
        r.cpu.set_gpr(4, rider as u64);
        r.cpu.set_gpr(5, pos as u64);
        r.cpu.set_gpr(6, vel as u64);
        r.cpu.set_gpr(7, flag as u64);
        r.cpu.set_f(12, 0.0);
        r.cpu.set_f(13, 0.0);
        r.call(0x11cae8).expect("Boarder_PlaceAt runs to its return");
        let (gn, gu) = (get_v(&mut r, rider + 0x2a0), get_v(&mut r, rider + 0x2c0));
        let ours = bits(gt::placed_normal(if ray { Some(normal) } else { None }));
        if ray { hits += 1 } else if flag { flagged += 1 } else { misses += 1 }
        if gn != ours || gu != ours {
            wrong.push(format!("call {n}: flag {flag} ray {ray}: game normal {gn:x?} up {gu:x?}, ours {ours:x?}"));
            if wrong.len() > 10 {
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(hits > 20 && misses > 20 && flagged > 20, "hits {hits}, misses {misses}, flagged {flagged}");
}

#[test]
fn respawn_sets_the_placement_normal() {
    // the port's respawn (no ray cast yet: STANDIN) leaves the game's default normal and the up equal to it
    use glam::Vec3;
    use tricky_game::rider::Rider;
    let mut me = Rider::new(Vec3::new(0.0, 5.0, 0.0), 0.0);
    me.normal = Vec3::new(0.3, 0.8, 0.5).normalize();
    me.respawn((Vec3::new(1.0, 2.0, 3.0), 0.0));
    assert_eq!(me.normal, Vec3::Y);
    assert_eq!(me.up.map(f32::to_bits), [0.0, 0.0, 1.0, 0.0].map(f32::to_bits));
}
