//! The remaining `Rand_Fx` users of row E1b2i against the original's, in the function runner on the player's
//! SLUS_203.26 (TRICKY_ELF; skipped without it): `Speech_RollChance` 0x235d58, `FUN_00224658` (the sound bank's
//! variant pick) and `FUN_00169378` (the roster's per-rider look picks), with `Rand_Fx` answered from the same
//! script on both sides.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::fxrand::{bank_variant, rider_scene, speech_roll, RiderScene, SceneTables, Slot, NO_RIDER};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn at(r: &mut Runner, addr: u32, n: usize) -> Vec<u8> {
    (0..n as u32 / 4).flat_map(|i| r.mem.read_u32(addr + 4 * i).to_le_bytes()).collect()
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn speech_roll_and_bank_variant_match_the_original() {
    let Some(mut r) = runner() else { return };
    let q: Rc<RefCell<u32>> = Rc::default();
    let qq = q.clone();
    r.stub(0x23d4a8, move |c, _| c.set_gpr(2, *qq.borrow() as i32 as i64 as u64));
    r.allow(0x235d58);
    r.allow(0x224658);
    let table: Vec<i32> = at(&mut r, 0x3a3528, 40 * 10 * 4).chunks(4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let sys = r.mem.alloc(0x7000);
    // FUN_00224658: the generator is the sound system's virtual at DAT_003436f8 -> +4 -> vtable +0x20 (delta), +0x24
    let obj = r.mem.alloc(0x40);
    let vt = r.mem.alloc(0x100);
    r.mem.write_u32(0x3436f8, obj);
    r.mem.write_u32(obj + 4, vt);
    r.mem.write_u16(vt + 0x20, 0);
    r.mem.write_u32(vt + 0x24, 0x23d4a8);
    let bank = r.mem.alloc(0x100);
    let mut g = Rng(0x3c91_07a5_e2d4_b618);
    let mut played = 0;
    for case in 0..30_000 {
        let rv = g.next() ^ (g.next() << 16);
        *q.borrow_mut() = rv;
        let level = [0.0f32, 1.0, 2.5, 3.99, 9.0, 12.0, -1.0][g.next() as usize % 7];
        let cap = (g.next() % 12) as i32 - 1;
        let kind = (g.next() % 24) as usize;
        r.mem.write_f32(sys + 0x383c, level);
        r.mem.write_u32(sys + 0x6e68, cap as u32);
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, kind as u64);
        let ours = speech_roll(rv, level, cap, kind, &table);
        r.call(0x235d58).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = r.cpu.gpr(2) as u32 != 0;
        // an index outside 0..10 reads the neighbouring row: the same flat table
        if let Some(o) = ours {
            assert_eq!(theirs, o, "case {case}: kind {kind} level {level} cap {cap} rand {rv:#x}");
            played += o as u32;
        }
        let count = 1 + (g.next() % 9) as i32;
        r.mem.write_u32(bank + 0xf8, count as u32);
        r.cpu.set_gpr(4, bank as u64);
        r.call(0x224658).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 as i32, bank_variant(rv, count), "case {case}: variant of {count} for {rv:#x}");
    }
    assert!(played > 3000, "rolls that played: {played}");
}

