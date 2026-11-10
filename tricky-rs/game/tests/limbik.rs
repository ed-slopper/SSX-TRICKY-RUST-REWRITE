//! `tricky_game::limbik` (board row G7r4) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is a fresh 19-bone skeleton (parents among earlier bones; the legs' bones of lengths
//! 5 to 30), random angles, offsets, a random root matrix and targets near and far (so the knee bends, stays
//! straight, and the turn's axis is small or not), then one of the row's functions on both sides. The sine
//! pair and `Math_Asin` are the game's own. The memory is compared.

use r5900::Runner;
use tricky_game::limbik as ik;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const PART: u32 = ARENA;
const SKIN: u32 = ARENA + 0x100;
const ANGLES: u32 = ARENA + 0x400;
const BONES: u32 = ARENA + 0x800;
const POINTS: u32 = ARENA + 0x1000;
const MATS: u32 = ARENA + 0x1400;
const ROOT: u32 = ARENA + 0x2000;
const TARGETS: u32 = ARENA + 0x2100;
const OUT: u32 = ARENA + 0x2200;

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn limb_ik_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [0x2059f8u32, 0x205a78, 0x205e98, 0x208398, 0x2083f8];
    for f in funcs.into_iter().chain([0x250c10, 0x250fd0]) {
        r.allow(f);
    }
    let mut g = Rng(0x5ce1_3000_0000_0001);
    let (mut bent, mut straight) = (0usize, 0usize);
    for case in 0..5 * 2000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(PART, ANGLES);
        m.w32(PART + 4, MATS);
        m.w32(PART + 0x18, BONES);
        m.w32(PART + 0x1c, POINTS);
        for b in 0..19u32 {
            let rec = BONES + 0x54 * b;
            // the legs (13, 14, 15 and 16, 17, 18) chains; the rest parents among the earlier bones
            let parent = match b {
                0 => 0xffff,
                14 | 15 | 17 | 18 => b - 1,
                _ => g.below(b),
            };
            m.w16(rec + 0x12, parent as u16);
            let len = { let v = g.f(5.0, 30.0); g.pick(&[10.0f32, v]) };
            wf(&mut m, rec + 0x18, len);
            wf(&mut m, rec + 0x1c, g.f(-3.0, 3.0));
            wf(&mut m, rec + 0x20, g.f(-3.0, 3.0));
            for k in 0..3 {
                wf(&mut m, ANGLES + 12 * b + 4 * k, { let v = g.f(-3.0, 3.0); g.pick(&[0.0f32, v, v]) });
                wf(&mut m, ANGLES + 0x108 + 12 * b + 4 * k, g.f(-20.0, 20.0));
            }
        }
        for k in 0..19 * 16 {
            wf(&mut m, MATS + 4 * k, g.f(-2.0, 2.0));
        }
        for k in 0..8u32 {
            let p = POINTS + 16 * k;
            for j in 0..3 {
                wf(&mut m, p + 4 * j, g.f(-10.0, 10.0));
            }
            m.w32(p + 0xc, g.below(19));
        }
        // the root: a rotation-like matrix with a place
        for k in 0..16u32 {
            let v = if k % 5 == 0 { 1.0 } else if k >= 12 { g.f(-30.0, 30.0) } else { g.f(-0.3, 0.3) };
            wf(&mut m, ROOT + 4 * k, if k == 15 { 1.0 } else { v });
        }
        // the targets: near (the knee bends) or far (straight)
        for t in 0..2u32 {
            let reach = g.pick(&[5.0f32, 20.0, 80.0, 200.0]);
            for j in 0..3 {
                wf(&mut m, TARGETS + 0x10 * t + 4 * j, g.f(-reach, reach));
            }
            wf(&mut m, TARGETS + 0x10 * t + 0xc, g.pick(&[1.0f32, 0.0]));
        }
        // the skin's part of each kind (for the two hand-offs)
        let kind = g.below(3);
        m.w32(SKIN + 0x3cf8, PART);
        m.w32(SKIN + 0x3cf8 + 4 * kind, PART);
        let i = g.below(8);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        let which = (case % 5) as usize;
        let func = funcs[which];
        let regs: Vec<u32> = match which {
            0 => {
                ik::bone_world_pos(&mut m, OUT, PART, i, MATS);
                vec![OUT, PART, i, MATS]
            }
            1 => {
                ik::pose_bones(&mut m, PART, ROOT, TARGETS, TARGETS + 0x10);
                vec![PART, ROOT, TARGETS, TARGETS + 0x10]
            }
            2 => {
                let (u, mid, e) = g.pick(&[(16u32, 17u32, 18u32), (13, 14, 15)]);
                ik::solve_limb(&mut m, PART, u, mid, e, TARGETS, MATS);
                vec![PART, u, mid, e, TARGETS, MATS]
            }
            3 => {
                ik::boarder_bone_world_pos(&mut m, OUT, SKIN, kind, i, MATS);
                vec![OUT, SKIN, kind, i, MATS]
            }
            _ => {
                ik::build_bone_matrices(&mut m, SKIN, ROOT, TARGETS, TARGETS + 0x10);
                vec![SKIN, ROOT, TARGETS, TARGETS + 0x10]
            }
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        // the knee: the middle bone's matrix turned from the upper's (bent) or the same rotation (straight-ish)
        if matches!(which, 1 | 2 | 4) {
            let up = f32::from_bits(m.r32(MATS + 0x40 * 16));
            let mid = f32::from_bits(m.r32(MATS + 0x40 * 17));
            if (up - mid).abs() > 0.05 { bent += 1 } else { straight += 1 }
        }
    }
    println!("{bent} bent, {straight} near straight");
    assert!(bent > 500 && straight > 100);
}
