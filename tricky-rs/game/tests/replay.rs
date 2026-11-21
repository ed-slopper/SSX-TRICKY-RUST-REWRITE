//! Our functions against the running game, tick by tick (board row F4h): replays a race recorded over PINE
//! (`tools/pine.py record <seconds> <file>`, TRICKY_RECORDING; skipped without it) through our exact ports.
//!
//! The air step: each tick the game calls `Air_IntegrateRK4` 0x12b340 on the rider itself, with
//! dt = time scale (rider+0x12c) · 0.016666668, position rider+0x140, velocity rider+0x150, no sub-steps.
//! For every pair of consecutive recorded ticks we run ours from tick t and look for tick t+1's position and
//! velocity: a rider in the air gets exactly that (nothing else moves it in the air unless it hits something).

use tricky_game::air::{integrate_rk4, predict_at_takeoff, Predict, RayHit};
use tricky_game::jump::{jump_impulse, JumpState};
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::spin::{landing_align, spin_update, AlignRider, Anim, SpinObj, SpinRider};

mod course_clock;
mod recording;
use recording::{f32_at, load, player_pad, u32_le, vec4};

#[test]
fn air_steps_match_the_running_game() {
    let Ok(paths) = std::env::var("TRICKY_RECORDING") else { return };
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let (n, ticks) = load(path);
        let (mut pairs, mut exact, mut per_rider, mut near) = (0, 0, vec![(0, 0); n], [0; 4]);
        for w in ticks.windows(2) {
            if w[1].tick != w[0].tick + 1 {
                continue;
            }
            for (i, (a, b)) in w[0].riders.iter().zip(&w[1].riders).enumerate() {
                let dt = f32_at(a, 0x12c) * 0.016666668;
                let (mut p, mut v) = (vec4(a, 0x140), vec4(a, 0x150));
                integrate_rk4(dt, &mut p, &mut v, false);
                pairs += 1;
                if std::env::var("TRICKY_REPLAY_RIDER").is_ok_and(|r| r == i.to_string()) {
                    let ok = p.map(f32::to_bits) == vec4(b, 0x140).map(f32::to_bits) && v.map(f32::to_bits) == vec4(b, 0x150).map(f32::to_bits);
                    eprintln!("  tick {} rider {i}: {}", w[0].tick, if ok { "air step" } else { "-" });
                }
                per_rider[i].0 += 1;
                if p.map(f32::to_bits) == vec4(b, 0x140).map(f32::to_bits) && v.map(f32::to_bits) == vec4(b, 0x150).map(f32::to_bits) {
                    exact += 1;
                    per_rider[i].1 += 1;
                } else {
                    // how far off: the worst lane in ulps (1, 2-4, 5-64, more: not an air step)
                    let ulps = |x: f32, y: f32| (x.to_bits() as i64 - y.to_bits() as i64).unsigned_abs();
                    let (gp, gv) = (vec4(b, 0x140), vec4(b, 0x150));
                    let worst = (0..4).map(|k| ulps(p[k], gp[k]).max(ulps(v[k], gv[k]))).max().unwrap();
                    if (2..=64).contains(&worst) {
                        eprintln!("  tick {} rider {i}: off by {worst} ulp: ours {p:?} {v:?}, game {gp:?} {gv:?}, from {:?} {:?} dt {dt}",
                            w[0].tick, vec4(a, 0x140), vec4(a, 0x150));
                    }
                    near[match worst { 1 => 0, 2..=4 => 1, 5..=64 => 2, _ => 3 }] += 1;
                }
            }
        }
        eprintln!("{path}: {exact} of {pairs} rider-ticks are exactly our air step; per rider (ticks, exact): {per_rider:?};             the rest off by 1 ulp, 2-4, 5-64, more: {near:?}");
    }
}

/// The rubber band: AI riders' `AI_RubberBandSpeedScale` 0x13a0f0 (with `Math_Cos` 0x250e98) run in the function
/// runner on the rider as recorded at tick t, then `Boarder_SetTimeScaleRateLimited` 0x11cff0 (at most
/// 0.008446341 a tick toward it), against the time scale (rider+0x12c) recorded at tick t+1.
#[test]
fn rubber_band_matches_the_running_game() {
    let Ok(paths) = std::env::var("TRICKY_RECORDING") else { return };
    let Some(mut r) = r5900::Runner::from_env() else { return };
    // round to nearest unless TRICKY_FLOAT_RULES says otherwise (to see that the replay tells the rules apart)
    let rules = std::env::var("TRICKY_FLOAT_RULES").unwrap_or_else(|_| "add=n,mul=n,div=n,cvt=n".into());
    r.float_rules = r5900::ps2float::Rules::parse(&rules).unwrap();
    r.allow(0x250e98);
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let (n, ticks) = load(path);
        let rider = r.mem.alloc(ticks[0].riders[0].len() as u32);
        let ai = r.mem.alloc(16);
        r.mem.write_u32(ai, rider);
        let (mut pairs, mut exact, mut per_rider) = (0, 0, vec![(0, 0); n]);
        for w in ticks.windows(2) {
            if w[1].tick != w[0].tick + 1 {
                continue;
            }
            for (i, (a, b)) in w[0].riders.iter().zip(&w[1].riders).enumerate() {
                r.mem.write(rider, a).unwrap();
                r.cpu.set_gpr(4, ai as u64);
                r.call(0x13a0f0).unwrap_or_else(|e| panic!("tick {} rider {i}: {e}", w[0].tick));
                let target = r.cpu.f(0);
                let now = f32_at(a, 0x12c);
                let next = if target + 0.008446341 < now {
                    now - 0.008446341
                } else if now < target - 0.008446341 {
                    now + 0.008446341
                } else {
                    target
                };
                pairs += 1;
                per_rider[i].0 += 1;
                if next.to_bits() == f32_at(b, 0x12c).to_bits() {
                    exact += 1;
                    per_rider[i].1 += 1;
                } else if std::env::var("TRICKY_REPLAY_RIDER").is_ok_and(|x| x == i.to_string()) {
                    eprintln!("  tick {} rider {i}: ours {next} (target {target}) vs the game's {}, from {now}", w[0].tick, f32_at(b, 0x12c));
                }
            }
        }
        eprintln!("{path}: rubber band {exact} of {pairs} rider-ticks exact; per rider (ticks, exact): {per_rider:?}");
    }
}

/// Which float rules the running game's air steps agree with: the game's own `Air_IntegrateRK4` in the
/// function runner under TRICKY_FLOAT_RULES (default: to nearest) on every recorded tick pair (board row F4g).
#[test]
fn air_steps_tell_the_float_rules_apart() {
    let Ok(paths) = std::env::var("TRICKY_RECORDING") else { return };
    let Some(mut r) = r5900::Runner::from_env() else { return };
    let rules = std::env::var("TRICKY_FLOAT_RULES").unwrap_or_else(|_| "add=n,mul=n,div=n,cvt=n".into());
    r.float_rules = r5900::ps2float::Rules::parse(&rules).unwrap();
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let (_, ticks) = load(path);
        let rider = r.mem.alloc(ticks[0].riders[0].len() as u32);
        let (mut ours, mut runner) = (0, 0);
        for w in ticks.windows(2) {
            if w[1].tick != w[0].tick + 1 {
                continue;
            }
            for (a, b) in w[0].riders.iter().zip(&w[1].riders) {
                let dt = f32_at(a, 0x12c) * 0.016666668;
                let (mut p, mut v) = (vec4(a, 0x140), vec4(a, 0x150));
                integrate_rk4(dt, &mut p, &mut v, false);
                let (gp, gv) = (vec4(b, 0x140).map(f32::to_bits), vec4(b, 0x150).map(f32::to_bits));
                if p.map(f32::to_bits) != gp || v.map(f32::to_bits) != gv {
                    continue; // not an air step (or nudged): only the airborne ticks
                }
                ours += 1;
                r.mem.write(rider, a).unwrap();
                r.cpu.set_f(12, dt);
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_gpr(5, (rider + 0x140) as u64);
                r.cpu.set_gpr(6, (rider + 0x150) as u64);
                r.cpu.set_gpr(7, 0);
                r.call(0x12b340).unwrap();
                let (rp, rv) = ((0..4).map(|k| r.mem.read_u32(rider + 0x140 + 4 * k)).collect::<Vec<_>>(), (0..4).map(|k| r.mem.read_u32(rider + 0x150 + 4 * k)).collect::<Vec<_>>());
                if rp == gp && rv == gv {
                    runner += 1;
                }
            }
        }
        eprintln!("{path}: under {rules} the runner gives the game's air step on {runner} of {ours}");
        // `AirPredict_Tick` 0x123ec8 (three COP1 `add.s`/`sub.s`) on the prediction record (rider+0x57b0, F4h2)
        let Some(k) = ticks[0].extra.iter().position(|(off, _)| *off == 0x57b0) else { continue };
        let rec = r.mem.alloc(0x60);
        let (mut steps, mut same) = (0, 0);
        for w in ticks.windows(2) {
            if w[1].tick != w[0].tick + 1 {
                continue;
            }
            for (pa, pb) in w[0].extra[k].1.iter().zip(&w[1].extra[k].1) {
                let x4c = |p: &[u8]| f32_at(p, 0x4c);
                if x4c(pb) == 0.0 || x4c(pa) == 0.0 && pb[8..0x4c] != pa[8..0x4c] || f32_at(pa, 0x44) == 0.0 && f32_at(pa, 0x48) == 0.0 {
                    continue; // not a tick in the air (or a fresh prediction, or nothing to count down)
                }
                r.mem.write(rec, pa).unwrap();
                r.cpu.set_gpr(4, rec as u64);
                r.call(0x123ec8).unwrap();
                let got: Vec<u32> = [0x4c, 8, 0x50].iter().map(|o| r.mem.read_u32(rec + o)).collect();
                let want: Vec<u32> = [0x4c, 8, 0x50].iter().map(|o| f32_at(pb, *o).to_bits()).collect();
                // one tick apart only (the recorder sometimes falls a frame behind: those are left out)
                if ((x4c(pb) - x4c(pa)) * 60.0 - 1.0).abs() < 0.01 {
                    steps += 1;
                    same += (got == want) as usize;
                }
            }
        }
        eprintln!("{path}: under {rules} the runner gives the game's AirPredict_Tick on {same} of {steps} single ticks");
    }
}

fn is_air_step(a: &[u8], b: &[u8]) -> bool {
    let dt = f32_at(a, 0x12c) * 0.016666668;
    let (mut p, mut v) = (vec4(a, 0x140), vec4(a, 0x150));
    integrate_rk4(dt, &mut p, &mut v, false);
    p.map(f32::to_bits) == vec4(b, 0x140).map(f32::to_bits) && v.map(f32::to_bits) == vec4(b, 0x150).map(f32::to_bits)
}

/// Takeoffs (row F4h2): the ticks a rider in the jump state (0xa) leaves the snow (air steps from the next
/// tick on). The game's `Jump_ApplyImpulse` 0x1284e0 runs in the function runner on the rider recorded at the
/// takeoff tick (stat block from TRICKY_STATS: `rider stats-address hex` lines saved over PINE in the same race;
/// scoring stubbed). The game's whole tick also runs the ground forces, so the check is what is left of the
/// game's velocity change once the impulse is taken out: a tick of gravity and ground forces, a few cm/s.
#[test]
fn takeoffs_against_the_running_game() {
    let (Ok(paths), Ok(stats)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS")) else { return };
    let Some(mut r) = r5900::Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x250e98, 0x250d60, 0x102f50] {
        r.allow(f); // Math_Cos, Math_Sin, Vec4_Scale
    }
    r.stub(0x156618, |c, _| c.set_f(0, 0.0)); // scoring
    r.stub(0x11b018, |_, _| {}); // the meter
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let path = paths.split(';').next().unwrap();
    let (n, ticks) = load(path);
    let rider = r.mem.alloc(0x6000);
    let stat = r.mem.alloc(0x100);
    let mut worst: f32 = 0.0;
    let mut jumps = 0;
    for w in ticks.windows(3) {
        if w[1].tick != w[0].tick + 1 || w[2].tick != w[1].tick + 1 {
            continue;
        }
        for i in 0..n {
            let (a, b, c) = (&w[0].riders[i], &w[1].riders[i], &w[2].riders[i]);
            let state = u32::from_le_bytes(a[0x428..0x42c].try_into().unwrap());
            if state != 0xa || is_air_step(a, b) || !is_air_step(b, c) {
                continue;
            }
            r.mem.write(rider, a).unwrap();
            r.mem.write(stat, &stats[i]).unwrap();
            r.mem.write_u32(rider + 0x464, stat);
            // the smallest jump: the caller's argument (0 from 0x128470, 630.88 from 0x1003xx; TRICKY_JUMP_MIN)
            let min = std::env::var("TRICKY_JUMP_MIN").ok().and_then(|x| x.parse().ok()).unwrap_or(630.88f32);
            r.cpu.set_f(12, min);
            r.cpu.set_gpr(4, rider as u64);
            r.cpu.set_gpr(5, rider as u64);
            r.call(0x1284e0).unwrap_or_else(|e| panic!("tick {} rider {i}: {e}", w[0].tick));
            let imp: Vec<f32> = (0..3).map(|q| r.mem.read_f32(rider + 0x150 + 4 * q as u32) - f32_at(a, 0x150 + 4 * q)).collect();
            let game: Vec<f32> = (0..3).map(|q| f32_at(b, 0x150 + 4 * q) - f32_at(a, 0x150 + 4 * q)).collect();
            let rest: Vec<f32> = (0..3).map(|q| game[q] - imp[q]).collect();
            let size = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
            eprintln!("  tick {} rider {i}: game Δv {:.1} cm/s, impulse {:.1}, left over {rest:.2?} ({:.2} cm/s)", w[0].tick, size(&game), size(&imp), size(&rest));
            worst = worst.max(size(&rest));
            jumps += 1;
        }
    }
    eprintln!("{path}: {jumps} jumps; the game's Δv less the impulse is at most {worst:.2} cm/s");
}