#[test]
fn rider_scene_matches_the_original() {
    let Some(mut r) = runner() else { return };
    r.allow(0x169378);
    let q: Rc<RefCell<Vec<u32>>> = Rc::default();
    let qq = q.clone();
    let calls: Rc<RefCell<u32>> = Rc::default();
    let cc = calls.clone();
    r.stub(0x23d4a8, move |c, _| {
        *cc.borrow_mut() += 1;
        let v = qq.borrow_mut().remove(0);
        c.set_gpr(2, v as i32 as i64 as u64)
    });
    // table a (0x3cc028) is built at run time (the file holds zeros): scripted rows, the same on both sides
    let mut g = Rng(0x61d2_f00d_4ab7_9c33);
    let mut ta = vec![0u8; 0x1ae * 10 + 4];
    for id in 0..0x1ae {
        for k in 0..6 {
            ta[id * 10 + k] = g.next() as u8;
        }
        let base = (g.next() % 1500) as i16;
        let count = if g.next() % 5 == 0 { 0 } else { 1 + (g.next() % 6) as i16 };
        ta[id * 10 + 6..id * 10 + 8].copy_from_slice(&base.to_le_bytes());
        ta[id * 10 + 8..id * 10 + 10].copy_from_slice(&count.to_le_bytes());
    }
    for (i, w) in ta.chunks(4).enumerate() {
        let mut b = [0u8; 4];
        b[..w.len()].copy_from_slice(w);
        r.mem.write_u32(0x3cc028 + 4 * i as u32, u32::from_le_bytes(b));
    }
    let tb = at(&mut r, 0x32f468, 0x6000 * 4);
    let tc = at(&mut r, 0x37cd70, 500 * 8);
    let t = SceneTables { a: &ta, b: &tb, c: &tc };
    let h = |o: usize| i16::from_le_bytes([ta[o], ta[o + 1]]) as i32;
    // the look ids with variants (a count of 0 traps in the game)
    let valid: Vec<i32> = (0..0x1ad).filter(|&id| h(id as usize * 10 + 8) > 0).collect();
    assert!(valid.len() > 10, "valid ids: {}", valid.len());
    let roster = r.mem.alloc(0x100);
    let scene = r.mem.alloc(0x33c + 15 * 100 + 0x40);
    let mut drawn = 0;
    for case in 0..4000 {
        let mut ids = [0i32; 15];
        let mut firsts = [0i32; 15];
        for i in 0..15 {
            ids[i] = if g.next() % 4 == 0 { NO_RIDER } else { valid[g.next() as usize % valid.len()] };
            firsts[i] = g.next() as i32;
            r.mem.write_u32(roster + 0x14 + 4 * i as u32, ids[i] as u32);
            r.mem.write_u32(roster + 0x50 + 4 * i as u32, firsts[i] as u32);
            r.mem.write_u32(roster + 0x8c + 4 * i as u32, 0x5a5a5a5a);
        }
        // scene records start with random words (what a slot keeps when its key is not in the table)
        let mut ours = [RiderScene::default(); 15];
        for i in 0..15 {
            ours[i].first = g.next() as i32;
            for s in ours[i].slots.iter_mut() {
                *s = Slot { a: g.next() as i32, b: g.next() as i32, c: g.next() as i32, d: g.next() as i32 };
            }
            let base = scene + 0x33c + 100 * i as u32;
            r.mem.write_u32(base, ours[i].first as u32);
            for (k, s) in ours[i].slots.iter().enumerate() {
                for (j, v) in [s.a, s.b, s.c, s.d].iter().enumerate() {
                    r.mem.write_u32(base + 4 + 16 * k as u32 + 4 * j as u32, *v as u32);
                }
            }
        }
        let forced = if g.next() % 3 == 0 { (g.next() % 3) as i32 } else { -1 };
        let draws: Vec<u32> = (0..15).map(|_| g.next() ^ (g.next() << 16)).collect();
        *q.borrow_mut() = draws.clone();
        *calls.borrow_mut() = 0;
        r.cpu.set_gpr(4, roster as u64);
        r.cpu.set_gpr(5, scene as u64);
        r.cpu.set_gpr(6, forced as i64 as u64);
        r.call(0x169378).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut k = 0;
        let mut picks = [0x5a5a5a5ai32; 15];
        rider_scene(&ids, &firsts, forced, &mut || { let v = draws[k]; k += 1; v }, &t, &mut ours, &mut picks);
        drawn += k;
        assert_eq!(*calls.borrow() as usize, k, "case {case}: draws");
        for i in 0..15 {
            let base = scene + 0x33c + 100 * i as u32;
            let first = r.mem.read_u32(base) as i32;
            assert_eq!(first, ours[i].first, "case {case} rider {i}: first");
            for s in 0..6 {
                let o = ours[i].slots[s];
                let w = |j: u32, m: &mut r5900::Mem| m.read_u32(base + 4 + 16 * s as u32 + 4 * j) as i32;
                let theirs = Slot { a: w(0, &mut r.mem), b: w(1, &mut r.mem), c: w(2, &mut r.mem), d: w(3, &mut r.mem) };
                assert_eq!(theirs, o, "case {case} rider {i} slot {s} (id {:#x})", ids[i]);
            }
            assert_eq!(r.mem.read_u32(roster + 0x8c + 4 * i as u32) as i32, picks[i], "case {case} rider {i}: pick");
        }
    }
    assert!(drawn > 20_000, "draws: {drawn}");
}
