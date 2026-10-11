//! The rider's stat bytes against the original's, bit for bit (board row G20a): `Career_SetupPlayerRider` 0x16a9d8
//! and `Career_SetupAIRider` 0x16ae68 (with `Career_AverageStatGroups` 0x16b298 running as the original) run in the
//! function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), their other calls stubbed, and the
//! 16 bytes they leave at entry+0x0e..0x1d equal `tricky_game::stats`. The tables in `stats.rs` and `trickdata.rs`
//! are compared with the exe's own.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::stats::{self, BASE, CAPS, GROUPS};
use tricky_game::trickdata::{BOARDS, RIDERS};

const MAX_ATTRIB: u32 = 0x3352ac; // gCheat_MaxAttrib
const MALLORA: u32 = 0x3352b0; // gCheat_Mallora
const ALL_MAX: u32 = 0x3352b8; // the word that sets the entry's +0x80 to 0
const MODE: u32 = 0x32de8c; // the game mode (6 = trick tutorial)

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // calls that do not touch the stat bytes: recorded as doing nothing
    for a in [0x2fd14c, 0x166550, 0x164840, 0x16b1f8, 0x16b220, 0x16b248, 0x16b270, 0x16b1d0, 0x23d4a8] {
        r.stub(a, |cpu, _| cpu.set_gpr(2, 0));
    }
    // the average of each group runs as the original
    r.allow(0x16b298);
    // EAMem_Copy(dst, src, n)
    r.stub(0x2ce114, |cpu, mem| {
        let (d, s, n) = (cpu.gpr(4) as u32, cpu.gpr(5) as u32, cpu.gpr(6) as usize);
        let mut b = vec![0u8; n];
        mem.read(s, &mut b).unwrap();
        mem.write(d, &b).unwrap();
    });
    Some(r)
}

fn read_bytes(r: &mut Runner, at: u32, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    r.mem.read(at, &mut b).unwrap();
    b
}

