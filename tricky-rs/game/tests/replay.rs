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
use tricky_game::spin::{spin_update, Anim, SpinObj, SpinRider};

mod recording;
use recording::{f32_at, load, u32_le, vec4};

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


/// The spin object (`SpinState_Update`'s `this`: the state manager *(rider+0x5ae0) + 0x740, which is rider − 0x170,
/// recorded as the window `-170:74`) as `spin::SpinObj`.
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
/// Not in the recording, so 0 for all three: the pad word (built on the stack by the rider's virtual 9 in `cBoarder::vf7`
/// 0x117198 and handed to `Boarder_UpdateControl` 0x11c9a0, never stored) and the animation's answers (clip, type,
/// phase, trick clips). So only what does not read them is compared: the turn toward the landing slope (the quaternion
/// +0x170, the axes +0x180..+0x1a0 and the angle left st+0x34) and the spin and flip angles (+0x1d0, +0x1d4); the
/// rates are compared only in mode 2 at the end, where the stick is not read for them.
///
/// The recorder's sample can fall between two frames or inside one, between the rider's motion (`Boarder_UpdateMotion`
/// 0x11c770, whose `AirPredict_Tick` 0x123ec8 moves the time to touch down) and its control (0x11c9a0): the time to
/// touch down the control read is then the first sample's (+0x57b8) rather than the second's. Both are tried, the
/// second's first. The time scale (+0x12c) is always the second sample's: the controller virtual (`GetControls`
/// 0x151250 / 0x1384b8) moves it by `Boarder_SetTimeScaleRateLimited` 0x11cff0 between the motion and the control
/// (F4b3m). Pairs that hold no update of the rider or two (the position or the angles moved by 0 or 2 ticks'
/// worth) are left out. Needs TRICKY_STATS (the tricks stat, stats+0x1a) and a recording with the windows `57b0:64`
/// and `-170:74` (the spin object: the state manager *(rider+0x5ae0) + 0x740).
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
    // the rider addresses (header `race X riders a b c ...`): the window is the spin object only where its +0x70 is the
    // rider (the state manager sits 0x8b0 before most riders, but 0x880 before one)
    let head = std::fs::read_to_string(path).unwrap().lines().next().unwrap().to_string();
    let addr: Vec<u32> = head.split_whitespace().skip(3).take(n).map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
    let mut runners: Vec<(&str, r5900::Runner, [u32; 3])> = Vec::new();
    for rules in ["add=n,mul=n,div=n,cvt=n", "add=n,fadd=g1,mul=n,div=n,cvt=n"] {
        let Some(mut r) = r5900::Runner::from_env() else { break };
        r.float_rules = r5900::ps2float::Rules::parse(rules).unwrap();
        for f in [0x250c10, 0x102e80, 0x102f80, 0x102f50, 0x129008, 0x2510f8] {
            r.allow(f); // Math_SinCos, Quat_Multiply, Vec3_Scale, Vec4_Scale, Boarder_UpdateOrientation, Math_Atan
        }
        for f in [0x155e00, 0x155e68, 0x102800, 0x127918, 0x15fcf8, 0x15eef8, 0x11db00, 0x127d08, 0x118f10] {
            r.stub(f, |_, _| {});
        }
        for f in [0x1272e8, 0x102158, 0x15fdd0, 0x15eec0] {
            r.stub(f, |c, _| c.set_gpr(2, 0));
        }
        r.stub(0x15fe70, |c, _| c.set_f(0, 0.0));
        let at = [r.mem.alloc(0x6400) + 0x200, r.mem.alloc(0x100), r.mem.alloc(4)];
        runners.push((rules, r, at));
    }
    let one_step = |a: &[u8], b: &[u8]| {
        let (p, q, v) = (vec4(a, 0x140), vec4(b, 0x140), vec4(a, 0x150));
        let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        let e = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * f32_at(a, 0x12c) / 60.0;
        let dt = f32_at(a, 0x12c) / 60.0;
        let turned = [(0x1d0, 0x2c), (0x1d4, 0x30)].iter().all(|&(o, w)| {
            let step = f32_at(&a[0x600..], w) * dt;
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
    let (mut other, mut skew) = (0, 0);
    for w in ticks.windows(2) {
        if w[1].tick != w[0].tick + 1 {
            continue;
        }
        for i in 0..n {
            let (a, b) = (&w[0].riders[i], &w[1].riders[i]);
            let (pa, pb) = (&w[0].extra[kp].1[i], &w[1].extra[kp].1[i]);
            let (sa, sb) = (&w[0].extra[ks].1[i], &w[1].extra[ks].1[i]);
            if u32_le(a, 0x428) != 0x10 || u32_le(b, 0x428) != 0x10 || u32_le(a, 0x424) != 1 || u32_le(b, 0x424) != 1 {
                continue;
            }
            if u32_le(sa, 0x70) != addr[i] {
                other += 1;
                continue;
            }
            let ab: Vec<u8> = [&a[..], &sa[..]].concat();
            if !one_step(&ab, b) {
                skew += 1;
                continue;
            }
            let stat = &stats[i];
            let mode = u32_le(sa, 8);
            let want = pick(&spin_obj(sb), b);
            // one way's fields for a time to touch down
            let ours = |land: f32, ts: f32| {
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
                    ..Default::default()
                };
                let calls = spin_update(0, &mut st, &mut r, &Anim::default());
                let mut o = Orient { q: r.quat, fwd: vec4(a, 0x180), side: vec4(a, 0x190), up: vec4(a, 0x1a0), ..Default::default() };
                if calls.contains(&tricky_game::spin::SpinCall::Orientation) {
                    update_orientation(&mut o);
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
            let mut ways: Vec<Box<dyn FnMut(f32, f32) -> Vec<Vec<u32>> + '_>> = vec![Box::new(ours)];
            for (_, g, [rd, stats_at, pad]) in runners.iter_mut() {
                let (rd, stats_at, pad) = (*rd, *stats_at, *pad);
                ways.push(Box::new(move |land: f32, ts: f32| {
                    g.mem.write(rd, a).unwrap();
                    g.mem.write_f32(rd + 0x12c, ts);
                    g.mem.write(rd + 0x57b0, pb).unwrap();
                    g.mem.write_f32(rd + 0x57b8, land);
                    g.mem.write(rd - 0x170, sa).unwrap();
                    g.mem.write_u32(rd - 0x170 + 0x70, rd);
                    g.mem.write(stats_at, stat).unwrap();
                    g.mem.write_u32(rd + 0x464, stats_at);
                    g.mem.write_u32(pad, 0);
                    g.cpu.set_gpr(4, (rd - 0x170) as u64);
                    g.cpu.set_gpr(5, pad as u64);
                    g.call(0x100908).unwrap_or_else(|e| panic!("tick {} rider {i}: {e:?}", w[0].tick));
                    let (mut gs, mut gr) = (vec![0u8; 0x74], vec![0u8; 0x600]);
                    g.mem.read(rd - 0x170, &mut gs).unwrap();
                    g.mem.read(rd, &mut gr).unwrap();
                    pick(&spin_obj(&gs), &gr)
                }));
            }
            for (k, way) in ways.iter_mut().enumerate() {
                let mut got = way(f32_at(pb, 8), f32_at(b, 0x12c));
                if got[..5] != want[..5] {
                    let other = way(f32_at(pa, 8), f32_at(b, 0x12c));
                    if other[..5] == want[..5] {
                        got = other;
                        first_land[k] += 1;
                    }
                }
                for (f, c) in count[k].iter_mut().enumerate() {
                    if f < 5 || mode == 2 {
                        c[0] += 1;
                        c[1] += (got[f] == want[f]) as usize;
                    }
                }
            }
        }
    }
    eprintln!("{path}: spin ticks (state 0x10 in the air; left out: {other} whose window is not the spin object, {skew} not one update apart):");
    for (k, way) in ["ours (spin::spin_update)".to_string()].into_iter().chain(runners.iter().map(|(r, _, _)| format!("the game's in the runner, {r}"))).enumerate() {
        let line: Vec<String> = names.iter().zip(&count[k]).map(|(n, c)| format!("{n} {} of {}", c[1], c[0])).collect();
        eprintln!("  {way}: {} (the first sample's time to touch down on {})", line.join(", "), first_land[k]);
    }
}
