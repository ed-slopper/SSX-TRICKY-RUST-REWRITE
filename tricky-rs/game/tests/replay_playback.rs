//! The replay's playback control and the manager's life (board row G12g) against the game in the function runner
//! (TRICKY_ELF; skipped without it): random managers (keyframes, highlights, pins, playlists, camera tracks, pad
//! streams, the race's riders through gApp) built the same in the runner and in our memory, then random calls of
//! `Replay_Update`, `_DoCommand`, `_Step`, playback start / stop / abort, recording, status, clear and the pad
//! streams; every result, every world / HUD / camera call and all the memory compared after each. The manager's
//! set-up (`ReplayMgr_Init`, `Replay_Init`, `_FreeAll`) has its own test.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::{Error, Runner};
use tricky_game::replaymgr as rm;

mod replay_common;
use replay_common::*;

#[test]
fn replay_playback_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    allow_replay(&mut r);
    let w = Rc::new(RefCell::new(World::default()));
    stub_world(&mut r, &w);
    let a = addrs(&mut r);
    let mgr = a.mgr;
    let mut g = Rng(0x706c_6179_6261_0c01);
    let mut seen = [0u32; 16];
    let mut failed_both = 0;
    // coverage: modes updated, commands run, kinds of world call made
    let mut modes = [0u32; 16];
    let mut cmds = [0u32; 12];
    let mut kinds = std::collections::BTreeMap::<String, u32>::new();
    for trial in 0..1500 {
        let mut m = build(&mut g, &mut r, &a);
        for step in 0..12 {
            let op = g.below(16) as usize;
            let what = format!("trial {trial} step {step} op {op} mode {} cmd {}", m.r32(mgr + rm::MODE), m.r32(mgr + 0x28));
            let world = World {
                n: 0,
                menu: g.below(10) == 0,
                overflow: (g.below(6) == 0) as u32,
                alloc_at: a.inputs + IN_LEN * (4 + g.below(2)),
                log: vec![],
                ..Default::default()
            };
            let cmd = g.below(12);
            let jump = g.below(2) == 0;
            let frame = g.below(m.r32(mgr + rm::END) + 50);
            let flag = g.below(2);
            let pl = if g.below(2) == 0 { 0 } else { mgr + 0x38ec };
            let s = m.r32(mgr + rm::INPUTS + 4 * g.below(4));
            if op == 1 {
                m.w32(mgr + 0x28, cmd);
                r.mem.write_u32(mgr + 0x28, cmd);
            }
            if op == 4 && pl != 0 && m.r32(mgr + 0x38ec) == 0 {
                continue;
            }
            let mut ours_env = Ours(world.clone());
            let mut ours = m.clone();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<u32> {
                let (m, e) = (&mut ours, &mut ours_env);
                match op {
                    0 => {
                        rm::update(m, e, mgr);
                        None
                    }
                    1 => {
                        rm::do_command(m, e, mgr);
                        None
                    }
                    2 => {
                        rm::step(m, e, mgr, jump);
                        None
                    }
                    3 => {
                        rm::start_playback(m, e, mgr, pl);
                        None
                    }
                    4 => {
                        rm::set_playlist(m, e, mgr, pl);
                        None
                    }
                    5 => {
                        rm::stop_playback(m, e, mgr);
                        None
                    }
                    6 => {
                        rm::abort(m, e, mgr);
                        None
                    }
                    7 => {
                        rm::save_current_keyframe(m, e, mgr);
                        None
                    }
                    8 => {
                        rm::get_status(m, mgr, a.out);
                        None
                    }
                    9 => Some(rm::record_keyframe(m, e, mgr, frame, flag)),
                    10 => {
                        rm::clear(m, mgr);
                        None
                    }
                    11 => Some(rm::add_input_stream(m, e, mgr)),
                    12 => {
                        rm::remove_input_stream(m, e, mgr, s);
                        None
                    }
                    13 => {
                        rm::free_all(m, e, mgr);
                        None
                    }
                    14 => {
                        rm::mode_10_to_11(m, mgr);
                        None
                    }
                    _ => {
                        rm::freeze(m, mgr);
                        None
                    }
                }
            }));
            let (f, args): (u32, Vec<u32>) = match op {
                0 => (0x191298, vec![mgr]),
                1 => (0x191830, vec![mgr]),
                2 => (0x190878, vec![mgr, jump as u32]),
                3 => (0x191b60, vec![mgr, pl]),
                4 => (0x191b18, vec![mgr, pl]),
                5 => (0x191be0, vec![mgr]),
                6 => (0x193c98, vec![mgr]),
                7 => (0x193d30, vec![mgr]),
                8 => (0x191d88, vec![mgr, a.out]),
                9 => (0x18f328, vec![mgr, frame, flag]),
                10 => (0x18ebd0, vec![mgr]),
                11 => (0x191ca0, vec![mgr]),
                12 => (0x191d30, vec![mgr, s]),
                13 => (0x18f208, vec![mgr]),
                14 => (0x191278, vec![mgr]),
                _ => (0x191230, vec![mgr]),
            };
            *w.borrow_mut() = world;
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
            }
            let ret = r.call(f);
            match res {
                Err(_) => {
                    // our side stopped where the game traps or never finishes
                    assert!(matches!(ret, Err(Error::Trap { .. }) | Err(Error::TooLong { .. })), "{what}: the game went on: {ret:?}");
                    failed_both += 1;
                    break;
                }
                Ok(v) => {
                    ret.unwrap_or_else(|e| panic!("{what}: the game failed: {e:?}"));
                    if let Some(v) = v {
                        assert_eq!(r.cpu.gpr(2) as u32, v, "{what}: result");
                    }
                }
            }
            seen[op] += 1;
            if op == 0 {
                modes[m.r32(mgr + rm::MODE).min(15) as usize] += 1;
            }
            if op == 1 {
                cmds[cmd as usize] += 1;
            }
            for c in &ours_env.0.log {
                *kinds.entry(format!("{c:?}").split(['(', ' ']).next().unwrap().to_string()).or_default() += 1;
            }
            assert_eq!(w.borrow().log, ours_env.0.log, "{what}: calls");
            m = ours;
            compare(&mut r, &a, &m, &what);
        }
    }
    eprintln!("ops {seen:?}, {failed_both} stopped on both sides
modes {modes:?}
commands {cmds:?}
calls {kinds:?}");
}

