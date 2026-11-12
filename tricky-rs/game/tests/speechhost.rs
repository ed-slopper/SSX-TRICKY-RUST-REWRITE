//! `tricky_game::speechhost` on the disc (board row E1b2g7d3b2; TRICKY_DISC, skipped without it): the speech system
//! set up for a race loads the announcer's and the riders' banks from SPEECH.BIG, and the announcer's place-change
//! event (0x1002049, `Speech_OnPlaceChange` 0x233ba0) posted over its words is started on the announcer's stream as
//! parts of the MC banks.

use tricky_game::speechhost::SpeechHost;

fn host() -> Option<SpeechHost> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    let inf = files.load("data/config/speech.inf").ok().flatten().expect("SPEECH.INF");
    Some(SpeechHost::new(&inf, Box::new(move |p: &str| files.load(p).ok().flatten())))
}

#[test]
fn an_announcer_line_from_the_disc() {
    let Some(mut h) = host() else { return };
    assert_eq!(h.bank_count(), 115);
    let mut plays = Vec::new();
    for who in 0..3 {
        for new in 0..6i64 {
            for grade in 0..4 {
                let mut ev = [0u32; 20];
                ev[0] = 0x1002049;
                ev[1] = who;
                ev[2] = tricky_game::speech::place_mask(new as i32 + 1, new);
                ev[3] = grade;
                assert_eq!(h.post(&ev), 1);
                h.update();
                plays.extend(h.take_plays());
                h.frame(10.0);
            }
        }
    }
    assert!(!plays.is_empty());
    assert!(plays.iter().all(|p| p.stream == 0 && p.file.starts_with("|data\\speech\\mc\\") && p.file.ends_with(".dat")));
}

/// The same speech system run by the original (TRICKY_ELF too): the host's memory after its set-up copied into the
/// function runner, then each event posted (`SpeechLib_PostEvent` 0x2bba80) and the slots serviced
/// (`SpeechSys_UpdateAll` 0x228998) on both sides, the runner's calls to the host's stand-ins answered as the host
/// answered them; the calls (their arguments, pointers into the stacks aside), the answers and the memory must agree
mod against_the_original {
    use super::host;
    use r5900::Runner;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use tricky_game::speechhost::*;

    const POST: u32 = 0x2bba80;
    const UPDATE_ALL: u32 = 0x228998;
    const MEMSET: u32 = 0x2cc808;
    const SPRINTF: u32 = 0x2fca28;
    const FLOATS: [u32; 2] = [0x229578, 0x2296c0];

    fn on_stack(v: u32) -> bool {
        (0x0180_0000..STACK_TOP).contains(&v)
    }

    fn cstr(m: &mut r5900::Mem, at: u32) -> Vec<u8> {
        let mut s = Vec::new();
        let mut b = [0u8];
        while m.read(at + s.len() as u32, &mut b).is_some() && b[0] != 0 {
            s.push(b[0]);
        }
        s
    }

    type Log = Rc<RefCell<(VecDeque<LeafCall>, Vec<String>)>>;

    fn stub_leaves(r: &mut Runner, log: &Log) {
        for t in LEAVES.iter().chain(LEAVES_MORE.iter()).copied() {
            let log = log.clone();
            r.stub(t, move |c, mem| {
                let mut l = log.borrow_mut();
                let Some(e) = l.0.pop_front() else {
                    l.1.push(format!("the original calls {t:#x}, the host didn't"));
                    c.set_gpr(2, 0);
                    return;
                };
                let args: Vec<u32> =
                    (0..e.args.len()).map(|k| c.gpr(4 + k) as u32).collect();
                let same = e.target == t
                    && args.iter().zip(&e.args).all(|(a, b)| a == b || (on_stack(*a) && on_stack(*b)));
                if !same {
                    l.1.push(format!("the original calls {t:#x} {args:x?}, the host {:#x} {:x?}", e.target, e.args));
                }
                match t {
                    MEMSET => {
                        for k in 0..args[2] {
                            mem.write_u8(args[0] + k, args[1] as u8);
                        }
                    }
                    SPRINTF => {
                        // the host's formats: `%s` and `%d`
                        let f = cstr(mem, args[1]);
                        let mut s = Vec::new();
                        let (mut i, mut k) = (0, 2);
                        while i < f.len() {
                            if f[i] == b'%' {
                                match f[i + 1] {
                                    b's' => s.extend(cstr(mem, c.gpr(4 + k) as u32)),
                                    _ => s.extend((c.gpr(4 + k) as u32 as i32).to_string().bytes()),
                                }
                                k += 1;
                                i += 2;
                            } else {
                                s.push(f[i]);
                                i += 1;
                            }
                        }
                        s.push(0);
                        let _ = mem.write(args[0], &s);
                    }
                    _ => {}
                }
                if FLOATS.contains(&t) {
                    c.set_f(0, f32::from_bits(e.answer));
                } else {
                    c.set_gpr(2, e.answer as i32 as i64 as u64);
                }
            });
        }
    }