/// The flight prediction (row F4h2): `AirPredict_AtTakeoff` 0x123b10 (`air::predict_at_takeoff`) and
/// `AirPredict_Tick` 0x123ec8 (`Predict::tick`) against the record at rider+0x57b0 of a race recorded with
/// `pine.py record <s> <file> 0x600 57b0:64`. A take-off's prediction is a record with +0x4c (time since take-off)
/// 0 that changed this tick. With no landing the record keeps its own inputs (+0x10 position, +0x20 velocity,
/// +0x30 normal) and is made again from them whole. With a landing it is made from the rider at the end of the
/// tick (+0x140, +0x150, +0x2a0). Without the course here, our ray answers with the game's landing (point +0x10,
/// normal +0x30, word +0x40) on the first of our segments passing within 0.05 cm of that point, so the time to the
/// top (+0x48) is bit for bit and the landing on our flight shows the steps are the game's. The landing time and
/// velocity use our ray's fraction, so they are only near. Then every tick in the air: +0x4c, +8 and +0x50 from
/// the tick before, in IEEE f32 (the port) and with PCSX2's adder (`pcsx2_add`). Findings: `checking.md`, F4h2.
///
/// A take-off in the motion (`Boarder_TakeOff` 0x128470) predicts before `Jump_ApplyImpulse` 0x1284e0 (row F4h3): its
/// record's velocity is the one before the jump. With no landing the jump is run on it (ours, and the game's in the
/// runner under IEEE and PCSX2's adder) against the rider's velocity at the end of the tick. With a landing whose
/// time to the top or landing misses from the end of the tick, the velocity before the jump is found (`unjump`, with
/// the game's jump under PCSX2's adder) and the flight made again from it. The jump's strength needs the stat blocks
/// (TRICKY_STATS) out of the jump state (0xa).
#[test]
fn flight_predictions_against_the_running_game() {
    let Ok(paths) = std::env::var("TRICKY_RECORDING") else { return };
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let (n, ticks) = load(path);
        let Some(k) = ticks[0].extra.iter().position(|(off, _)| *off == 0x57b0) else {
            eprintln!("{path}: no 57b0 window (record with `57b0:64`)");
            continue;
        };
        let (mut made, mut mid, mut top, mut tops, mut on_seg, mut off_seg) = (0, 0, 0, 0, 0, 0);
        let (mut no_hit, mut no_hit_exact, mut no_hit_rider) = (0, 0, 0);
        let (mut worst_line, mut worst_t, mut worst_v) = (0f32, 0f32, 0f32);
        let (mut steps, mut steps_exact, mut steps_guard, mut steps_two) = (0, 0, 0, 0);
        let (mut before_jump, mut steep_after, mut steep_exact) = (0, 0, 0);
        let mut next_sample = [0usize; 2];
        // recorded before F4b3g (no `5900:4` window): a sample was read as soon as the race tick moved, while the
        // riders were ticking, so a rider's block and its windows may straddle its tick
        let early = !ticks[0].extra.iter().any(|(off, _)| *off == 0x5900);
        // the game's `Jump_ApplyImpulse` in the function runner, IEEE adds and PCSX2's adder
        let mut predictors: Vec<Option<Predictor>> = ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"].into_iter().map(Predictor::new).collect();
        let mut jumper: Option<Vec<(&str, Jumper)>> = ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"]
            .into_iter().map(|rules| Jumper::new(rules).map(|j| (rules, j))).collect();
        // the stat blocks (TRICKY_STATS: `rider stats-address hex` lines), for the jump's strength
        let stats: Vec<Vec<u8>> = std::env::var("TRICKY_STATS").ok().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| t.lines()
            .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
            .collect()).unwrap_or_default();
        let u32_at = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        for (wi, w) in ticks.windows(2).enumerate() {
            if w[1].tick != w[0].tick + 1 {
                continue;
            }
            for i in 0..n {
                let (pa, pb) = (&w[0].extra[k].1[i], &w[1].extra[k].1[i]);
                let rb = &w[1].riders[i];
                let x4c = |p: &[u8]| f32_at(p, 0x4c);
                if x4c(pb) == 0.0 && pb[0x08..0x54] != pa[0x08..0x54] {
                    // a take-off this tick
                    let (gpoint, gnormal, gword) = (vec4(pb, 0x10), vec4(pb, 0x30), u32_at(pb, 0x40));
                    if gword != 0xffff_ffff && f32_at(pb, 0x44) == 0.0 {
                        mid += 1; // read while the game was inside the prediction: +8 counting up, no landing time yet
                        continue;
                    }
                    made += 1;
                    let ctx = format!("tick {} rider {i}", w[1].tick);
                    let (pos, vel, normal) = (vec4(rb, 0x140), vec4(rb, 0x150), vec4(rb, 0x2a0));
                    if gword == 0xffff_ffff {
                        // no landing: the record keeps the prediction's own inputs (+0x10 position, +0x20 velocity,
                        // +0x30 normal), so the whole record can be made again from them
                        no_hit += 1;
                        let q = predict_at_takeoff(gpoint, vec4(pb, 0x20), gnormal, &mut |_, _| (-1.0, RayHit::default()));
                        let mut ours = pb[..0x54].to_vec();
                        for (o, v) in [(8, q.t), (0x44, q.t_land), (0x48, q.t_top), (0x4c, q.x4c), (0x50, q.x50)] {
                            ours[o..o + 4].copy_from_slice(&v.to_le_bytes());
                        }
                        for (o, v) in [(0x10, q.point), (0x20, q.vel), (0x30, q.normal)] {
                            for (c, x) in v.iter().enumerate() {
                                ours[o + 4 * c..o + 4 * c + 4].copy_from_slice(&x.to_le_bytes());
                            }
                        }
                        ours[0x40..0x44].copy_from_slice(&q.word.to_le_bytes());
                        if ours[8..0x54] == pb[8..0x54] {
                            no_hit_exact += 1;
                        } else {
                            eprintln!("  {ctx}: no landing, and ours made from the record's own inputs differs");
                        }
                        if (gpoint, vec4(pb, 0x20), gnormal) == (pos, vel, normal) {
                            no_hit_rider += 1;
                        } else {
                            // made before a jump the tick did afterwards: `Boarder_TakeOff` 0x128470 predicts first,
                            // then `Jump_ApplyImpulse` 0x1284e0
                            let state = u32_at(&w[0].riders[i], 0x428);
                            let js = jump_state(rb, stats.get(i).map(|s| s[0x17]), state, vec4(pb, 0x20));
                            let after = js.map(|js| jump_impulse(0.0, &js));
                            if let Some((v, true)) = after {
                                steep_after += 1;
                                let game = jumper.as_mut().map(|j| j.iter_mut().map(|(_, r)| game_jump(r, rb, stats.get(i), vec4(pb, 0x20), state).map(f32::to_bits) == vel.map(f32::to_bits)).collect::<Vec<_>>());
                                steep_exact += (v.map(f32::to_bits) == vel.map(f32::to_bits)) as usize;
                                eprintln!("  {ctx}: no landing, made before a steep-lip jump: ours after it {v:?}, the game's {vel:?} \
                                    (the runner's IEEE, fadd=g1 equal: {game:?}; normal z {}, forward z {})", gnormal[2], vec4(rb, 0x320)[2]);
                            } else {
                                eprintln!("  {ctx}: no landing, made from velocity {:?}, the rider ends the tick with {vel:?} \
                                    (the jump after it: {after:?})", vec4(pb, 0x20));
                            }
                        }
                        continue;
                    }
                    // a landing: from the rider at the end of the tick, our flight; the ray answers with the game's
                    // landing on the first of our segments that passes within 0.05 cm of its point
                    let (mut p, mut hit_seg, mut best, mut nearest) = fly(pos, vel, normal, gpoint, gnormal, gword);
                    if p.t_top.to_bits() != f32_at(pb, 0x48).to_bits() || hit_seg.is_none() {
                        // made before a jump the tick did afterwards (`Boarder_TakeOff` 0x128470 predicts first, then
                        // `Jump_ApplyImpulse` 0x1284e0): the velocity before that jump, found from the one after it
                        let state = u32_at(&w[0].riders[i], 0x428);
                        let js = jump_state(rb, stats.get(i).map(|s| s[0x17]), state, vel);
                        if let Some(js) = js {
                            // the velocity before, found with the game's jump under PCSX2's adder (else ours)
                            let (before, exact) = match jumper.as_mut() {
                                Some(j) => unjump(&js, &mut |v| game_jump(&mut j[1].1, rb, stats.get(i), v, state)),
                                None => unjump(&js, &mut |v| jump_impulse(0.0, &JumpState { vel: v, ..js }).0),
                            };
                            let q = fly(pos, before, normal, gpoint, gnormal, gword);
                            eprintln!("  {ctx}: from the velocity before the tick's jump {before:?} ({} found exactly; \
                                state {state:x}, steep lip {}): time to the top {}, landing {}", if exact { "" } else { "not" },
                                jump_impulse(0.0, &JumpState { vel: before, ..js }).1,
                                q.0.t_top.to_bits() == f32_at(pb, 0x48).to_bits(),
                                if q.1.is_some() { "on our flight".to_string() } else { format!("{} cm off", q.3.0) });
                            if q.0.t_top.to_bits() == f32_at(pb, 0x48).to_bits() && q.1.is_some() {
                                before_jump += 1;
                                (p, hit_seg, best, nearest) = q;
                            }
                        }
                    }
                    // a sample of the early recorder whose window was read after the rider's next tick (it made
                    // the prediction there): the next sample's rider, else (the rider's block read one air tick later
                    // still) that rider one air step back (`unstep`); each from the velocity as it is, else from the
                    // one before that tick's jump (as above)
                    let next = ticks.get(wi + 2).filter(|t| early && t.tick == w[1].tick + 1).map(|t| &t.riders[i]);
                    let missed = |q: &(Predict, Option<usize>, f32, (f32, f32))| q.0.t_top.to_bits() != f32_at(pb, 0x48).to_bits() || q.1.is_none();
                    if let (Some(rc), true) = (next, p.t_top.to_bits() != f32_at(pb, 0x48).to_bits() || hit_seg.is_none()) {
                        let state = u32_at(rb, 0x428);
                        for stepped in [false, true] {
                            if stepped && u32_at(rc, 0x424) != 1 {
                                continue;
                            }
                            let (pos, vc) = if stepped { unstep(vec4(rc, 0x140), vec4(rc, 0x150), f32_at(rc, 0x12c) / 60.0) } else { (vec4(rc, 0x140), vec4(rc, 0x150)) };
                            let mut q = fly(pos, vc, vec4(rc, 0x2a0), gpoint, gnormal, gword);
                            if let (true, Some(js)) = (missed(&q), jump_state(rc, stats.get(i).map(|s| s[0x17]), state, vc)) {
                                let (before, _) = match jumper.as_mut() {
                                    Some(j) => unjump(&js, &mut |v| game_jump(&mut j[1].1, rc, stats.get(i), v, state)),
                                    None => unjump(&js, &mut |v| jump_impulse(0.0, &JumpState { vel: v, ..js }).0),
                                };
                                q = fly(pos, before, vec4(rc, 0x2a0), gpoint, gnormal, gword);
                            }
                            if !missed(&q) {
                                eprintln!("  {ctx}: made from the next sample's rider{} (motion {} → {}): the window was read after the rider's next tick", if stepped { " one air step back" } else { "" }, u32_at(rb, 0x424), u32_at(rc, 0x424));
                                next_sample[stepped as usize] += 1;
                                (p, hit_seg, best, nearest) = q;
                                break;
                            }
                        }
                    }
                    tops += 1;
                    if p.t_top.to_bits() == f32_at(pb, 0x48).to_bits() {
                        top += 1;
                    } else {
                        eprintln!("  {ctx}: time to the top ours {} game {} (vel z {}, states {:x} → {:x})", p.t_top, f32_at(pb, 0x48), vel[2], u32_at(&w[0].riders[i], 0x428), u32_at(rb, 0x428));
                    }
                    if hit_seg.is_some() {
                        on_seg += 1;
                        worst_line = worst_line.max(best);
                        worst_t = worst_t.max((p.t_land - f32_at(pb, 0x44)).abs());
                        let gv = vec4(pb, 0x20);
                        worst_v = worst_v.max((0..3).map(|q| (p.vel[q] - gv[q]).abs()).fold(0.0, f32::max));
                    } else {
                        off_seg += 1;
                        let game: Vec<(f32, bool, f32)> = predictors.iter_mut().flatten().map(|q| q.fly(pos, vel, normal, gpoint, gnormal, gword)).collect();
                        eprintln!("  {ctx}: the game's landing {gpoint:?} at {} s is on none of our segments \
                            (nearest {} cm off, at {} along); the runner's (IEEE, fadd=g1): {game:?}", f32_at(pb, 0x44), nearest.0, nearest.1);
                        if u32_at(rb, 0x424) == 5 {
                            eprintln!("    (the wipe-out motion's: pushed after it by the rider collisions, see `ground_ticks_against_the_running_game`)");
                        }
                    }
                } else if x4c(pb) != 0.0 && x4c(pa).to_bits() != x4c(pb).to_bits() {
                    // a tick in the air: `AirPredict_Tick`
                    steps += 1;
                    let mut q = Predict { t_land: f32_at(pa, 0x44), t_top: f32_at(pa, 0x48), x4c: x4c(pa), ..Default::default() };
                    q.tick();
                    let game = [x4c(pb), f32_at(pb, 8), f32_at(pb, 0x50)].map(f32::to_bits);
                    if [q.x4c, q.t, q.x50].map(f32::to_bits) == game {
                        steps_exact += 1;
                    }
                    // the same three operations with PCSX2's adder as these ticks show it (`pcsx2_add`)
                    let x = pcsx2_add(x4c(pa), 0.016666668);
                    let ours = [x, pcsx2_add(f32_at(pa, 0x44), -x), pcsx2_add(f32_at(pa, 0x48), -x)].map(f32::to_bits);
                    if ours == game {
                        steps_guard += 1;
                    } else if game[0] == pcsx2_add(pcsx2_add(x4c(pa), 0.016666668), 0.016666668).to_bits() {
                        steps_two += 1; // two ticks between the recorded records (the recorder fell a tick behind)
                    } else if std::env::var("TRICKY_REPLAY_RIDER").is_ok_and(|r| r == i.to_string()) {
                        eprintln!("  tick {} rider {i}: tick ours {:?} game {:?}", w[1].tick, ours.map(f32::from_bits), game.map(f32::from_bits));
                    }
                }
            }
        }
        eprintln!("{path}: {made} take-off predictions (+{mid} read half-made). {no_hit} found no landing: bit for bit \
            from their own inputs on {no_hit_exact}, those inputs the rider's at the end of the tick on {no_hit_rider}. \
            {tops} found a landing: from the rider at the end of the tick, time to the top bit for bit on {top}, the \
            game's landing on our flight on {on_seg} (at most {worst_line:.4} cm off the segment; landing time within \
            {worst_t:e} s, velocity within {worst_v:e} cm/s), on none of our segments on {off_seg}. AirPredict_Tick bit for bit on {steps_exact} of {steps} ticks in IEEE f32, \
            on {steps_guard} with PCSX2's adder (+{steps_two} two ticks apart). Made before a steep-lip jump with no landing: \
            {steep_after}, ours after the jump the game's on {steep_exact}; landings from the velocity before the tick's jump: {before_jump}; from the next sample's rider (early recorder): {}, one air step back from it: {}", next_sample[0], next_sample[1]);
    }
}