#[test]
fn replay_manager_set_up_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    allow_replay(&mut r);
    let w = Rc::new(RefCell::new(World::default()));
    stub_world(&mut r, &w);
    let a = addrs(&mut r);
    let mgr = a.mgr;
    let mut g = Rng(0x7365_7475_7067_0001);
    let mut done = [0u32; 3];
    for case in 0..200 {
        let mut m = build(&mut g, &mut r, &a);
        let (units, two, secs) = ([0i32, 1, 2, 7, 40][g.below(5) as usize], g.below(3) as i64, [0i32, 60, 300, 7200][g.below(4) as usize]);
        let world = World { alloc_at: 0, ..Default::default() };
        let mut ours_env = Ours(world.clone());
        let mut ours = m.clone();
        let op = g.below(3);
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match op {
            0 => rm::mgr_init(&mut ours, &mut ours_env, mgr, units, two, secs),
            1 => rm::init(&mut ours, &mut ours_env, mgr),
            _ => rm::free_all(&mut ours, &mut ours_env, mgr),
        }));
        *w.borrow_mut() = world;
        let (f, args): (u32, Vec<u64>) = match op {
            0 => (0x18e850, vec![mgr as u64, units as i64 as u64, two as u64, secs as i64 as u64]),
            1 => (0x18eac8, vec![mgr as u64]),
            _ => (0x18f208, vec![mgr as u64]),
        };
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v);
        }
        let ret = r.call(f);
        let what = format!("case {case} op {op} ({units}, {two}, {secs})");
        if res.is_err() {
            assert!(matches!(ret, Err(Error::Trap { .. })), "{what}: the game went on: {ret:?}");
            continue;
        }
        ret.unwrap_or_else(|e| panic!("{what}: the game failed: {e:?}"));
        assert_eq!(w.borrow().log, ours_env.0.log, "{what}: calls");
        m = ours;
        compare(&mut r, &a, &m, &what);
        done[op as usize] += 1;
    }
    eprintln!("set-up done {done:?}");
    assert!(done.iter().all(|&d| d > 20));
}