#[test]
fn tables_are_the_exes() {
    let Some(mut r) = runner() else { return };
    let mut wrong = Vec::new();
    // the group map: count, then the byte positions
    for (g, idx) in GROUPS.iter().enumerate() {
        let at = 0x335260 + g as u32 * 16;
        let count = r.mem.read_u32(at) as usize;
        let game: Vec<usize> = (0..count).map(|k| r.mem.read_u32(at + 4 + k as u32 * 4) as usize).collect();
        if game != *idx {
            wrong.push(format!("group {g}: game {game:?}, ours {idx:?}"));
        }
    }
    for i in 0..12 {
        let row = 0x331ea8 + i as u32 * 0x38;
        if read_bytes(&mut r, row + 0xc, 16) != BASE[i] {
            wrong.push(format!("rider {i} base bytes"));
        }
        if read_bytes(&mut r, row + 0x1c, 4) != CAPS[i] {
            wrong.push(format!("rider {i} cap bytes"));
        }
        // trickdata's attributes are the group base and cap, the jump, the wind-up and the weight
        let avg = stats::average_groups(&BASE[i]);
        let d = &RIDERS[i];
        for (g, got) in [d.edging, d.speed, d.stability, d.tricks].into_iter().enumerate() {
            if got != (avg[g], CAPS[i][g]) {
                wrong.push(format!("{} group {g}: trickdata {got:?}, game ({}, {})", d.name, avg[g], CAPS[i][g]));
            }
        }
        if (d.jump, d.windup) != (BASE[i][stats::JUMP], BASE[i][10]) {
            wrong.push(format!("{} jump/wind-up", d.name));
        }
        // the probe's scale, stats+0x60: the row's float at +0x24
        if r.mem.read_u32(row + 0x24) != stats::SCALE[i].to_bits() {
            wrong.push(format!("{} scale: game {:#x}, ours {:#x}", d.name, r.mem.read_u32(row + 0x24), stats::SCALE[i].to_bits()));
        }
        if d.weight as u32 != r.mem.read_u32(row + 0x20) {
            wrong.push(format!("{} weight", d.name));
        }
        // the boards' bonuses: 12 per rider, 20 bytes each, the four bytes at +16 (0x332148 + (rider · 0x3c + board · 5 + 4) · 4)
        for b in 0..12 {
            let at = 0x332148 + (i as u32 * 0x3c + b as u32 * 5 + 4) * 4;
            if read_bytes(&mut r, at, 4) != BOARDS[i][b].bonus {
                wrong.push(format!("{} board {b} ({}) bonus", d.name, BOARDS[i][b].name));
            }
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn group_averages_are_the_originals() {
    // `Career_AverageStatGroups` 0x16b298 on its own: any 16 bytes in, four averages out
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x5eed_0016_b298);
    let (src, dst) = (r.mem.alloc(16), r.mem.alloc(8));
    for n in 0..20_000u32 {
        let mut base = [0u8; 16];
        for b in base.iter_mut() {
            *b = match g.next() % 8 {
                0 => 0,
                1 => 100,
                2 => 255,
                _ => (g.next() % 101) as u8,
            };
        }
        r.mem.write(src, &base).unwrap();
        r.mem.write(dst, &[0xaa; 8]).unwrap();
        r.cpu.set_gpr(4, 0x32b390);
        r.cpu.set_gpr(5, dst as u64);
        r.cpu.set_gpr(6, src as u64);
        r.call(0x16b298).expect("Career_AverageStatGroups runs to its return");
        let game = read_bytes(&mut r, dst, 4);
        assert_eq!(game, stats::average_groups(&base), "call {n}: base {base:?}");
        assert_eq!(read_bytes(&mut r, dst + 4, 4), [0xaa; 4], "wrote past the four averages");
    }
}

#[test]
fn player_stat_bytes_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x5eed_0016_a9d8);
    let career = r.mem.alloc(0x40);
    let rec = r.mem.alloc(0x40);
    let pv3 = r.mem.alloc(0x40);
    let lv = r.mem.alloc(0x20);
    let entry = r.mem.alloc(0x90);
    let name = r.mem.alloc(8);
    let mut wrong = Vec::new();
    let mut runs = 0;
    let mut caps_runs = 0;
    let mut scale_runs = 0;
    for n in 0..20_000u32 {
        let rider = (g.next() % 12) as usize;
        let board = (g.next() % 12) as usize;
        // levels: anywhere from 0 to 100, with the edges and values under the group's average
        let lvl = |g: &mut Rng| match g.next() % 6 {
            0 => 0,
            1 => 100,
            _ => (g.next() % 101) as u8,
        };
        let levels = [lvl(&mut g), lvl(&mut g), lvl(&mut g), lvl(&mut g)];
        let (max_attrib, mallora, all_max, mode) = if n % 4 == 0 { (g.next() % 2, g.next() % 2, g.next() % 2, [6, 0, 1][(g.next() % 3) as usize]) } else { (0, 0, 0, 0) };
        r.mem.write_u32(MAX_ATTRIB, max_attrib);
        r.mem.write_u32(MALLORA, mallora);
        r.mem.write_u32(ALL_MAX, all_max);
        r.mem.write_u32(MODE, mode);
        for k in 0..0x40 {
            r.mem.write_u8(entry + k, 0xaa);
        }
        r.mem.write_u8(rec, 0);
        r.mem.write_u32(rec + 4, 0);
        r.mem.write_u32(rec + 0xc, name);
        r.mem.write_u32(rec + 0x14, pv3);
        r.mem.write_u32(rec + 0x18, career);
        r.mem.write_u32(pv3, rider as u32);
        r.mem.write_u32(pv3 + 4, 0);
        r.mem.write_u32(pv3 + 0xc, 0);
        r.mem.write_u32(pv3 + 0x10, board as u32);
        for k in 0..4 {
            r.mem.write_u8(career + 0xc + k, levels[k as usize]);
        }
        let _ = lv;
        r.cpu.set_gpr(4, career as u64);
        r.cpu.set_gpr(5, rec as u64);
        r.cpu.set_gpr(6, entry as u64);
        r.call(0x16a9d8).expect("Career_SetupPlayerRider runs to its return");
        let game = read_bytes(&mut r, entry + 0xe, 16);
        // the original's own condition for the max-attribute path
        let b1 = all_max != 0 || (mallora != 0 && rider == 7);
        let mix = (max_attrib == 0 && !b1) || mode == 6;
        let ours = stats::player_bytes(&BASE[rider], &CAPS[rider], BOARDS[rider][board].bonus, levels, !mix);
        // stats+0x60, the probe's scale
        let (game_scale, our_scale) = (r.mem.read_u32(entry + 0x60), stats::player_scale(rider, all_max != 0, mode).to_bits());
        if game_scale != our_scale {
            wrong.push(format!("rider {rider} all_max {all_max} mode {mode}: scale game {game_scale:#x}, ours {our_scale:#x}"));
        }
        scale_runs += (all_max != 0 && mode != 6) as u32;
        runs += 1;
        caps_runs += (!mix) as u32;
        if game != ours {
            wrong.push(format!("rider {rider} board {board} levels {levels:?} caps {}: game {game:?}, ours {ours:?}", !mix));
            if wrong.len() > 10 {
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {runs} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(caps_runs > 100, "the max-attribute path was run {caps_runs} times");
    assert!(scale_runs > 100, "the all-max scale was run {scale_runs} times");
}

#[test]
fn ai_stat_bytes_are_the_originals() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x5eed_0016_ae68);
    let val = r.mem.alloc(8);
    let entry = r.mem.alloc(0x90);
    let mut wrong = Vec::new();
    let mut runs = 0;
    for n in 0..20_000u32 {
        let rider = (g.next() % 12) as usize;
        let value = match g.next() % 5 {
            0 => 0,
            1 => 400,
            2 => -(g.next() as i32 % 30),
            _ => (g.next() % 400) as i32,
        };
        let null = n % 7 == 0;
        let max_attrib = if n % 5 == 0 { g.next() % 2 } else { 0 };
        r.mem.write_u32(MAX_ATTRIB, max_attrib);
        r.mem.write_u32(val, value as u32);
        for k in 0..0x40 {
            r.mem.write_u8(entry + k, 0xaa);
        }
        r.cpu.set_gpr(4, 0x32b390);
        r.cpu.set_gpr(5, (0x331ea8 + rider as u32 * 0x38) as u64);
        r.cpu.set_gpr(6, if null { 0 } else { val as u64 });
        r.cpu.set_gpr(7, entry as u64);
        r.call(0x16ae68).expect("Career_SetupAIRider runs to its return");
        let game = read_bytes(&mut r, entry + 0xe, 16);
        let ours = stats::ai_bytes(&BASE[rider], &CAPS[rider], if null { None } else { Some(value) }, max_attrib != 0);
        let (game_scale, our_scale) = (r.mem.read_u32(entry + 0x60), stats::ai_scale(rider).to_bits());
        if game_scale != our_scale {
            wrong.push(format!("rider {rider}: scale game {game_scale:#x}, ours {our_scale:#x}"));
        }
        runs += 1;
        if game != ours {
            wrong.push(format!("rider {rider} value {value} null {null} max {max_attrib}: game {game:?}, ours {ours:?}"));
            if wrong.len() > 10 {
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {runs} differ:\n{}", wrong.len(), wrong.join("\n"));
}

// ---- the lean record: rider+0x244 value, +0x248 rate, +0x24c target (board row G20b)

const LEAN: u32 = 0x244;

fn float_edge(g: &mut Rng) -> f32 {
    match g.next() % 10 {
        0 => 0.0,
        1 => 1.0,
        2 => -1.0,
        3 => 0.905,
        4 => -0.905,
        5 => (g.next() as f32 / 65536.0 - 0.5) * 8.0,
        _ => ((g.next() % 20_001) as f32 - 10_000.0) / 10_000.0,
    }
}

#[test]
fn lean_targets_are_the_originals() {
    // `Boarder_UpdateLeanTargets` 0x129500: the rate (+0x248) and target (+0x24c) it leaves, in every motion, over the
    // steers and the speeds under and over its 1.44 cm/s knee
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x5eed_0012_9500);
    let rider = r.mem.alloc(0x400);
    let mut wrong = Vec::new();
    let (mut slow, mut rail) = (0, 0);
    for n in 0..20_000u32 {
        let motion = [1u32, 2, 2, 2, 3, 5, 6, 4][(g.next() % 8) as usize];
        let steer = float_edge(&mut g);
        let speed = match g.next() % 4 {
            0 => 0.0,
            1 => (g.next() % 3000) as f32 / 1000.0,
            _ => (g.next() % 300_000) as f32 / 100.0,
        };
        // a direction, so |v| = speed up to rounding; the fourth word is the vector's w
        let (a, b) = ((g.next() % 6283) as f32 / 1000.0, (g.next() % 3141) as f32 / 1000.0);
        let vel = [speed * b.sin() * a.cos(), speed * b.sin() * a.sin(), speed * b.cos(), 0.0];
        r.mem.write_u32(rider + 0x424, motion);
        r.mem.write_f32(rider + 0x214, steer);
        for k in 0..4 {
            r.mem.write_f32(rider + 0x150 + 4 * k, vel[k as usize]);
        }
        // the half that makes the rider's up (+0x2c0) needs a normal and an up
        for (k, v) in [0.0f32, 0.0, 1.0, 0.0].into_iter().enumerate() {
            r.mem.write_f32(rider + 0x2a0 + 4 * k as u32, v);
            r.mem.write_f32(rider + 0x2c0 + 4 * k as u32, v);
        }
        for k in 0..3 {
            r.mem.write_u32(rider + LEAN + 4 * k, 0xdead_0000 + k);
        }
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x129500).expect("Boarder_UpdateLeanTargets runs to its return");
        let (rate, target) = tricky_game::rider::ground_tick::lean_targets(motion, steer, vel);
        let (game_rate, game_target) = (r.mem.read_u32(rider + LEAN + 4), r.mem.read_u32(rider + LEAN + 8));
        slow += (motion != 3 && (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt() * 0.6944444 < 1.0) as u32;
        rail += (motion == 3) as u32;
        // the value itself (+0x244) is not touched
        if r.mem.read_u32(rider + LEAN) != 0xdead_0000 {
            wrong.push(format!("call {n}: wrote the lean value"));
        }
        if (game_rate, game_target) != (rate.to_bits(), target.to_bits()) {
            wrong.push(format!("call {n}: motion {motion} steer {steer} vel {vel:?}: game ({game_rate:#x}, {game_target:#x}), ours ({:#x}, {:#x})", rate.to_bits(), target.to_bits()));
            if wrong.len() > 10 {
                break;
            }
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(slow > 1000 && rail > 1000, "knee {slow}, rail {rail}");
}

#[test]
fn lean_ease_is_the_originals() {
    // `cBoarder::vf7` 0x117198 run whole, everything it calls recorded as doing nothing: the lean value (+0x244)
    // after it is `lean_ease` of the record before it
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x5eed_0011_7198);
    let rider = r.mem.alloc(0x6000);
    let vt = r.mem.alloc(0x100);
    let game = r.mem.alloc(0x100);
    let tbl = r.mem.alloc(0x400);
    let sub = r.mem.alloc(0x100);
    // every call the function makes is a stub; the one through the vtable (slot +0x54) too
    let code_start = 0x117198u32;
    let mut jals = std::collections::BTreeSet::new();
    for k in 0..(2952 / 4) {
        let w = r.mem.read_u32(code_start + 4 * k);
        if w >> 26 == 3 {
            jals.insert((w & 0x03ff_ffff) << 2);
        }
    }
    for a in jals {
        r.stub(a, |cpu, _| cpu.set_gpr(2, 0));
    }
    r.stub(0x4000, |cpu, _| cpu.set_gpr(2, 0));
    r.mem.write_u32(vt + 0x54, 0x4000);
    r.mem.write_u32(rider, vt);
    r.mem.write_u32(0x337c58 + 0x730, game);
    r.mem.write_u32(game + 0x24, tbl);
    r.mem.write_u32(game + 0x1c, sub);
    r.mem.write_u32(sub + 0x1c, 1);
    let dt = 1.0f32 / 60.0;
    let mut wrong = Vec::new();
    let (mut up, mut down, mut near) = (0, 0, 0);
    for n in 0..20_000u32 {
        let lean = [float_edge(&mut g) * 60.0, float_edge(&mut g).abs() * 70.0, float_edge(&mut g) * 60.0];
        r.mem.write_u32(rider + 0x46b0, 1);
        for k in 0..3 {
            r.mem.write_f32(rider + LEAN + 4 * k as u32, lean[k]);
        }
        r.cpu.set_gpr(4, rider as u64);
        r.call(0x117198).expect("cBoarder::vf7 runs to its return");
        let game_value = r.mem.read_u32(rider + LEAN);
        let ours = tricky_game::rider::ground_tick::lean_ease(lean, dt);
        let step = lean[1] * dt;
        up += (lean[2] - step > lean[0]) as u32;
        down += (lean[2] + step < lean[0]) as u32;
        near += (lean[2] + step >= lean[0] && lean[2] - step <= lean[0]) as u32;
        if game_value != ours.to_bits() {
            wrong.push(format!("call {n}: lean {lean:?}: game {game_value:#x}, ours {:#x}", ours.to_bits()));
            if wrong.len() > 10 {
                break;
            }
        }
        // the rate and target are left alone (the stub stands for `Boarder_UpdateLeanTargets`)
        assert_eq!((r.mem.read_f32(rider + LEAN + 4), r.mem.read_f32(rider + LEAN + 8)), (lean[1], lean[2]));
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(up > 1000 && down > 1000 && near > 1000, "up {up}, down {down}, within a step {near}");
}
