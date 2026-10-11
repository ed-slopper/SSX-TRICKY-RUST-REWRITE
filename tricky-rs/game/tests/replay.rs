//! Our functions against the running game, tick by tick (board row F4h): replays a race recorded over PINE
//! (`tools/pine.py record <seconds> <file>`, TRICKY_RECORDING; skipped without it) through our exact ports.
//!
//! The air step: each tick the game calls `Air_IntegrateRK4` 0x12b340 on the rider itself, with
//! dt = time scale (rider+0x12c) · 0.016666668, position rider+0x140, velocity rider+0x150, no sub-steps.
//! For every pair of consecutive recorded ticks we run ours from tick t and look for tick t+1's position and
//! velocity: a rider in the air gets exactly that (nothing else moves it in the air unless it hits something).

use tricky_game::air::integrate_rk4;

struct Tick {
    tick: u32,
    riders: Vec<Vec<u8>>,
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
    let ticks = lines
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let riders = w[2..2 + n].iter().map(|h| (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect::<Vec<u8>>()).collect();
            Tick { tick: w[1].parse().unwrap(), riders }
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