    /// The first bytes where the host's memory and the runner's differ, in the host's regions (its stack and the
    /// executable's strings aside)
    fn diffs(h: &SpeechHost, r: &mut Runner) -> Vec<String> {
        let mut out = Vec::new();
        for (at, bytes) in &h.mem.regions {
            if *at == STACK_BASE || *at == 0x39e000 {
                continue;
            }
            let mut theirs = vec![0u8; bytes.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            for (k, (a, b)) in bytes.iter().zip(&theirs).enumerate() {
                if a != b && out.len() < 16 {
                    out.push(format!("{:#x}: host {a:02x}, original {b:02x}", at + k as u32));
                }
            }
        }
        out
    }

    #[test]
    fn the_host_runs_as_the_original() {
        let Some(mut h) = host() else { return };
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for (at, bytes) in &h.mem.regions {
            if *at != STACK_BASE && *at != 0x39e000 {
                r.mem.write(*at, bytes).unwrap();
            }
        }
        for f in ALLOWED {
            r.allow(*f);
        }
        let log: Log = Rc::new(RefCell::new((VecDeque::new(), Vec::new())));
        stub_leaves(&mut r, &log);
        h.keep_log();
        let mut plays = Vec::new();
        let mut g = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = |n: u32| {
            g ^= g << 13;
            g ^= g >> 7;
            g ^= g << 17;
            ((g >> 16) as u32) % n
        };
        for step in 0..600 {
            let mut ev = [0u32; 20];
            if step < 72 {
                let (who, new, grade) = (step / 24, (step / 4 % 6) as i64, step % 4);
                ev[0] = 0x1002049;
                ev[1] = who;
                ev[2] = tricky_game::speech::place_mask(new as i32 + 1, new);
                ev[3] = grade;
            } else {
                ev[0] = IDS[rnd(IDS.len() as u32) as usize];
                for w in ev[1..1 + rnd(5) as usize].iter_mut() {
                    *w = if rnd(4) == 0 { 1 << rnd(12) } else { rnd(8) };
                }
            }
            let what = format!("step {step}, event {:x?}", &ev[..6]);
            let ours = h.post(&ev);
            let calls = h.take_log();
            log.borrow_mut().0.extend(calls);
            for (k, w) in ev.iter().enumerate() {
                r.mem.write_u32(EVENT + 4 * k as u32, *w);
            }
            r.cpu.set_gpr(4, EVENT as u64);
            r.call(POST).unwrap_or_else(|e| panic!("{what}: post: {e}"));
            assert_eq!(r.cpu.gpr(2) as u32, ours, "{what}: post's answer");
            h.update();
            let calls = h.take_log();
            log.borrow_mut().0.extend(calls);
            r.cpu.set_gpr(4, SYS_AT as u64);
            r.call(UPDATE_ALL).unwrap_or_else(|e| panic!("{what}: update: {e}"));
            {
                let l = log.borrow();
                assert!(l.1.is_empty(), "{what}: {:#?}", l.1);
                assert!(l.0.is_empty(), "{what}: the host calls more: {:x?}", l.0);
            }
            let d = diffs(&h, &mut r);
            assert!(d.is_empty(), "{what}: memory: {d:#?}");
            for p in h.take_plays() {
                h.line_started(p.handle, 200.0 + rnd(3000) as f32);
                plays.push((step, ev[0], p));
            }
            h.frame(0.1 + rnd(20) as f32 / 10.0);
        }
        // the run must have played lines, some of them in several parts (a name, then a place)
        assert!(plays.len() > 100, "{} lines played", plays.len());
        assert!(plays.windows(2).any(|w| w[0].0 == w[1].0), "no line in several parts");
    }

    /// The events `tricky_game::speech` posts
    const IDS: &[u32] = &[
        0x100200f, 0x1002010, 0x100201a, 0x100201c, 0x100201f, 0x1002020, 0x100202c, 0x100202e, 0x1002030, 0x1002031,
        0x1002033, 0x1002034, 0x1002036, 0x1002037, 0x1002046, 0x1002049, 0x100204a, 0x100204c, 0x100204d, 0x100204e,
        0x100204f, 0x1002050, 0x1002051, 0x1002052, 0x1002053, 0x1002054, 0x1002056, 0x300600f, 0x3006022, 0x3006025,
        0x3006026, 0x300602b, 0x3006036, 0x4008002, 0x4008003, 0x4008004, 0x4008005, 0x4008006, 0x4008007, 0x4008009,
        0x400800a, 0x400800b, 0x400800c, 0x400800d, 0x400800f, 0x4008010, 0x4008013, 0x4008015, 0x4008016, 0x4008017,
        0x500a000,
    ];

    /// The game's functions the original runs here (the rest are the host's stand-ins, stubbed)
    const ALLOWED: &[u32] = &[0x2bba50, 0x2bc9a0, 0x2bc9d8, 0x2bb270, 0x22a178, 0x2bccd0, 0x2bb2d8, 0x229fc8, 0x22a008, 0x227ca8, 0x228400, 0x228ae8, 0x227b00, 0x2bd928, 0x2bd798, 0x2be310, 0x2bdc98, 0x2bdb68, 0x2bd430, 0x2bc788, 0x2bd6a8, 0x2bb370, 0x2bca80, 0x2bc200, 0x2bc7a0, 0x2bea20, 0x2bea48, 0x2bcb30, 0x2bb180, 0x2bc850, 0x2be650, 0x2bcf90, 0x2bd078, 0x2bdad0, 0x2bcdb0, 0x2bcdd8, 0x2bb750, 0x2be468, 0x2bcf48, 0x2bde40, 0x2be140, 0x2bdae0, 0x2bc320, 0x2bc490, 0x2bbdd0, 0x2bbd68, 0x2bc048, 0x2bc5e0, 0x2bdc70, 0x229f60, 0x2282e0, 0x228370, 0x2bebb0, 0x2bb8d8, 0x2be880, 0x2be7d8, 0x2be678, 0x2be908, 0x2bb700, 0x2bb610, 0x2bb650, 0x2bb6b0, ];
}