/// The rider one air step (`Air_IntegrateRK4` 0x12b340 over `dt`, no substeps, as `AirMotion_Update` 0x108378
/// steps) before (pos, vel): the velocity found by iterating the step (it does not depend on the position), the
/// position less the step's move. Not exact to the bit: good to the landing's 0.05 cm.
fn unstep(pos: [f32; 4], vel: [f32; 4], dt: f32) -> ([f32; 4], [f32; 4]) {
    let mut v0 = vel;
    for _ in 0..20 {
        let (mut p, mut v) = ([0.0; 4], v0);
        integrate_rk4(dt, &mut p, &mut v, false);
        for c in 0..3 {
            v0[c] += vel[c] - v[c];
        }
    }
    let (mut d, mut v) = ([0.0; 4], v0);
    integrate_rk4(dt, &mut d, &mut v, false);
    (std::array::from_fn(|c| if c < 3 { pos[c] - d[c] } else { pos[c] }), v0)
}

/// What `Jump_ApplyImpulse` 0x1284e0 read on a take-off tick, from the rider at the end of the tick with the velocity
/// `vel` it had before the jump: out of the jump state (0xa) `Boarder_TakeOff` 0x128470 first sets the crouch +0x1c8
/// to +0x208 (no strength: only a steep lip changes the velocity). `None` when the jump has strength and the stat
/// block is not there.
fn jump_state(rb: &[u8], jump_stat: Option<u8>, state: u32, vel: [f32; 4]) -> Option<JumpState> {
    let x208 = f32_at(rb, 0x208);
    let crouch = if state == 0xa { f32_at(rb, 0x1c8) } else { x208 };
    if crouch > x208 && jump_stat.is_none() {
        return None;
    }
    Some(JumpState {
        crouch,
        x208,
        vel,
        jump_stat: jump_stat.unwrap_or(0),
        motion: u32_le(rb, 0x424),
        up: vec4(rb, 0x1a0),
        normal: vec4(rb, 0x2a0),
        forward: vec4(rb, 0x320),
    })
}

/// The velocity before `jump_impulse(0, js)` that gives `js.vel` after it: Newton's steps in f64 on our jump, then
/// the f32 values a few ulps around for one that `exact` (a jump: ours, or the game's in the runner) gives it bit for
/// bit (true when found).
fn unjump(js: &JumpState, exact: &mut dyn FnMut([f32; 4]) -> [f32; 4]) -> ([f32; 4], bool) {
    let after = js.vel;
    let jump = |v: [f64; 3]| -> [f64; 3] {
        let (o, _) = jump_impulse(0.0, &JumpState { vel: [v[0] as f32, v[1] as f32, v[2] as f32, 0.0], ..*js });
        [o[0] as f64, o[1] as f64, o[2] as f64]
    };
    let mut v = [after[0] as f64, after[1] as f64, after[2] as f64];
    for _ in 0..40 {
        let f = jump(v);
        let r = [f[0] - after[0] as f64, f[1] - after[1] as f64, f[2] - after[2] as f64];
        let mut m = [[0f64; 3]; 3];
        for c in 0..3 {
            let mut u = v;
            u[c] += 1.0;
            let g = jump(u);
            for k in 0..3 {
                m[k][c] = g[k] - f[k];
            }
        }
        let det = |m: [[f64; 3]; 3]| {
            m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
        };
        let d = det(m);
        for c in 0..3 {
            let mut mc = m;
            for k in 0..3 {
                mc[k][c] = r[k];
            }
            v[c] -= det(mc) / d;
        }
    }
    let near = [v[0] as f32, v[1] as f32, v[2] as f32, 0.0];
    let step = |x: f32, k: i32| f32::from_bits((x.to_bits() as i32 + if x < 0.0 { -k } else { k }) as u32);
    for dx in -6..=6 {
        for dy in -6..=6 {
            for dz in -6..=6 {
                let c = [step(near[0], dx), step(near[1], dy), step(near[2], dz), 0.0];
                if exact(c).map(f32::to_bits) == after.map(f32::to_bits) {
                    return (c, true);
                }
            }
        }
    }
    (near, false)
}

/// Our flight from `pos`, `vel`, `normal` with the ray answering the game's landing (point, normal, word) on the first
/// of our segments passing within 0.05 cm of its point: the prediction, that segment, its distance, and the nearest
/// any segment came (distance, fraction along).
fn fly(pos: [f32; 4], vel: [f32; 4], normal: [f32; 4], gpoint: [f32; 4], gnormal: [f32; 4], gword: u32) -> (Predict, Option<usize>, f32, (f32, f32)) {
    let (mut best, mut hit_seg, mut seg, mut nearest) = (f32::MAX, None, 0, (f32::MAX, 0.0f32));
    let mut ray = |a: [f32; 4], b: [f32; 4]| {
        seg += 1;
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let q = [gpoint[0] - a[0], gpoint[1] - a[1], gpoint[2] - a[2]];
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let h = (q[0] * d[0] + q[1] * d[1] + q[2] * d[2]) / dd;
        let off = [q[0] - h * d[0], q[1] - h * d[1], q[2] - h * d[2]];
        let dist = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
        if hit_seg.is_none() && dist < nearest.0 {
            nearest = (dist, h);
        }
        if hit_seg.is_none() && (0.0..=1.0).contains(&h) && dist < 0.05 {
            best = dist;
            hit_seg = Some(seg);
            (h, RayHit { point: gpoint, normal: gnormal, word: gword })
        } else {
            (-1.0, RayHit::default())
        }
    };
    let p = predict_at_takeoff(pos, vel, normal, &mut ray);
    (p, hit_seg, best, nearest)
}

/// `AirPredict_AtTakeoff` 0x123b10 in the function runner (`AirPredict_Reset` 0x123ab8 and `Air_IntegrateRK4`
/// 0x12b340 run as the original), the world ray answered as in `fly`: whether the game's landing is on its flight,
/// and the nearest any segment came.
struct Predictor {
    r: r5900::Runner,
    rd: u32,
    oracle: std::rc::Rc<std::cell::RefCell<([f32; 4], [f32; 4], u32, Option<[f32; 4]>, bool, f32)>>,
}

impl Predictor {
    fn new(rules: &str) -> Option<Predictor> {
        let mut r = r5900::Runner::from_env()?;
        r.float_rules = r5900::ps2float::Rules::parse(rules).unwrap();
        r.allow(0x123ab8);
        r.allow(0x12b340);
        // (landing point, normal, word, the segment's start, hit yet, nearest)
        let oracle: std::rc::Rc<std::cell::RefCell<([f32; 4], [f32; 4], u32, Option<[f32; 4]>, bool, f32)>> = Default::default();
        let (o1, o2) = (oracle.clone(), oracle.clone());
        let get4 = |m: &mut r5900::Mem, at: u32| -> [f32; 4] { std::array::from_fn(|k| m.read_f32(at + 4 * k as u32)) };
        // the `cWorldLine` constructor: the segment's two ends
        r.stub(0x25d7f0, move |c, m| {
            let (a, b) = (get4(m, c.gpr(5) as u32), get4(m, c.gpr(6) as u32));
            let mut o = o1.borrow_mut();
            o.3 = Some(a);
            let g = o.0;
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let q = [g[0] - a[0], g[1] - a[1], g[2] - a[2]];
            let h = (q[0] * d[0] + q[1] * d[1] + q[2] * d[2]) / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
            let off = [q[0] - h * d[0], q[1] - h * d[1], q[2] - h * d[2]];
            let dist = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
            if !o.4 && dist < o.5 {
                o.5 = dist;
            }
            // the fraction kept in the start's w for the ray (a hit only within 0.05 cm, the first time)
            o.3 = if !o.4 && (0.0..=1.0).contains(&h) && dist < 0.05 { o.4 = true; Some([h, 0.0, 0.0, 0.0]) } else { None };
        });
        r.stub(0x25aff8, move |c, m| {
            let o = o2.borrow();
            let out = c.gpr(6) as u32;
            match o.3 {
                Some(h) => {
                    for k in 0..4 {
                        m.write_f32(out + 0x10 + 4 * k, o.0[k as usize]);
                        m.write_f32(out + 0x20 + 4 * k, o.1[k as usize]);
                    }
                    m.write_u32(out + 0x34, o.2);
                    c.set_f(0, h[0]);
                }
                None => c.set_f(0, -1.0),
            }
        });
        let rd = r.mem.alloc(0x6000);
        r.mem.write_u32(rd + 0x57b0 + 0x60, rd);
        Some(Predictor { r, rd, oracle })
    }

    /// (time to the top, landing on the flight, nearest)
    fn fly(&mut self, pos: [f32; 4], vel: [f32; 4], normal: [f32; 4], gpoint: [f32; 4], gnormal: [f32; 4], gword: u32) -> (f32, bool, f32) {
        *self.oracle.borrow_mut() = (gpoint, gnormal, gword, None, false, f32::MAX);
        for (o, v) in [(0x140, pos), (0x150, vel), (0x2a0, normal)] {
            for (k, x) in v.iter().enumerate() {
                self.r.mem.write_f32(self.rd + o + 4 * k as u32, *x);
            }
        }
        self.r.cpu.set_gpr(4, (self.rd + 0x57b0) as u64);
        self.r.call(0x123b10).unwrap();
        let o = self.oracle.borrow();
        (self.r.mem.read_f32(self.rd + 0x57b0 + 0x48), o.4, o.5)
    }
}

