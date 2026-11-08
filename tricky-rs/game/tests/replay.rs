//! Our functions against the running game, tick by tick (board row F4h): replays a race recorded over PINE
//! (`tools/pine.py record <seconds> <file>`, TRICKY_RECORDING; skipped without it) through our exact ports.
//!
//! The air step: each tick the game calls `Air_IntegrateRK4` 0x12b340 on the rider itself, with
//! dt = time scale (rider+0x12c) · 0.016666668, position rider+0x140, velocity rider+0x150, no sub-steps.
//! For every pair of consecutive recorded ticks we run ours from tick t and look for tick t+1's position and
//! velocity: a rider in the air gets exactly that (nothing else moves it in the air unless it hits something).

use tricky_game::air::{integrate_rk4, predict_at_takeoff, Predict, RayHit};

struct Tick {
    tick: u32,
    riders: Vec<Vec<u8>>,
    /// the windows `pine.py record ... <off>:<len>` adds (header `extra <off> <len>`): (offset, per rider)
    extra: Vec<(usize, Vec<Vec<u8>>)>,
}

fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn vec4(b: &[u8], off: usize) -> [f32; 4] {
    [f32_at(b, off), f32_at(b, off + 4), f32_at(b, off + 8), f32_at(b, off + 12)]
}

fn load(path: &str) -> (usize, Vec<Tick>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("header").split_whitespace().collect();
    // race X riders a b c ... bytes N [pad P]
    let at = head.iter().position(|w| *w == "bytes").unwrap();
    let (n, size) = (at - 3, usize::from_str_radix(head[at + 1], 16).unwrap());
    // the pad's block follows the riders when the recorder found it, then the extra windows, rider by rider
    let pad = head.iter().position(|w| *w == "pad").is_some_and(|k| head[k + 1] != "0") as usize;
    let windows: Vec<usize> = head.iter().enumerate().filter(|(_, w)| **w == "extra").map(|(k, _)| usize::from_str_radix(head[k + 1], 16).unwrap()).collect();
    let hex = |h: &str| (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
    let ticks = lines
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let riders = w[2..2 + n].iter().map(|h| hex(h)).collect();
            let extra = windows.iter().enumerate().map(|(k, off)| {
                let from = 2 + n + pad + k * n;
                (*off, w[from..from + n].iter().map(|h| hex(h)).collect())
            }).collect();
            Tick { tick: w[1].parse().unwrap(), riders, extra }
        })
        .collect::<Vec<_>>();
    assert!(ticks.iter().all(|t| t.riders.iter().all(|r| r.len() == size)));
    (n, ticks)
}

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
        let u32_at = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        for w in ticks.windows(2) {
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
                            eprintln!("  {ctx}: no landing, made from velocity {:?}, the rider ends the tick with {vel:?}", vec4(pb, 0x20));
                        }
                        continue;
                    }
                    // a landing: from the rider at the end of the tick, our flight; the ray answers with the game's
                    // landing on the first of our segments that passes within 0.05 cm of its point
                    let (mut best, mut hit_seg, mut seg) = (f32::MAX, None, 0);
                    let mut ray = |a: [f32; 4], b: [f32; 4]| {
                        seg += 1;
                        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                        let q = [gpoint[0] - a[0], gpoint[1] - a[1], gpoint[2] - a[2]];
                        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                        let h = (q[0] * d[0] + q[1] * d[1] + q[2] * d[2]) / dd;
                        let off = [q[0] - h * d[0], q[1] - h * d[1], q[2] - h * d[2]];
                        let dist = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
                        if hit_seg.is_none() && (0.0..=1.0).contains(&h) && dist < 0.05 {
                            best = dist;
                            hit_seg = Some(seg);
                            (h, RayHit { point: gpoint, normal: gnormal, word: gword })
                        } else {
                            (-1.0, RayHit::default())
                        }
                    };
                    let p = predict_at_takeoff(pos, vel, normal, &mut ray);
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
                        eprintln!("  {ctx}: the game's landing {gpoint:?} at {} s is on none of our segments", f32_at(pb, 0x44));
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
            on {steps_guard} with PCSX2's adder (+{steps_two} two ticks apart)");
    }
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