/// The function runner set up for `Jump_ApplyImpulse` 0x1284e0 (scoring and the meter stubbed).
struct Jumper {
    r: r5900::Runner,
    rd: u32,
    st: u32,
}

impl Jumper {
    fn new(rules: &str) -> Option<Jumper> {
        let mut r = r5900::Runner::from_env()?;
        r.float_rules = r5900::ps2float::Rules::parse(rules).unwrap();
        for f in [0x250e98, 0x250d60, 0x102f50] {
            r.allow(f); // Math_Cos, Math_Sin, Vec4_Scale
        }
        r.stub(0x156618, |c, _| c.set_f(0, 0.0));
        r.stub(0x11b018, |_, _| {});
        let rd = r.mem.alloc(0x6000);
        let st = r.mem.alloc(0x100);
        Some(Jumper { r, rd, st })
    }
}

/// The game's jump on the rider at the end of the tick `rb` with the velocity `vel` (as `jump_state`).
fn game_jump(j: &mut Jumper, rb: &[u8], stat: Option<&Vec<u8>>, vel: [f32; 4], state: u32) -> [f32; 4] {
    j.r.mem.write(j.rd, rb).unwrap();
    if let Some(s) = stat {
        j.r.mem.write(j.st, s).unwrap();
    }
    j.r.mem.write_u32(j.rd + 0x464, j.st);
    if state != 0xa {
        j.r.mem.write_f32(j.rd + 0x1c8, f32_at(rb, 0x208));
    }
    for (c, x) in vel.iter().enumerate() {
        j.r.mem.write_f32(j.rd + 0x150 + 4 * c as u32, *x);
    }
    j.r.cpu.set_f(12, 0.0);
    j.r.cpu.set_gpr(4, j.rd as u64);
    j.r.cpu.set_gpr(5, j.rd as u64);
    j.r.call(0x1284e0).unwrap();
    std::array::from_fn(|c| j.r.mem.read_f32(j.rd + 0x150 + 4 * c as u32))
}

/// `add.s` as PCSX2 runs it in normal play, read off the recorded `AirPredict_Tick` 0x123ec8 (row F4h2; F4g): the
/// smaller operand is cut toward zero to one bit below the larger's last mantissa bit (one guard bit, no sticky
/// bit), then the sum is rounded to nearest, ties to even. A test model only, not the port's float rule (F4c).
fn pcsx2_add(a: f32, b: f32) -> f32 {
    let (big, small) = if b.abs() > a.abs() { (b, a) } else { (a, b) };
    if small == 0.0 || big == 0.0 {
        return a + b;
    }
    let e = ((big.to_bits() >> 23) & 0xff) as i32 - 127; // the larger operand's exponent
    let guard = 2f64.powi(e - 24); // one bit below its last mantissa bit
    let cut = (small as f64 / guard).trunc() * guard;
    (big as f64 + cut) as f32 // exact in f64, rounded once to nearest
}


/// The animation's answers `SpinState_Update` asks for, from sample `t` (F4b3d): `AnimCtl_GetClip`(layer 0) from the
/// `46c0` window (+8), `AnimCtl_GetLayerPhase`(0) as `AnimClip_GetPhase` 0x159e80 on the layer's sequence of that
/// clip (`AnimLayer_FindClip`: the list's first node `4718>0` or its next `4718>30>0`, the list ending at the sentinel
/// rider + 0x4720; the clip's length the u16 `…>78>8>a`): t (+0x48) over the last frame (1 for a one-frame clip),
/// from the end when the speed (+0x10) is below 0; 1 with no sequence of the clip. The clip type and the trick clips'
/// answers stay 0.
fn spin_anim(t: &recording::Tick, i: usize, ka: usize, addr: u32) -> Anim {
    let clip = u32_le(&t.extra[ka].1[i], 8);
    let w = |name: &str| t.chased.iter().find(|(s, _)| s == name).map(|(_, v)| &v[i]);
    let seq = match (w("4718>0"), w("4718>30>0"), w("4718>78>8>a"), w("4718>30>78>8>a")) {
        (Some(s1), _, Some(l1), _) if u32_le(s1, 0x40) == clip => Some((s1, l1)),
        (Some(s1), Some(s2), _, Some(l2)) if u32_le(s1, 0x30) != addr + 0x4720 && u32_le(s2, 0x40) == clip => Some((s2, l2)),
        _ => None,
    };
    let phase = match seq {
        None => 1.0,
        Some((s, l)) => {
            let mut last = (u16::from_le_bytes([l[0], l[1]]) as i32 - 1) as f32;
            if last == 0.0 {
                last = 1.0;
            }
            let t = f32_at(s, 0x48);
            if f32_at(s, 0x10) < 0.0 {
                (last - t) / last
            } else {
                t / last
            }
        }
    };
    Anim { clip, phase, ..Default::default() }
}

/// The spin object (`SpinState_Update`'s `this`: the state manager *(rider+0x5ae0) + 0x740, which is rider − 0x170,
/// recorded as the window `-170:74`) as `spin::SpinObj`.
/// A computer's stick in `SpinState_Update` 0x100908's mode 2, read back from the second sample (F4b3d): the pad
/// bits 20-21 (spin) and 22-23 (flip), two-bit signed. With the wind-up on (st+0x5c) a non-zero stick steps +0x274
/// (spin) and +0x278 (flip) toward it, 0 leaves them, so the sign of the change is the stick; the flip's is
/// negated, and negated again when st+0x60 is set. With the wind-up off, a stick starts it (st+0x5c 0 → 1): any
/// direction does, spin right is used. Pad word 0 when nothing shows.
fn stick_from(a: &[u8], b: &[u8], sa: &[u8], sb: &[u8]) -> u32 {
    let field = |d: i32| (d as u32 & 3) as u32;
    if u32_le(sa, 0x5c) == 0 {
        return if u32_le(sb, 0x5c) == 1 { field(1) << 20 } else { 0 };
    }
    let dir = |o: usize| {
        let (x, y) = (f32_at(a, o), f32_at(b, o));
        if y > x { 1 } else if y < x { -1 } else { 0 }
    };
    let s = dir(0x274);
    // the flip: target = −(st+0x60 ? −raw : raw)
    let t = dir(0x278);
    let f = if u32_le(sa, 0x60) != 0 { t } else { -t };
    field(s) << 20 | field(f) << 22
}

/// A patch's points as PCSX2's patch program makes them (`Vu0_RunPatchProgram` 0x2576a8: every product and sum
/// toward zero, proven on the running game's patch cache, `tests/terrain.rs`, F4h5m)
fn patch_points_toward_zero(c: &[[f32; 4]; 16]) -> [[f32; 4]; 100] {
    let mul = |a: f32, b: f32| {
        let x = a as f64 * b as f64;
        let f = x as f32;
        if (f as f64).abs() > x.abs() { f32::from_bits(f.to_bits() - 1) } else { f }
    };
    tricky_game::terrain::tessellate_by(c, mul, |a, b| r5900::ps2float::add_by(a, b, r5900::ps2float::Round::Chop))
}

/// The rider's collision body (+0x470) as a sample holds it (as `cPostAI::vf1` 0x114730 posed it at the end of the
/// frame before)
fn body_of(r: &[u8]) -> tricky_game::collbody::CollBody {
    let mut b = tricky_game::collbody::CollBody::new();
    b.center = vec4(r, 0x480);
    b.radius = f32_at(r, 0x490);
    b.x24 = u32_le(r, 0x494) as i32;
    b.count = (u32_le(r, 0x498) as i32).clamp(0, 18);
    for k in 0..18 {
        b.spheres[k] = (vec4(r, 0x4a0 + 0x20 * k), f32_at(r, 0x4b0 + 0x20 * k));
    }
    b
}

fn spin_obj(st: &[u8]) -> SpinObj {
    SpinObj {
        mode: u32_le(st, 8),
        phase: f32_at(st, 0xc),
        owed: f32_at(st, 0x10),
        spin_hi: f32_at(st, 0x14),
        spin_lo: f32_at(st, 0x18),
        spin_floor: f32_at(st, 0x1c),
        flip_hi: f32_at(st, 0x20),
        flip_lo: f32_at(st, 0x24),
        flip_floor: f32_at(st, 0x28),
        w_spin: f32_at(st, 0x2c),
        w_flip: f32_at(st, 0x30),
        align: f32_at(st, 0x34),
        align_axis: [f32_at(st, 0x40), f32_at(st, 0x44), f32_at(st, 0x48)],
        align_w: f32_at(st, 0x4c),
        emax_spin: f32_at(st, 0x50),
        emax_flip: f32_at(st, 0x54),
        x58: u32_le(st, 0x58),
        x5c: u32_le(st, 0x5c),
        x60: u32_le(st, 0x60),
        x64: u32_le(st, 0x64),
    }
}

/// The air's spin state over a recorded race (board row F4b3a). For every pair of samples one tick apart in which a
/// rider is in the air (motion 1) and in control state 0x10 (spin) at both: `SpinState_Update` 0x100908 on the spin
/// object and the rider as recorded at the first, against the second. Run three ways: our `spin::spin_update` (then
/// `orient::update_orientation` when it asks for `Boarder_UpdateOrientation` 0x129008), and the game's own code in the
/// function runner under IEEE adds and under PCSX2's COP1 adder (F4g).
///
/// The pad word (built on the stack by the rider's virtual 9 in `cBoarder::vf7` 0x117198 and handed to
/// `Boarder_UpdateControl` 0x11c9a0, never stored) is the player's from the recorded pad (`player_pad`, F4b3d) and a
/// computer's 0, except in mode 2 where its stick is also tried as read back from the second sample (`stick_from`).
/// The animation's clip and phase (`AnimCtl_GetClip` 0x15fdd0, `AnimCtl_GetLayerPhase` 0x15fe70) come from the
/// `46c0` and sequence windows when recorded (TRICKY_NOANIM=1 leaves them 0); the other answers (type, trick clips)
/// are 0. So for the computers only what does not read the rest of the pad is compared: the turn toward the landing
/// slope (the quaternion +0x170, the axes +0x180..+0x1a0 and the angle left st+0x34) and the spin and flip angles
/// (+0x1d0, +0x1d4); the rates only in mode 2 at the end, where the stick is not read for them. The player's ticks
/// compare every field, but need its spin object (the `-140` window: its state manager is at rider − 0x880).
///
/// The recorder's sample can fall between two frames or inside one, between the rider's motion (`Boarder_UpdateMotion`
/// 0x11c770, whose `AirPredict_Tick` 0x123ec8 moves the time to touch down) and its control (0x11c9a0): the time to
/// touch down the control read is then the first sample's (+0x57b8) rather than the second's. Both are tried, the
/// second's first. The time scale (+0x12c) is always the second sample's: the controller virtual (`GetControls`
/// 0x151250 / 0x1384b8) moves it by `Boarder_SetTimeScaleRateLimited` 0x11cff0 between the motion and the control
/// (F4b3m). Pairs that hold no update of the rider or two (the position or the angles moved by 0 or 2 ticks'
/// worth) are left out. Needs TRICKY_STATS (the tricks stat, stats+0x1a) and a recording with the windows `57b0:64`
/// and `-170:74` (the spin object: the state manager *(rider+0x5ae0) + 0x740).
/// The pad word the rider's controller handed `Boarder_UpdateControl` for the tick that ends at sample `t`, kept by
/// `pine.py record ... hookpad` with the rider's speed-sample count (+0x5900) of that tick (F4b3k): None without the
/// window, or when its count is not the sample's (the rider did not tick in this frame)
fn hook_pad(t: &recording::Tick, i: usize) -> Option<u32> {
    let x = &t.chased.iter().find(|(s, _)| s == "hookpad")?.1[i];
    let count = t.extra.iter().find(|(o, _)| *o == 0x5900).map(|(_, v)| f32_at(&v[i], 0));
    (count.is_none() || count == Some(f32_at(x, 4))).then(|| u32_le(x, 0))
}

/// `AnimCtl_GetClipType` 0x15eec0 on clip `clip`: the clip table's (0x325fa8, 0x1c bytes a clip) first word, 8 for
/// no clip (0x300), read from the ELF (file offset = address − 0xff000)
fn clip_type_of(elf: &[u8], clip: u32) -> u32 {
    if clip == 0x300 {
        return 8;
    }
    let at = (0x325fa8 + clip as usize * 0x1c) - 0xff000;
    elf.get(at..at + 4).map_or(8, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The rider's animation controller from sample `t` for the replay of the frame's start (F4b3d2: the ended clips
/// flushed, then `SpinState_UpdateTrickClips` 0x1272e8), as memory regions: the rider window at `rd` with its stats
/// pointer (+0x464 = `stats`), the controller's first 0x10 bytes (window `46c0`: the layers' clips), its speed
/// +0x18 1 and rider +0x24, layer 0 with the recorded sequences (`4718>0`, `4718>30>0`, each with a tree and header holding its length
/// `…>78>8>a`, sources 1 and 2 none) at `nodes`, the stats `stat` at `stats`; past them at `nodes` + [`POOL`] the
/// pool new sequences take and at + [`NEW_TREE`] the one tree new sources get ([`AnimEnv`]). STANDIN: layers 1 and 2
/// empty and layer 0's sequences past the second left out (not recorded); they only take fades and stops, which the
/// answers do not read
fn trick_regions(t: &recording::Tick, i: usize, ka: usize, addr: u32, a: &[u8], rd: u32, nodes: u32, stats: u32, stat: &[u8]) -> Vec<(u32, Vec<u8>)> {
    use tricky_game::animctl as ac;
    use tricky_game::animseq as aseq;
    let mut m = tricky_game::pf::Mem { regions: vec![(rd, vec![0; 0x4900]), (nodes, vec![0; 0x800]), (stats, stat.to_vec())] };
    m.regions[0].1[..a.len()].copy_from_slice(a);
    m.w32(rd + 0x464, stats);
    m.regions[0].1[0x46c0..0x46d0].copy_from_slice(&t.extra[ka].1[i][..0x10]);
    m.w32(rd + 0x46c0 + 0x18, 1f32.to_bits());
    // the rider (+0x24) and +0x1c −1 as the controller is made; the stance (+0x14) and facing (+0x20) new clips take
    // are not recorded (0)
    m.w32(rd + 0x46c0 + 0x24, rd);
    m.w32(rd + 0x46c0 + 0x1c, u32::MAX);
    for k in 0..3 {
        aseq::list_init(&mut m, ac::layer(rd + 0x46c0, k));
    }
    let w = |name: &str| t.chased.iter().find(|(s, _)| s == name).map(|(_, v)| &v[i]);
    // a node is real when its next (+0x30) is not 0 (the list's end has 0 there)
    let mut seqs = Vec::new();
    if let (Some(s1), Some(l1)) = (w("4718>0"), w("4718>78>8>a")) {
        if u32_le(s1, 0x30) != 0 {
            seqs.push((s1, l1));
            if let (Some(s2), Some(l2)) = (w("4718>30>0"), w("4718>30>78>8>a")) {
                if u32_le(s1, 0x30) != addr + 0x4720 && u32_le(s2, 0x30) != 0 {
                    seqs.push((s2, l2));
                }
            }
        }
    }
    for (k, (s, l)) in seqs.iter().enumerate().rev() {
        let (n, tree) = (nodes + 0x90 * k as u32, nodes + 0x200 + 0x40 * k as u32);
        m.regions[1].1[(n - nodes) as usize..][..0x80].copy_from_slice(&s[..0x80]);
        m.w32(n + 0x78, tree);
        m.w32(n + 0x7c, 0);
        m.w32(tree + 8, tree + 0x20);
        m.w16(tree + 0x20 + 0xa, u16::from_le_bytes([l[0], l[1]]));
        aseq::list_insert(&mut m, ac::layer(rd + 0x46c0, 0), n);
    }
    // the new sources' tree: 30 frames, no markers, no children
    let t = nodes + NEW_TREE;
    m.w32(t + 8, t + 0x20);
    m.w16(t + 0x20 + 0xa, 30);
    for k in 0..8 {
        m.w16(t + 0x20 + 0xc + 2 * k, 0xffff);
    }
    m.regions
}

/// Where in the `nodes` block of [`trick_regions`] new sequences are taken from (0x90 bytes each) and the new tree
const POOL: u32 = 0x400;
const NEW_TREE: u32 = 0x700;

/// The answers of the animation's and the trick clips' calls out in the replay (F4b3d2), the same for ours and the
/// runner: the pool's next sequence, every clip an animation (1), every source the one tree; the sounds, the scoring,
/// deletes and the clip data nothing. `AnimCtl_PlayClip` as the trick clips call it out runs ours
/// ([`tricky_game::animctl::play_clip`]; the runner runs the original)
fn anim_answer(nodes: u32, next: &mut u32, func: u32) -> u32 {
    use tricky_game::animctl as ac;
    use tricky_game::animseq as aseq;
    match func {
        aseq::POOL_ALLOC => {
            *next += 1;
            nodes + POOL + 0x90 * (*next - 1)
        }
        ac::RESOLVE => 1,
        aseq::SKEL_ANIM_CREATE => nodes + NEW_TREE,
        _ => 0,
    }
}

struct AnimEnv {
    nodes: u32,
    next: u32,
}
impl tricky_game::animseq::SeqEnv for AnimEnv {
    fn call(&mut self, m: &mut tricky_game::pf::Mem, func: u32, args: &[u32]) -> u32 {
        if func == tricky_game::spin::PLAY_CLIP {
            return tricky_game::animctl::play_clip(m, self, args[0], args[1], args[2], args[3]);
        }
        anim_answer(self.nodes, &mut self.next, func)
    }
}

#[test]
fn spin_ticks_against_the_running_game() {
    let (Ok(paths), Ok(stats)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS")) else { return };
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let path = paths.split(';').next().unwrap();
    let (n, ticks) = load(path);
    let (Some(kp), Some(ks)) = (ticks[0].extra.iter().position(|(o, _)| *o == 0x57b0), ticks[0].extra.iter().position(|(o, _)| *o == -0x170)) else {
        eprintln!("{path}: no 57b0 and -170 windows: skipping");
        return;
    };
    let ks140 = ticks[0].extra.iter().position(|(o, _)| *o == -0x140);
    // the half-pipe turn's inputs (F4b3d2): the rider's place on the half-pipe (+0x4ab0, window `4aa0`) and the bone
    // it pivots on (window `8c0`: bone 0's x y z at +0x8c8, with its index +0x44d8, window `44d8`), when recorded
    let khp = ticks[0].extra.iter().position(|(o, _)| *o == 0x4aa0).filter(|_| std::env::var("TRICKY_NOHP").is_err());
    let kbone = ticks[0].extra.iter().position(|(o, _)| *o == 0x8c0);
    let kbi = ticks[0].extra.iter().position(|(o, _)| *o == 0x44d8);
    // the rider addresses (header `race X riders a b c ...`): the window is the spin object only where its +0x70 is the
    // rider (the state manager sits 0x8b0 before most riders, but 0x880 before one)
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners: Vec<(&str, r5900::Runner, [u32; 5])> = Vec::new();
    let elf = std::env::var("TRICKY_ELF").ok().and_then(|p| std::fs::read(p).ok());
    let anim_now = std::rc::Rc::new(std::cell::Cell::new(Anim::default()));
    let trick_on = std::rc::Rc::new(std::cell::Cell::new(false));
    let pool_next = std::rc::Rc::new(std::cell::Cell::new(0u32));
    // the animation's answers (F4b3d): the layer-0 clip (window `46c0`) and its phase from the sequence windows
    // (`4718>0`, `4718>30>0` and their lengths), when recorded
    let ka = ticks[0].extra.iter().position(|(o, _)| *o == 0x46c0).filter(|_| std::env::var("TRICKY_NOANIM").is_err());
    // the air motion before the control (F4b3d4): `AirMotion_Update` 0x108378's wall ray (`cWorldBoarder`(rider+0x470,
    // 0), the body's mask 0xcf) on the course (TRICKY_DISC; its patches toward zero, as PCSX2's patch program) from
    // the body the first sample holds; where it hits, the frame's air motion runs before the spin's update, the
    // original's in the runner and `air_motion::wall_push` / `wall_bump` in ours (the board's up from the frame,
    // window `4aa0`). TRICKY_NOAIRWALL=1: not cast, as before
    // The keyframed objects are posed as each frame saw them (F4h5l: `course_clock`, by PCSX2's adder)
    let kframe = ticks[0].extra.iter().position(|(o, _)| *o == 0x4aa0);
    let files = recording::course_files(path).filter(|_| kframe.is_some() && std::env::var("TRICKY_NOAIRWALL").is_err());
    let course = files.as_ref().and_then(|(p, l, s)| tricky_game::course_ground::CourseGround::new(p, l, s));
    let keyframed = course.as_ref().zip(files.as_ref()).and_then(|(c, (p, _, s))| {
        c.tessellate_with(patch_points_toward_zero);
        let pbd = tricky_data::pbd::Pbd::parse(p)?;
        let ssf = tricky_data::ssf::Ssf::parse(s, pbd.instances.len() as u32).ok()?;
        let samples: Vec<(u32, Vec<[f32; 4]>)> = ticks.iter().map(|t| (t.tick, (0..n).map(|i| vec4(&t.riders[i], 0x140)).collect())).collect();
        let clocks = c.with_world(|w| course_clock::clocks(&pbd, &ssf, w, &samples, |a, b| r5900::ps2float::add_by(a, b, r5900::ps2float::Round::Guard)));
        let posing = course_clock::Posing::new(p, &pbd, &[&clocks]);
        Some((clocks, posing))
    });
    let mut keyframed = keyframed;
    let wall_now = std::rc::Rc::new(std::cell::Cell::new(None::<tricky_game::air_motion::Ray>));
    let mut wall_ticks = [0usize];
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut r) = r5900::Runner::from_env() else { break };
        r.float_rules = r5900::ps2float::Rules::parse(rules).unwrap();
        for f in [0x250c10, 0x102e80, 0x102f80, 0x102f50, 0x129008, 0x2510f8] {
            r.allow(f); // Math_SinCos, Quat_Multiply, Vec3_Scale, Vec4_Scale, Boarder_UpdateOrientation, Math_Atan
        }
        // `SpinControl_CalcLandingAlign` and its maths, after the half-pipe turn (F4b3d2)
        for f in [0x11db00, 0x2508b8, 0x10c120, 0x250fd0, 0x250d60] {
            r.allow(f);
        }
        for f in [0x155e00, 0x155e68, 0x127918, 0x15eef8, 0x127d08, 0x118f10] {
            r.stub(f, |_, _| {});
        }
        // `SpinState_PlayExitClip` 0x102800 (F4b3d3): the original on the recorded controller (as 0x1272e8), else the
        // exit clip written as the controller's layer-0 clip (+8, which `AnimCtl_GetClip` reads); TRICKY_NOEXIT=1:
        // nothing played, as before
        let on = trick_on.clone();
        let no_exit = std::env::var("TRICKY_NOEXIT").is_ok();
        r.allow(0x102800);
        r.hook(0x102800, move |c, m| {
            if no_exit {
                return true;
            }
            if !on.get() {
                let rider = m.read_u32(c.gpr(4) as u32 + 0x70);
                if let Some(x) = tricky_game::spin::exit_clip(m.read_u32(rider + 0x46c8)) {
                    m.write_u32(rider + 0x46c8, x);
                }
            }
            !on.get()
        });
        // `SpinState_StartFromStick` 0x102158 and the state change it makes (`Boarder_SetState` 0x11f608: the control
        // states' exit 0x11c840 and enter 0x11c8e8, for 0x10 `SpinControl_Exit` 0x1029d0 and `SpinControl_Enter`
        // 0x100598) as the original (F4b3d2); the scoring (`Score_CommitAndRestartChain` 0x156568, `Boarder_AddMeter`
        // 0x11b018) answered 0. TRICKY_NOSTART=1: answered 0 as before
        if std::env::var("TRICKY_NOSTART").is_ok() {
            r.stub(0x102158, |c, _| c.set_gpr(2, 0));
        }
        for f in [0x102158, 0x11f608, 0x11c840, 0x11c8e8, 0x1029d0, 0x100598] {
            r.allow(f);
        }
        for f in [0x156568, 0x11b018] {
            r.stub(f, |c, _| c.set_gpr(2, 0));
        }
        // `SpinState_UpdateTrickClips` 0x1272e8 (F4b3d2): the original on the recorded controller where the tick's
        // trick clips are written (`trick_regions`), else the answer the tick is tried with; its reads of the
        // controller run as the original, its calls out as `TrickEnv`'s. `AnimCtl_GetClip` 0x15fdd0 and
        // `AnimCtl_GetClipType` 0x15eec0 read the clip the tick is tried with from the controller (+8)
        let (a, on) = (anim_now.clone(), trick_on.clone());
        r.hook(0x1272e8, move |c, _| {
            if !on.get() {
                c.set_gpr(2, a.get().trick_clips as u64);
            }
            !on.get()
        });
        for f in [
            0x1272e8, 0x203a40, 0x203ae0, 0x203b00, 0x203b48, 0x203b60, 0x203b98, 0x203bd8, 0x203c20, 0x203cb0, 0x203cf8,
            0x203d38, 0x159a80, 0x159d88, 0x159e38, 0x159e80, 0x159ff8, 0x15a030, 0x15a330, 0x15a3a8, 0x15a600, 0x15a6c8,
            0x15a700, 0x15a760, 0x15eec0, 0x15ef98, 0x15f050, 0x15fdd0, 0x15fec8, 0x15ffb8, 0x160008,
        ] {
            r.allow(f);
        }
        // the clip begun (`AnimCtl_PlayClip` 0x15fcf8 and what it calls) and the ended clips' flush
        // (`AnimCtl_FlushEndedClips` 0x15f700) as the original, their calls out answered as `anim_answer`
        for f in [0x2cc808, 0x159b08, 0x159b50, 0x159ba8, 0x159bb8, 0x159ee8, 0x15ed68, 0x15ee30, 0x15f020, 0x15f060, 0x15f5b8, 0x15f700, 0x15fcf8, 0x161360, 0x1613a0, 0x161400, 0x161528, 0x161568, 0x1615a8, 0x1615e8, 0x161640] {
            r.allow(f);
        }
        // the air motion (F4b3d4): `AirMotion_Update` 0x108378 and `Air_IntegrateRK4` 0x12b340 as the original; its
        // down ray (a `cWorldLine`, the rider's last-hit record +0x430 passed) answered as a miss (the pairs stay in
        // the air), its wall ray (none passed) with the course's hit; answered as nothing: the flight prediction
        // (`AirPredict_Tick` 0x123ec8), the shapes (`cWorldLine::cWorldLine` 0x25d7f0, `cWorldBoarder::cWorldBoarder`
        // 0x257810, `cWorldBoarder::vf10` 0x257ca0), `Rail_TryCapture` 0x125cb8, `Boarder_InstanceCollisions`
        // 0x125088, `Boarder_Translate` 0x1267a0 (the body is not compared) and a crash (`Boarder_Wipeout` 0x11d830,
        // `Score_Crash` 0x156de0)
        for f in [0x108378, 0x12b340] {
            r.allow(f);
        }
        for f in [0x123ec8, 0x25d7f0, 0x257810, 0x257ca0, 0x125cb8, 0x125088, 0x1267a0, 0x11d830, 0x156de0] {
            r.stub(f, |c, _| {
                c.set_gpr(2, 0);
                c.set_f(0, 0.0);
            });
        }
        let wn = wall_now.clone();
        r.hook(0x25aff8, move |c, m| {
            match wn.get().filter(|_| c.gpr(7) == 0) {
                Some(h) => {
                    let out = c.gpr(6) as u32;
                    for k in 0..4u32 {
                        m.write_f32(out + 0x10 + 4 * k, h.point[k as usize]);
                        m.write_f32(out + 0x20 + 4 * k, h.normal[k as usize]);
                    }
                    m.write_u32(out + 0x34, h.word);
                    c.set_f(0, h.frac);
                }
                None => c.set_f(0, -1.0),
            }
            true
        });
        let at = [r.mem.alloc(0x6400) + 0x200, r.mem.alloc(0x100), r.mem.alloc(4), r.mem.alloc(0x800), r.mem.alloc(0x40)];
        {
            use tricky_game::{animctl as ac, animseq as aseq};
            for f in [aseq::POOL_ALLOC, ac::RESOLVE, aseq::SKEL_ANIM_CREATE, aseq::DELETE, ac::GET_CLIP_ID, ac::GET_MARKER_FRAME, ac::SET_MARKER, ac::GET_MARKER_PHASE, ac::MARKER_SOUND, ac::CLIP_STARTED, ac::IDS_INIT, ac::BLEND_POSE] {
                let next = pool_next.clone();
                let nodes = at[3];
                r.stub(f, move |c, _| {
                    let mut n = next.get();
                    let v = anim_answer(nodes, &mut n, f);
                    next.set(n);
                    c.set_gpr(2, v as u64);
                });
            }
        }
        for f in [0x155ee8, 0x156148, 0x156050, 0x218e20, 0x218e28, 0x218b78] {
            r.stub(f, |_, _| {});
        }
        // `AnimCtl_GetLayerPhase` 0x15fe70: the original on the recorded controller (as 0x1272e8), else the phase the
        // tick is tried with
        let (a, on) = (anim_now.clone(), trick_on.clone());
        r.allow(0x15fe70);
        r.hook(0x15fe70, move |c, _| {
            if !on.get() {
                c.set_f(0, a.get().phase);
            }
            !on.get()
        });
        runners.push((rules, r, at));
    }
    // the trick clips' replay (F4b3d2): on the first runner's addresses (every runner allocates the same), with the
    // ELF's clip and trick tables; TRICKY_NOTRICK=1: answered 0
    let trick_at = runners.first().map(|(_, _, at)| (at[0], at[3], at[1])).filter(|_| ka.is_some() && std::env::var("TRICKY_NOTRICK").is_err());
    // ours reads the tables, vtables and the key table as the runner holds them
    let trick_consts: Vec<(u32, Vec<u8>)> = match runners.first_mut() {
        Some((_, r, _)) => {
            use tricky_game::animseq as aseq;
            let mut from = |at: u32, len: u32| {
                let mut b = vec![0u8; len as usize];
                r.mem.read(at, &mut b).unwrap();
                (at, b)
            };
            vec![from(0x325fa8, 0x1c * 0x300), from(tricky_game::spin::TRICK_TABLE, 12 * 0x2d0), from(tricky_game::animctl::AUDIO, 4), from(0, 0x100),
                from(aseq::SEQ_NAME, 0x38), from(aseq::NO_KEY, 0x40), from(aseq::KEY_TABLE, 4 * 0x584)]
        }
        None => Vec::new(),
    };
    let one_step = |a: &[u8], sa: &[u8], b: &[u8]| {
        let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
        let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
        let dt = f32_at(a, 0x12c) / 60.0;
        let turned = [(0x1d0, 0x2c), (0x1d4, 0x30)].iter().all(|&(o, w)| {
            let step = f32_at(sa, w) * dt;
            step.abs() < 1e-4 || ((f32_at(b, o) - f32_at(a, o)) / step - 1.0).abs() < 0.5
        });
        e > 1.0 && d > 0.7 * e && d < 1.4 * e && turned
    };
    // what is compared, as bits: quaternion, axes, spin, flip, angle left, and (mode 2) rates, bands, largest errors
    let names = ["quaternion", "axes", "spin", "flip", "angle left", "rates (mode 2)", "bands (mode 2)", "largest errors (mode 2)"];
    let pick = |st: &SpinObj, r: &[u8]| -> Vec<Vec<u32>> {
        let b = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
        vec![b(&vec4(r, 0x170)), b(&[vec4(r, 0x180), vec4(r, 0x190), vec4(r, 0x1a0)].concat()), b(&[f32_at(r, 0x1d0)]), b(&[f32_at(r, 0x1d4)]), b(&[st.align]),
            b(&[st.w_spin, st.w_flip]), b(&[st.spin_hi, st.spin_lo, st.spin_floor, st.flip_hi, st.flip_lo, st.flip_floor]), b(&[st.emax_spin, st.emax_flip])]
    };
    // per way: per field [ticks, bit for bit], and the ticks that took the first sample's time to touch down
    let mut count = vec![vec![[0usize; 2]; names.len()]; 1 + runners.len()];
    let mut first_land = vec![0usize; 1 + runners.len()];
    // the player's ticks (F4b3d): [compared, every field right], and those that took the first sample's pad
    let mut players = vec![[0usize; 2]; 1 + runners.len()];
    let mut first_pad = vec![0usize; 1 + runners.len()];
    let mut second_anim = vec![0usize; 1 + runners.len()];
    // the computer's stick read from the second sample (mode 2): [ticks it was tried], and per way those it made right
    let mut stick_tries = [0usize];
    let mut hooked_ticks = [0usize];
    let mut torn_ticks = [0usize];
    // mode-2 ticks on the half-pipe clips 0x23c / 0x23d (the phase moves the spin): [ticks], and per way right
    let mut pipe_ticks = [0usize];
    let mut pipe_right = vec![0usize; 1 + runners.len()];
    // the player's ticks by the spin's mode (0, 1, 2), and per way those every field right
    let mut player_modes = vec![[[0usize; 2]; 3]; 1 + runners.len()];
    let mut stick_right = vec![0usize; 1 + runners.len()];
    let n_ways = 1 + runners.len();
    let (mut other, mut skew) = (0, 0);
    for (wi, w) in ticks.windows(2).enumerate() {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        // the sample before, when it is the tick before (the first sample's pad then has its last tick's)
        let wp = (wi > 0 && ticks[wi - 1].tick + 1 == w[0].tick).then(|| &ticks[wi - 1]);
        // the keyframed objects as this frame's casts saw them (F4h5l)
        if let (Some(c), Some((clocks, posing))) = (&course, keyframed.as_mut()) {
            posing.pose(c, clocks.get(&(wi + 1)).map_or(&[][..], |l| &l[..]));
        }
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            let (pa, pb) = (&w[0].extra[kp].1[i], &w[1].extra[kp].1[i]);
            // the spin object: the `-170` window, or for the rider whose state manager is at rider − 0x880 (the player's
            // in rec14..rec19) the `-140` one when recorded
            let ks = match ks140 {
                Some(k) if u32_le(&w[0].extra[ks].1[i], 0x70) != addr[i] && u32_le(&w[0].extra[k].1[i], 0x70) == addr[i] => k,
                _ => ks,
            };
            let (sa, sb) = (&w[0].extra[ks].1[i], &w[1].extra[ks].1[i]);
            if std::env::var("TRICKY_SPDUMP").is_ok_and(|v| v.split(',').any(|t| t == w[1].tick.to_string())) {
                eprintln!("SPDUMP {} r{i} motion {} state {} spin {} flip {} ts {} | st mode {} ph {} owed {} w {} {} x58 {} x5c {} x60 {} x64 {} align {} emax {} {} | hook {:x?} clip {:x} n5900 {:?} hookn {:?} sw {} stance {} x138 {} ctl {:x?}",
                    w[1].tick, u32_le(b, 0x424), u32_le(b, 0x428), f32_at(b, 0x1d0), f32_at(b, 0x1d4), f32_at(b, 0x12c), u32_le(sb, 8), f32_at(sb, 0xc), f32_at(sb, 0x10),
                    f32_at(sb, 0x2c), f32_at(sb, 0x30), u32_le(sb, 0x58), u32_le(sb, 0x5c), u32_le(sb, 0x60), u32_le(sb, 0x64), f32_at(sb, 0x34), f32_at(sb, 0x50), f32_at(sb, 0x54),
                    hook_pad(&w[1], i), ka.map(|k| u32_le(&w[1].extra[k].1[i], 8)).unwrap_or(0),
                    w[1].extra.iter().find(|(o, _)| *o == 0x5900).map(|(_, v)| f32_at(&v[i], 0)), w[1].chased.iter().find(|(s, _)| s == "hookpad").map(|(_, v)| f32_at(&v[i], 4)),
                    u32_le(b, 0x1b4), u32_le(&stats[i], 0x40), f32_at(b, 0x138), ka.map(|k| (0..4).map(|j| u32_le(&w[1].extra[k].1[i], 4 * j)).collect::<Vec<_>>()));
            }
            if u32_le(a, 0x428) != 0x10 || u32_le(b, 0x428) != 0x10 || u32_le(a, 0x424) != 1 || u32_le(b, 0x424) != 1 {
                continue;
            }
            if u32_le(sa, 0x70) != addr[i] {
                other += 1;
                continue;
            }
            // (the spin object's rates; recordings of 0x700 bytes a rider no longer put it at +0x600)
            if !one_step(a, sa, b) {
                skew += 1;
                continue;
            }
            let stat = &stats[i];
            let mode = u32_le(sa, 8);
            let want = pick(&spin_obj(sb), b);
            if mode == 2 && std::env::var("TRICKY_HPDBG").is_ok() {
                let game_turn = u32_le(a, 0x1b4) != u32_le(b, 0x1b4);
                let nose = |t: &recording::Tick| khp.map(|k| vec4(&t.extra[k].1[i], 0x10));
                let (n0, n1) = (nose(&w[0]), nose(&w[1]));
                let test = |nose: [f32; 4]| {
                    let (face, axis, vel) = (vec4(pb, 0x20), vec4(pb, 0x30), vec4(a, 0x150));
                    let d4 = |x: [f32; 4], y: [f32; 4]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2] + x[3] * y[3];
                    let d = d4(nose, axis);
                    let out: Vec<f32> = (0..4).map(|k| nose[k] - axis[k] * d).collect();
                    let out = [out[0], out[1], out[2], out[3]];
                    let l = d4(out, out).sqrt();
                    let nn = [out[0] / l, out[1] / l, out[2] / l, out[3] / l];
                    (d4(nn, face) < 0.0, d4(nose, vel) < 277.777_77, f32_at(a, 0x1f0) == 0.0)
                };
                let (t0, t1) = (n0.map(test), n1.map(test));
                if game_turn || t0.is_some_and(|t| t.0 && t.1 && t.2) {
                    eprintln!("HPDBG tick {} rider {i} game_turn {game_turn} x5c {} clip {:x} w {} {} a(face,vel,1f0) {t0:?} b {t1:?} class {}", w[1].tick, u32_le(sa, 0x5c), u32_le(&w[0].extra[ka.unwrap()].1[i], 8), f32_at(sa, 0x2c), f32_at(sa, 0x30), u32_le(a, 0x420));
                }
            }
            let hp_win = khp.map(|k| &w[if std::env::var("TRICKY_HP").as_deref() == Ok("b") { 1 } else { 0 }].extra[k].1[i][..]);
            let hp_at = hp_win.map(|x| vec4(x, 0x10));
            // (bone 0 only: its index has been 0 on every sample, F4b3t)
            let (bone_win, bi_win) = (kbone.map(|k| &w[0].extra[k].1[i][..]), kbi.map(|k| &w[0].extra[k].1[i][..]));
            let bone = bone_win.zip(bi_win).filter(|(_, b)| u32_le(b, 0) == 0).map(|(x, _)| [f32_at(x, 8), f32_at(x, 0xc), f32_at(x, 0x10)]);
            // one way's fields for a time to touch down
            // a torn first sample (F4b3d3): the recorder reads every rider once all their speed-sample counts moved,
            // which can land inside the last rider's control update (slot 0, the player, ticks last). Seen as a late
            // trick half done: +0x58 set and the rates `SpinState_StartFromStick` 0x102158 sets for the sample's own
            // tick's pad word (window `hookpad`), with its clip not begun yet. TRICKY_NOTORN=1: taken as it is
            let torn_pad = hook_pad(&w[0], i).filter(|_| std::env::var("TRICKY_NOTORN").is_err() && u32_le(sa, 0x58) == 1).filter(|&p| {
                tricky_game::jump::spin_start_from_stick(p, stat[0x1a], u32_le(a, 0x1b4) != 0).is_some_and(|s| {
                    s.spin.to_bits() == f32_at(sa, 0x2c).to_bits() && s.flip.to_bits() == f32_at(sa, 0x30).to_bits()
                        && s.clip.is_some_and(|c| ka.is_some_and(|k| u32_le(&w[0].extra[k].1[i], 8) != c))
                })
            });
            torn_ticks[0] += torn_pad.is_some() as usize;
            // the air motion's wall ray (F4b3d4) on the body the first sample holds, and the frame's up (+0x4ac0)
            let frame_win = kframe.map(|k| &w[0].extra[k].1[i][..]);
            let air_wall = course.as_ref().zip(frame_win).and_then(|(c, fw)| {
                let mut body = body_of(a);
                body.x24 = 0xcf;
                let mut q = tricky_game::worldboarder::WorldBoarder::new(&body, 0);
                let (t, h) = c.ray_cast(&mut q, false, None, &|_| false);
                if 0.0 < t && std::env::var("TRICKY_WALLDBG").is_ok() {
                    eprintln!("WALLDBG tick {} rider {i}: t {t} at {:?} n {:?} object {:?}", w[1].tick, h.map(|h| h.pos), h.map(|h| h.normal), h.and_then(|h| h.object));
                }
                h.filter(|_| 0.0 < t).map(|h| (tricky_game::air_motion::Ray { frac: t, point: h.pos, normal: h.normal, word: h.surface as u32 }, vec4(fw, 0x20)))
            });
            wall_ticks[0] += air_wall.is_some() as usize;
            let ours = |land: f32, ts: f32, pad: u32, mut anim: Anim, regs: Option<&[(u32, Vec<u8>)]>| {
                // the trick clips first, on the recorded controller (F4b3d2)
                // a late trick from the stick (`SpinState_StartFromStick` 0x102158) where the update asks for one
                let start = tricky_game::jump::spin_start_from_stick(pad, stat[0x1a], u32_le(a, 0x1b4) != 0).filter(|_| std::env::var("TRICKY_NOSTART").is_err());
                anim.start_from_stick = start.is_some() as u32;
                let mut tmem = None;
                // a torn first sample (F4b3d3): the late trick's start finished first, its clip and type
                let torn_start = torn_pad.and_then(|p| tricky_game::jump::spin_start_from_stick(p, stat[0x1a], u32_le(a, 0x1b4) != 0));
                let mut torn_type = 0xb;
                if let (Some(rg), Some((trd, nodes, _))) = (regs, trick_at) {
                    use tricky_game::animctl as ac;
                    let mut m = tricky_game::pf::Mem { regions: rg.to_vec() };
                    m.regions.extend(trick_consts.iter().cloned());
                    let mut env = AnimEnv { nodes, next: 0 };
                    let ctl = trd + 0x46c0;
                    if let Some(s) = torn_start {
                        ac::blend_out_all(&mut m, ctl);
                        if let Some(c) = s.clip {
                            ac::play_clip(&mut m, &mut env, ctl, c, 0, 5);
                        }
                        torn_type = ac::clip_type(&m, ctl, 0);
                    }
                    // the frame's start: `cPreAI::vf1` 0x1146b0 flushes every rider's ended clips (`Boarder_FlushEndedClips`
                    // 0x117190) before the riders tick, and the sample is taken after the last frame's `cPostAI` advanced
                    // them; TRICKY_NOFLUSH=1: not
                    if std::env::var("TRICKY_NOFLUSH").is_err() {
                        ac::flush_ended(&mut m, &mut env, ctl);
                    }
                    (anim.clip, anim.clip_type, anim.phase) = (ac::clip(&m, ctl, 0), ac::clip_type(&m, ctl, 0), ac::layer_phase(&m, ctl, 0).0);
                    // the stick let go (F4b3d3): `SpinState_PlayExitClip` 0x102800 begins the exit clip before the trick
                    // clips run (`spin_update` reads the layer-0 clip it leaves again by `spin::exit_clip`)
                    if std::env::var("TRICKY_NOEXIT").is_err() && tricky_game::spin::spin_lets_go(pad, &spin_obj(sa)) {
                        tricky_game::spin::play_exit_clip(&mut m, &mut env, trd);
                    }
                    anim.trick_clips = tricky_game::spin::update_trick_clips(&mut m, &mut env, trd, (pad >> 9) & 1, pad & 0xff);
                    anim.trick_clip = Some((tricky_game::animctl::clip(&m, ctl, 0), tricky_game::animctl::clip_type(&m, ctl, 0)));
                    if std::env::var("TRICKY_TRDBG").is_ok_and(|v| v.split(',').any(|t| t == w[1].tick.to_string())) {
                        let s0 = tricky_game::animseq::layer_find(&m, tricky_game::animctl::layer(ctl, 0), anim.clip);
                        eprintln!("TRDBG tick {} rider {i} pad {pad:x}: 1e4 {:x} 1e8 {} 1ec {} 158 {} 24 {} clip {:x} type {:x} seq {s0:x} flags {:x} {:x} ended {} current {} action {} -> answer {} clip {:x?}; second sample clip {:x} 1e4 {:x} 1e8 {}",
                            w[1].tick, u32_le(a, 0x1e4), u32_le(a, 0x1e8), u32_le(a, 0x1ec), f32_at(a, 0x158), f32_at(a, 0x24), anim.clip, anim.clip_type,
                            if s0 == 0 { 0 } else { m.r8(s0 + 0x74) }, if s0 == 0 { 0 } else { m.r8(s0 + 0x75) }, if s0 == 0 { 0 } else { m.r32(s0 + 0x24) }, if s0 == 0 { 0 } else { m.r32(s0 + 0x68) }, m.r32(tricky_game::animctl::row(anim.clip) + 0xc), anim.trick_clips, anim.trick_clip,
                            u32_le(&w[1].extra[ka.unwrap()].1[i], 8), u32_le(b, 0x1e4), u32_le(b, 0x1e8));
                    }
                    tmem = Some((m, env, ctl));
                }
                let mut st = spin_obj(sa);
                let mut r = SpinRider {
                    timescale: ts,
                    land,
                    quat: vec4(a, 0x170),
                    spin: f32_at(a, 0x1d0),
                    flip: f32_at(a, 0x1d4),
                    tricks: stat[0x1a],
                    stat10: stat[0x10],
                    mirror: u32_le(stat, 0x40),
                    switch: u32_le(a, 0x1b4),
                    wind_spin: f32_at(a, 0x274),
                    wind_flip: f32_at(a, 0x278),
                    x1f0: f32_at(a, 0x1f0),
                    x1f4: f32_at(a, 0x1f4),
                    x1f8: f32_at(a, 0x1f8),
                    class: u32_le(a, 0x420),
                    hp_face: vec4(pb, 0x20),
                    hp_axis: vec4(pb, 0x30),
                    vel: vec4(a, 0x150),
                    pos: vec4(a, 0x140),
                    x5800: f32_at(pb, 0x50),
                    hp_at: hp_at.unwrap_or_default(),
                    bone: bone.unwrap_or_default(),
                    ..Default::default()
                };
                let mut axes = (vec4(a, 0x180), vec4(a, 0x190), vec4(a, 0x1a0));
                // the torn sample's start (F4b3d3): the rates and +0x58 (already in it), `SpinControl_Exit` and
                // `SpinControl_Enter` as for a late trick below
                if let Some(s) = torn_start {
                    (st.w_spin, st.w_flip, st.x58) = (s.spin, s.flip, 1);
                    if let Some(o2) = tricky_game::spin::spin_exit(&mut r) {
                        (r.quat, axes) = (o2.q, (o2.fwd, o2.side, o2.up));
                    }
                    let mut e = tricky_game::spin::SpinEnterRider {
                        x138: f32_at(a, 0x138),
                        switch: u32_le(a, 0x1b4),
                        stance: u32_le(stat, 0x40),
                        kind: u32_le(a, 0x1dc),
                        up_z: axes.2[2],
                        ..Default::default()
                    };
                    let ar = AlignRider { slope_kind: u32_le(pb, 0x40), slope: vec4(pb, 0x30), fwd: axes.0, quat: r.quat, class: u32_le(a, 0x420) };
                    tricky_game::spin::spin_enter(&mut st, &mut e, &ar, torn_type);
                }
                // the air motion's wall (F4b3d4), as `AirMotion_Update` 0x108378 with no landing: the flight step, the
                // push out of the wall, the glancing bump (the crash leaves the spin state: not replayed)
                if let Some((wall, up)) = air_wall {
                    integrate_rk4(ts * f32::from_bits(0x3c88_8889), &mut r.pos, &mut r.vel, false);
                    let push = tricky_game::air_motion::wall_push(&wall);
                    r.pos = [r.pos[0] + push[0], r.pos[1] + push[1], r.pos[2] + push[2], r.pos[3] + push[3]];
                    let mut o = Orient { q: r.quat, fwd: axes.0, side: axes.1, up: axes.2, yaw: f32_at(a, 0x1b0) };
                    if tricky_game::air_motion::wall_bump(&mut r.vel, &mut o, up, &wall).is_none() {
                        (r.quat, axes) = (o.q, (o.fwd, o.side, o.up));
                    }
                }
                let calls = spin_update(pad, &mut st, &mut r, &anim);
                let mut o = Orient { q: r.quat, fwd: axes.0, side: axes.1, up: axes.2, ..Default::default() };
                if calls.contains(&tricky_game::spin::SpinCall::Orientation) {
                    update_orientation(&mut o);
                }
                // the late trick (F4b3d2): `SpinState_StartFromStick` sets the rates and +0x58, fades layer 1 out and plays
                // the direction's clip, scores the start, and `Boarder_SetState` 1 then 0x10 runs `SpinControl_Exit`
                // 0x1029d0 and `SpinControl_Enter` 0x100598 (state 1 has neither); the clip's type for the enter from the
                // controller when replayed, else 0xb (STANDIN: the clips 0x226..0x22d are of type 0xb)
                if let (Some(s), true) = (start, calls.contains(&tricky_game::spin::SpinCall::StartFromStick)) {
                    (st.w_spin, st.w_flip, st.x58) = (s.spin, s.flip, 1);
                    let mut clip_type = 0xb;
                    if let Some((m, env, ctl)) = tmem.as_mut() {
                        tricky_game::animctl::blend_out_all(m, *ctl);
                        if let Some(c) = s.clip {
                            tricky_game::animctl::play_clip(m, env, *ctl, c, 0, 5);
                        }
                        clip_type = tricky_game::animctl::clip_type(m, *ctl, 0);
                    }
                    if let Some(o2) = tricky_game::spin::spin_exit(&mut r) {
                        o = o2;
                    }
                    let mut e = tricky_game::spin::SpinEnterRider {
                        x138: f32_at(a, 0x138),
                        switch: u32_le(a, 0x1b4),
                        stance: u32_le(stat, 0x40),
                        kind: u32_le(a, 0x1dc),
                        up_z: o.up[2],
                        ..Default::default()
                    };
                    let ar = AlignRider { slope_kind: u32_le(pb, 0x40), slope: vec4(pb, 0x30), fwd: o.fwd, quat: o.q, class: u32_le(a, 0x420) };
                    tricky_game::spin::spin_enter(&mut st, &mut e, &ar, clip_type);
                }
                // the half-pipe turn's last call (F4b3d2): `SpinControl_CalcLandingAlign` 0x11db00 on the turned rider
                if calls.contains(&tricky_game::spin::SpinCall::LandingAlign) {
                    let ar = AlignRider { slope_kind: u32_le(pb, 0x40), slope: vec4(pb, 0x30), fwd: o.fwd, quat: o.q, class: u32_le(a, 0x420) };
                    let mut axis = [st.align_axis[0], st.align_axis[1], st.align_axis[2], st.align_w];
                    landing_align(&ar, &mut axis, &mut st.align);
                    (st.align_axis, st.align_w) = ([axis[0], axis[1], axis[2]], axis[3]);
                }
                let mut out = a.clone();
                for (k, v) in [o.q, o.fwd, o.side, o.up].iter().enumerate() {
                    for (j, x) in v.iter().enumerate() {
                        out[0x170 + 0x10 * k + 4 * j..][..4].copy_from_slice(&x.to_le_bytes());
                    }
                }
                out[0x1d0..0x1d4].copy_from_slice(&r.spin.to_le_bytes());
                out[0x1d4..0x1d8].copy_from_slice(&r.flip.to_le_bytes());
                pick(&st, &out)
            };
            type Regs<'r> = Option<&'r [(u32, Vec<u8>)]>;
            let mut ways: Vec<Box<dyn FnMut(f32, f32, u32, Anim, Regs) -> Vec<Vec<u32>> + '_>> = vec![Box::new(ours)];
            for (_, g, [rd, stats_at, pad, _, air]) in runners.iter_mut() {
                let (rd, stats_at, pad, air) = (*rd, *stats_at, *pad, *air);
                let wn = wall_now.clone();
                let (an, on, pn) = (anim_now.clone(), trick_on.clone(), pool_next.clone());
                ways.push(Box::new(move |land: f32, ts: f32, pad_word: u32, anim: Anim, regs: Regs| {
                    an.set(anim);
                    on.set(regs.is_some());
                    pn.set(0);
                    for (at, b) in regs.unwrap_or_default() {
                        g.mem.write(*at, b).unwrap();
                    }
                    g.mem.write(rd, a).unwrap();
                    if regs.is_none() {
                        g.mem.write_u32(rd + 0x46c8, anim.clip);
                    }
                    g.mem.write_f32(rd + 0x12c, ts);
                    g.mem.write(rd + 0x57b0, pb).unwrap();
                    if let Some(hp) = hp_win {
                        g.mem.write(rd + 0x4aa0, hp).unwrap();
                    }
                    if let (Some(bw), Some(bi)) = (bone_win, bi_win) {
                        g.mem.write(rd + 0x8c0, bw).unwrap();
                        g.mem.write(rd + 0x44d8, bi).unwrap();
                    }
                    g.mem.write_f32(rd + 0x57b8, land);
                    g.mem.write(rd - 0x170, sa).unwrap();
                    g.mem.write_u32(rd - 0x170 + 0x70, rd);
                    // the state manager (the spin object's − 0x740) and its rider, for `Boarder_SetState`
                    g.mem.write_u32(rd + 0x5ae0, rd - 0x170 - 0x740);
                    g.mem.write_u32(rd - 0x170 - 0x740, rd);
                    g.mem.write(stats_at, stat).unwrap();
                    g.mem.write_u32(rd + 0x464, stats_at);
                    g.mem.write_u32(pad, pad_word);
                    // a torn first sample (F4b3d3): `SpinState_StartFromStick` 0x102158 run again on it with its own tick's
                    // pad word finishes the start (its writes before the read are the same again)
                    if let Some(p) = torn_pad {
                        g.mem.write_u32(pad, p);
                        g.cpu.set_gpr(4, (rd - 0x170) as u64);
                        g.cpu.set_gpr(5, pad as u64);
                        g.call(0x102158).unwrap_or_else(|e| panic!("tick {} rider {i}: torn: {e:?}", w[0].tick));
                        g.mem.write_u32(pad, pad_word);
                    }
                    if regs.is_some() && std::env::var("TRICKY_NOFLUSH").is_err() {
                        g.cpu.set_gpr(4, (rd + 0x46c0) as u64);
                        g.call(0x15f700).unwrap_or_else(|e| panic!("tick {} rider {i}: flush: {e:?}", w[0].tick));
                    }
                    // the air motion before the control (F4b3d4), on the frame the first sample holds
                    if let (Some((wall, _)), Some(fw)) = (air_wall, frame_win) {
                        g.mem.write(rd + 0x4aa0, fw).unwrap();
                        g.mem.write_u32(air + 0x20, rd);
                        wn.set(Some(wall));
                        g.cpu.set_gpr(4, air as u64);
                        g.call(0x108378).unwrap_or_else(|e| panic!("tick {} rider {i}: the air motion: {e:?}", w[0].tick));
                        wn.set(None);
                        if let Some(hp) = hp_win {
                            g.mem.write(rd + 0x4aa0, hp).unwrap();
                        }
                    }
                    g.cpu.set_gpr(4, (rd - 0x170) as u64);
                    g.cpu.set_gpr(5, pad as u64);
                    g.call(0x100908).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                    let (mut gs, mut gr) = (vec![0u8; 0x74], vec![0u8; 0x600]);
                    g.mem.read(rd - 0x170, &mut gs).unwrap();
                    g.mem.read(rd, &mut gr).unwrap();
                    pick(&spin_obj(&gs), &gr)
                }));
            }
            // the player's pad word (F4b3d): from the pad read at the second sample (the frame's), else the first's
            let p1 = w[1].pad.as_deref().and_then(|now| player_pad(stat, w[0].pad.as_deref(), now, 0x10));
            let p0 = wp.and_then(|x| player_pad(stat, x.pad.as_deref(), w[0].pad.as_deref()?, 0x10));
            let mut pads: Vec<u32> = if p1.is_some() { [p1, p0].into_iter().flatten().collect() } else { vec![0] };
            let player = w[1].pad.is_some() && u32_le(stat, 0x6c) <= 1;
            // the pad word the tick was handed, where the hook kept it (F4b3d1, window `hookpad`): the only one tried
            let hooked = hook_pad(&w[1], i).filter(|_| std::env::var("TRICKY_NOHOOK").is_err());
            if let Some(h) = hooked {
                pads = vec![h];
                hooked_ticks[0] += 1;
            }
            // a computer's pad word is not recordable (F4b3k), but in mode 2 its stick (pad bits 20-21 and 22-23,
            // `SpinState_Update` 0x100908 reads them with `<< 10 >> 30` and `<< 8 >> 30`) shows in the second sample:
            // it steps the wind-up +0x274 / +0x278 toward ±1 (0 moves nothing), or starts the wind-up (st+0x5c 0 → 1)
            let derived = (!player && mode == 2 && hooked.is_none()).then(|| stick_from(a, b, sa, sb)).filter(|&p| p != 0);
            pads.extend(derived);
            stick_tries[0] += derived.is_some() as usize;
            let derived_at = derived.map(|_| pads.len() - 1);
            let anims: Vec<Anim> = match ka {
                Some(ka) => [&w[0], &w[1]].iter().map(|t| spin_anim(t, i, ka, addr[i])).collect(),
                None => vec![Anim::default()],
            };
            // the clip's type (`AnimCtl_GetClipType` 0x15eec0: the clip table 0x325fa8, 0x1c bytes a clip, 8 for
            // none), and the trick clips' memory from the same sample (F4b3d2: `SpinState_UpdateTrickClips` 0x1272e8
            // replayed on it, ours and the original; without it answered 0)
            let anims: Vec<Anim> = anims.into_iter().map(|mut an| {
                if let Some(elf) = &elf {
                    an.clip_type = clip_type_of(elf, an.clip);
                }
                an
            }).collect();
            let regs: Vec<Option<Vec<(u32, Vec<u8>)>>> = [&w[0], &w[1]][..anims.len()].iter()
                .map(|t| trick_at.map(|(trd, nodes, stats_at)| trick_regions(t, i, ka.unwrap(), addr[i], a, trd, nodes, stats_at, stat)))
                .collect();
            // compared: the turn and angles always; the rates, bands and largest errors in mode 2, and for the
            // player in every mode
            let all = |f: usize| f < 5 || mode == 2 || player;
            let pipe = mode == 2 && anims.iter().any(|a| a.clip == 0x23c || a.clip == 0x23d);
            pipe_ticks[0] += pipe as usize;
            for (k, way) in ways.iter_mut().enumerate() {
                let mut got = Vec::new();
                'tries: for (pi, &pad) in pads.iter().enumerate() {
                    for (li, land) in [f32_at(pb, 8), f32_at(pa, 8)].into_iter().enumerate() {
                      for (ai, &an) in anims.iter().enumerate() {
                        let g = way(land, f32_at(b, 0x12c), pad, an, regs[ai].as_deref());
                        let right = (0..names.len()).filter(|&f| all(f)).all(|f| g[f] == want[f]);
                        if got.is_empty() || right {
                            got = g;
                        }
                        if right {
                            stick_right[k] += (Some(pi) == derived_at) as usize;
                            first_land[k] += li;
                            first_pad[k] += (pi == 1 && Some(pi) != derived_at) as usize;
                            second_anim[k] += ai;
                            break 'tries;
                        }
                      }
                    }
                }
                if k == n_ways - 1 && got[2] != want[2] && std::env::var("TRICKY_DBG").is_ok() {
                    eprintln!("DBG spin tick {} rider {i}: mode {mode} clip {:x}/{:x} phase {}/{} spin game {} got {} (from {}) rate {}", w[1].tick, anims[0].clip, anims[anims.len() - 1].clip, anims[0].phase, anims[anims.len() - 1].phase, f32::from_bits(want[2][0]), f32::from_bits(got[2][0]), f32_at(a, 0x1d0), f32_at(sa, 0x2c));
                }
                for (f, c) in count[k].iter_mut().enumerate() {
                    if all(f) {
                        c[0] += 1;
                        c[1] += (got[f] == want[f]) as usize;
                    }
                }
                if !player && k == n_ways - 1 && std::env::var("TRICKY_DBG").is_ok() && (0..names.len()).filter(|&f| all(f)).any(|f| got[f] != want[f]) {
                    let bad: Vec<&str> = (0..names.len()).filter(|&f| all(f) && got[f] != want[f]).map(|f| names[f]).collect();
                    eprintln!("DBG rider tick {} rider {i} mode {mode} pad {:08x}: {bad:?}", w[1].tick, pads[0]);
                    for f in (0..names.len()).filter(|&f| all(f) && got[f] != want[f]) {
                        let d: Vec<i64> = got[f].iter().zip(&want[f]).map(|(g, w)| *g as i32 as i64 - *w as i32 as i64).collect();
                        eprintln!("    {}: game {:?} before {:?} ulps off {d:?}", names[f], want[f].iter().map(|b| f32::from_bits(*b)).collect::<Vec<_>>(), if f == 0 { vec4(a, 0x170).to_vec() } else { vec![] });
                    }
                }
                pipe_right[k] += (pipe && (0..names.len()).filter(|&f| all(f)).all(|f| got[f] == want[f])) as usize;
                if player {
                    let right = (0..names.len()).all(|f| got[f] == want[f]);
                    players[k][0] += 1;
                    players[k][1] += right as usize;
                    if let Some(m) = player_modes[k].get_mut(mode as usize) {
                        m[0] += 1;
                        m[1] += right as usize;
                    }
                    if !right && (k == n_ways - 1 || std::env::var("TRICKY_DBGALL").is_ok()) && std::env::var("TRICKY_DBG").is_ok() {
                        let bad: Vec<&str> = (0..names.len()).filter(|&f| got[f] != want[f]).map(|f| names[f]).collect();
                        eprintln!("DBG player tick {} way {k} mode {mode} pad {:08x}: {bad:?}", w[1].tick, pads[0]);
                        for f in (0..names.len()).filter(|&f| got[f] != want[f]) {
                            let d: Vec<i64> = got[f].iter().zip(&want[f]).map(|(g, w)| *g as i32 as i64 - *w as i32 as i64).collect();
                            eprintln!("    {}: game {:?} ulps off {d:?}", names[f], want[f].iter().map(|b| f32::from_bits(*b)).collect::<Vec<_>>());
                        }
                    }
                }
            }
        }
    }
    eprintln!("{path}: spin ticks (state 0x10 in the air; left out: {other} whose window is not the spin object, {skew} not one update apart; the hook's pad word on {}; torn first samples finished on {}; the air motion's wall ray hit on {}):", hooked_ticks[0], torn_ticks[0], wall_ticks[0]);
    for (k, way) in ["ours (spin::spin_update)".to_string()].into_iter().chain(runners.iter().map(|(r, _, _)| format!("the game's in the runner, {r}"))).enumerate() {
        let line: Vec<String> = names.iter().zip(&count[k]).map(|(n, c)| format!("{n} {} of {}", c[1], c[0])).collect();
        let pm: Vec<String> = player_modes[k].iter().enumerate().map(|(m, c)| format!("mode {m} {} of {}", c[1], c[0])).collect();
        eprintln!("  {way}: {} (the first sample's time to touch down on {}); the player's ticks every field {} of {} ({}; the first sample's pad on {}); the second sample's animation on {}; a computer's stick from the second sample right on {} of {}; the half-pipe clips (mode 2) right on {} of {}", line.join(", "), first_land[k], players[k][1], players[k][0], pm.join(", "), first_pad[k], second_anim[k], stick_right[k], stick_tries[0], pipe_right[k], pipe_ticks[0]);
    }
}
