//! The trick tutorial's lesson manager against the original's (board row C11b3), in the function runner on the
//! player's SLUS_203.26 (TRICKY_ELF; skipped without it): `cLessonMan::vf1` 0x14b9b8 with the real handlers
//! (`cLesson*Handler`, 0x14dba0 .. 0x14ebf0) and helpers (`Lesson_HandleMenuInput`, `LessonMan_EndAttempt`,
//! `Lesson_SetupRider`, the menu setups) on a lesson object, with everything outside the manager (the pad, the HUD,
//! sounds, speech, the camera, the scene, the rider, the recording, the race object's virtual `set_mode`) stubbed
//! to log its call; `tricky_game::lesson::Lesson` on a scripted `Env` that logs the same calls. Random pad edges,
//! flags and external state requests; after every step the manager's fields, the rider's speed fields, the
//! recording position and the call log must agree.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::lesson::{btn, Env, Lesson, State, text};

const GAPP: u32 = 0x337c58;
const SETMODE: u32 = 0x1000;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn rb(m: &mut r5900::Mem, a: u32) -> u8 {
    (m.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

fn cstr(m: &mut r5900::Mem, mut a: u32) -> String {
    let mut s = String::new();
    loop {
        let b = rb(m, a);
        if b == 0 {
            return s;
        }
        s.push(b as char);
        a += 1;
    }
}

/// What the outside world answers this step (both sides read the same).
#[derive(Clone, Default)]
struct Script {
    blocked_flag: bool,
    menu_active: bool,
    dialog: bool,
    poll: i64,
    cam_running: bool,
    pressed: u32,
    rand: u32,
    text: bool,
    replay: bool,
    rec_len: u32,
    riders: [u32; 16],
}

#[derive(Default)]
struct Mock {
    s: Script,
    log: Vec<String>,
    speed: (f32, f32),
    done: bool,
    pressed_called: bool,
}

fn first_trick(page: i32) -> i32 {
    page * 5
}

impl Env for Mock {
    fn dialog_active(&self) -> bool { self.s.dialog }
    fn blocked(&self) -> bool { self.s.blocked_flag || self.s.menu_active }
    fn cam_running(&self) -> bool { self.s.cam_running }
    fn dialog_poll(&mut self) -> i64 { self.log.push("poll".into()); self.s.poll }
    fn dialog_set(&mut self, s: [u32; 3]) { self.log.push(format!("dset {} {} {}", s[0], s[1], s[2])) }
    fn pressed(&mut self) -> u32 { self.pressed_called = true; self.s.pressed }
    fn nav_sound(&mut self) { self.log.push("nav".into()) }
    fn back_sound(&mut self) { self.log.push("back".into()) }
    fn select_sound(&mut self) { self.log.push("select".into()) }
    fn page_first_trick(&self, page: i32) -> i32 { first_trick(page) }
    fn hud_show(&mut self, i: u32) { self.log.push(format!("show {i}")) }
    fn hud_hide(&mut self, i: u32) { self.log.push(format!("hide {i}")) }
    fn speech_clear(&mut self) { self.log.push("sclear".into()) }
    fn speech_line(&mut self, n: u32) { self.log.push(format!("sline {n}")) }
    fn cam_script(&mut self, name: &str) { self.log.push(format!("cam {name}")) }
    fn cam_cut_fade(&mut self) { self.log.push("cutfade".into()) }
    fn reset_scene(&mut self) { self.log.push("reset".into()) }
    fn set_mode(&mut self, m: u32) { self.log.push(format!("mode {m}")) }
    fn restart_rider(&mut self) { self.log.push("restart".into()) }
    fn rider_finish(&mut self) { self.log.push("f1117f0".into()); self.log.push("finish".into()) }
    fn set_rider_speed(&mut self, a: f32, b: f32) { self.speed = (a, b) }
    fn pad_map(&mut self, lesson: bool) { self.log.push(format!("pad {}", if lesson { "lesson" } else { "player" })) }
    fn load_recording(&mut self, rider: usize, trick: i32) -> u32 {
        self.log.push(format!("load R{:x}:{}", self.s.riders[rider], trick + 1));
        self.s.rec_len
    }
    fn replay_clear(&mut self) {
        // (`Replay_Clear` only when the game object has a replay, +0x40)
        if self.s.replay {
            self.log.push("replay_clear".into())
        }
    }
    fn rand_fx(&mut self) -> u32 { self.log.push("rand".into()); self.s.rand }
    fn trick_text_nonempty(&self) -> bool { self.s.text }
    fn set_done(&mut self) { self.done = true }
}

#[test]
fn lesson_manager_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // the manager, its handlers and the helpers that run for real
    for f in [
        0x14b9b8u32, 0x14bb08, 0x14baa8, 0x14b638, 0x14b768, 0x14d6e0, 0x14daa8, 0x14d948, 0x14d9c8, 0x14da38,
        0x14dba0, 0x14dba8, 0x14dc20, 0x14dc30, 0x14dc38, 0x14dc88, 0x14dcc0, 0x14e138, 0x14e270, 0x14e2b0, 0x14e2e0,
        0x14e388, 0x14e558, 0x14e688, 0x14e6a8, 0x14e6d0, 0x14e7c0, 0x14e830, 0x14e838, 0x14e900, 0x14ea20, 0x14eac8,
        0x14ebe8, 0x14ebf0, 0x14f348, 0x14f2f0,
    ] {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let sc: Rc<RefCell<Script>> = Rc::default();

    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x200);
    let race = r.mem.alloc(0x100);
    let vt = r.mem.alloc(0x100);
    let players = r.mem.alloc(0x2000);
    let rider = r.mem.alloc(0x100);
    let mapper = r.mem.alloc(0x10);
    let dev = r.mem.alloc(0x100);
    let l = r.mem.alloc(0x700);
    let rec = r.mem.alloc(0x4d598);
    let gobj = r.mem.alloc(0x6000);
    let hud_a = r.mem.alloc(0x400);
    let hud_b = r.mem.alloc(0x500);
    let hud_x = r.mem.alloc(0x20);
    let textbuf = r.mem.alloc(0x10);
    let cam = r.mem.alloc(0x10);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x14, 0x1111);
    r.mem.write_u32(game + 0x1c, race);
    r.mem.write_u32(game + 0x34, l);
    r.mem.write_u32(game + 0x38, 0x2222);
    r.mem.write_u32(game + 0xa0, cam);
    r.mem.write_u32(game + 0xa4, gobj);
    r.mem.write_u32(race + 8, vt);
    r.mem.write_u16(vt + 0x50, 0);
    r.mem.write_u32(vt + 0x54, SETMODE);
    r.mem.write_u32(race + 0x8c, players);
    r.mem.write_u32(players, rider);
    r.mem.write_u32(players + 0x870, mapper);
    r.mem.write_u32(mapper, dev);
    r.mem.write_u32(dev + 0x2c, 7);
    r.mem.write_u32(hud_b + 0x42c, hud_x);
    r.mem.write_u32(l + 0x46c, rec);
    // the handlers' objects: a slot holds its vtable
    for (slot, vtab) in [
        (0x18u32, 0x36e700u32), (0x1c, 0x36e6e0), (0x24, 0x36e6c0), (0x28, 0x36e6a0), (0x2c, 0x36e680), (0x30, 0x36e660),
        (0x34, 0x36e640), (0x38, 0x36e620), (0x3c, 0x36e600), (0x40, 0x36e5e0), (0x44, 0x36e5c0),
    ] {
        r.mem.write_u32(l + slot, vtab);
    }
    // the lesson's rider table pointers (the ELF's names)
    let tbl: Vec<u32> = (0..16).map(|i| r.mem.read_u32(0x331eb0 + i * 0x38)).collect();

    // ---- the outside, logged ----
    macro_rules! logs {
        ($addr:expr, $name:expr) => {{
            let lg = log.clone();
            r.stub($addr, move |_, _| lg.borrow_mut().push($name.to_string()));
        }};
    }
    {
        let lg = log.clone();
        r.stub(0x1a8648, move |_, _| lg.borrow_mut().push("nav".into()));
        let lg = log.clone();
        r.stub(0x1a8658, move |_, _| lg.borrow_mut().push("back".into()));
        let lg = log.clone();
        r.stub(0x1a8668, move |_, _| lg.borrow_mut().push("select".into()));
        let lg = log.clone();
        r.stub(0x1a7848, move |c, _| lg.borrow_mut().push(format!("show {}", c.gpr(5) as u32)));
        let lg = log.clone();
        r.stub(0x1a7860, move |c, _| lg.borrow_mut().push(format!("hide {}", c.gpr(5) as u32)));
        let lg = log.clone();
        r.stub(0x228d10, move |_, _| lg.borrow_mut().push("sclear".into()));
        let lg = log.clone();
        r.stub(0x231398, move |c, _| lg.borrow_mut().push(format!("sline {}", c.gpr(5) as u32)));
        let lg = log.clone();
        r.stub(0x175de0, move |c, m| {
            let s = cstr(m, c.gpr(5) as u32);
            lg.borrow_mut().push(format!("cam {s}"))
        });
    }
    logs!(0x174c48, "cutfade");
    logs!(0x14d7d0, "reset");
    logs!(0x14d898, "restart");
    logs!(0x11da40, "finish");
    logs!(0x1117f0, "f1117f0");
    logs!(0x18ebd0, "replay_clear");
    {
        let lg = log.clone();
        r.stub(SETMODE, move |c, _| lg.borrow_mut().push(format!("mode {}", c.gpr(5) as u32)));
        let (lg, s) = (log.clone(), sc.clone());
        r.stub(0x1aac08, move |c, _| {
            lg.borrow_mut().push("poll".into());
            c.set_gpr(2, s.borrow().poll as u64);
        });
        let lg = log.clone();
        r.stub(0x1aa090, move |c, _| lg.borrow_mut().push(format!("dset {} {} {}", c.gpr(5) as u32, c.gpr(7) as u32, c.gpr(9) as u32)));
        let s = sc.clone();
        r.stub(0x175e50, move |c, _| c.set_gpr(2, s.borrow().cam_running as u64));
        let (lg, s) = (log.clone(), sc.clone());
        r.stub(0x23d4a8, move |c, _| {
            lg.borrow_mut().push("rand".into());
            c.set_gpr(2, s.borrow().rand as u64);
        });
        let s = sc.clone();
        r.stub(0x1895c8, move |c, _| c.set_gpr(2, s.borrow().menu_active as u64));
        r.stub(0x1a7c58, move |c, _| c.set_gpr(2, hud_b as u64));
        r.stub(0x1a7c48, move |c, _| c.set_gpr(2, hud_a as u64));
        r.stub(0x164110, |c, _| {
            let p = c.gpr(4) as i32;
            c.set_gpr(2, first_trick(p) as i64 as u64);
        });
        r.stub(0x2c5160, |c, _| c.set_gpr(2, c.gpr(5)));
        r.stub(0x14ed88, |c, _| c.set_gpr(2, c.gpr(5)));
        r.stub(0x2cc808, |c, m| {
            let (d, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
            for k in 0..n {
                m.write_u8(d + k, v);
            }
        });
        // sprintf(dst, fmt, rider name, trick + 1): the path as "R<name pointer>:<n>", read back by the loader
        r.stub(0x2fca28, |c, m| {
            let s = format!("R{:x}:{}", c.gpr(6) as u32, c.gpr(7) as u32);
            let d = c.gpr(4) as u32;
            for (k, b) in s.bytes().chain(std::iter::once(0)).enumerate() {
                m.write_u8(d + k as u32, b);
            }
        });
        let (lg, s) = (log.clone(), sc.clone());
        r.stub(0x2c8bc8, move |c, m| {
            let p = cstr(m, c.gpr(4) as u32);
            lg.borrow_mut().push(format!("load {p}"));
            let buf = c.gpr(5) as u32;
            m.write_u32(buf + 4, s.borrow().rec_len);
        });
        let (lg, s) = (log.clone(), textbuf);
        let _ = (&lg, s);
        r.stub(0x157978, move |c, _| c.set_gpr(2, textbuf as u64));
        r.stub(0x2fd260, |c, m| {
            let mut a = c.gpr(4) as u32;
            let mut n = 0u64;
            while rb(m, a) != 0 {
                a += 1;
                n += 1;
            }
            c.set_gpr(2, n);
        });
        let lg = log.clone();
        r.stub(0x17a2f8, move |c, _| {
            let a = c.gpr(5) as u32;
            let name = if a == l + 0x2a4 { "lesson" } else if a == l + 0xdc { "player" } else { "?" };
            lg.borrow_mut().push(format!("pad {name}"));
        });
    }

    let mut g = Rng(0x7e55_01a4_c9d3_2b61);
    let mut seen = [0u32; 12];
    let mut env = Mock::default();
    for b in env.s.riders.iter_mut().zip(tbl.iter()) {
        *b.0 = *b.1;
    }
    let mut lesson = Lesson::new(0);
    // all zero, state requested = Init (LessonMan_Reset 0x14b5a8)
    r.mem.write_u32(l + 0x10, 1);
    let mut prev_state = 0u32;
    for step in 0..200_000u32 {
        // the world this step
        let pick = |g: &mut Rng, n: u32| g.next() % n;
        let edges = [0, 0, 0, btn::ACCEPT, btn::ACCEPT, btn::UP, btn::DOWN, btn::BACK, btn::MENU_UP, btn::MENU_DOWN, btn::ACCEPT | btn::UP, 0x1_0000];
        let s = Script {
            blocked_flag: pick(&mut g, 40) == 0,
            menu_active: pick(&mut g, 60) == 0,
            dialog: pick(&mut g, 12) == 0,
            poll: [0, 1, 1, 2][pick(&mut g, 4) as usize],
            cam_running: pick(&mut g, 3) == 0,
            pressed: edges[pick(&mut g, edges.len() as u32) as usize],
            rand: g.next(),
            text: pick(&mut g, 4) == 0,
            replay: pick(&mut g, 2) == 0,
            rec_len: 1 + pick(&mut g, 6),
            riders: env.s.riders,
        };
        env.s = s.clone();
        *sc.borrow_mut() = s.clone();
        r.mem.write_u32(hud_a + 0x328, s.dialog as u32);
        r.mem.write_u32(hud_x + 0xc, s.blocked_flag as u32);
        r.mem.write_u32(dev + 0xc, 0);
        r.mem.write_u32(dev + 0x10, s.pressed);
        r.mem.write_u32(dev + 0x2c, 7);
        r.mem.write_u8(textbuf, if s.text { b'x' } else { 0 });
        r.mem.write_u32(game + 0x40, s.replay as u32);
        r.mem.write_u32(game + 0x54, 0);
        env.done = false;
        env.pressed_called = false;
        log.borrow_mut().clear();
        env.log.clear();
        // the recording position moves under the manager (the player advances it)
        if pick(&mut g, 3) == 0 {
            let p = r.mem.read_u32(rec) + 1;
            r.mem.write_u32(rec, p);
            lesson.rec_pos = p;
        }
        // the page, the trick and the menu are set from outside rarely, to reach every branch
        match pick(&mut g, 400) {
            0 => {
                let n = pick(&mut g, 12);
                r.cpu.set_gpr(4, l as u64);
                r.cpu.set_gpr(5, n as u64);
                r.call(0x14baa8).unwrap_or_else(|e| panic!("step {step}: {e}"));
                lesson.set_state(&mut env, State::from_u32(n));
                // (a request for state 0 is a request for none)
            }
            _ => {}
        }
        let before_log_len = log.borrow().len();
        let _ = before_log_len;
        r.cpu.set_gpr(4, l as u64);
        r.call(0x14b9b8).unwrap_or_else(|e| panic!("step {step}: {e}"));
        lesson.tick(&mut env);
        // a retry (race state 8) is the race's side of the story: its handler puts the lesson in Intro
        if env.log.iter().any(|x| x == "mode 8") {
            r.cpu.set_gpr(4, l as u64);
            r.cpu.set_gpr(5, 2);
            r.call(0x14baa8).unwrap();
        }
        let theirs = log.borrow().clone();
        assert_eq!(theirs, env.log, "step {step}: calls (state {} -> {})", prev_state, r.mem.read_u32(l + 0xc));
        let w = |m: &mut r5900::Mem, o: u32| m.read_u32(l + o);
        assert_eq!(w(&mut r.mem, 0xc), lesson.state as u32, "step {step}: state");
        assert_eq!(w(&mut r.mem, 0x10), lesson.requested as u32, "step {step}: requested");
        assert_eq!(w(&mut r.mem, 0x14), lesson.prev as u32, "step {step}: prev");
        assert_eq!(w(&mut r.mem, 0x50), lesson.ticks, "step {step}: ticks");
        assert_eq!(w(&mut r.mem, 0x54) != 0, lesson.attempt_done, "step {step}: attempt done");
        assert_eq!(w(&mut r.mem, 0x58) as i32, lesson.choice, "step {step}: choice");
        assert_eq!(w(&mut r.mem, 0x5c) as i32, lesson.count, "step {step}: count");
        assert_eq!(w(&mut r.mem, 0x60) as i32, lesson.last_action, "step {step}: last action");
        assert_eq!(w(&mut r.mem, 0x64) != 0, lesson.trick_seen, "step {step}: trick seen");
        for k in 0..7 {
            assert_eq!(w(&mut r.mem, 0x68 + 4 * k), lesson.items[k as usize], "step {step}: item {k} (state {} -> {}, level {}, count {})", prev_state, lesson.state as u32, lesson.menu_level, lesson.count);
        }
        assert_eq!(w(&mut r.mem, 0xac), lesson.cam_pick, "step {step}: camera pick");
        assert_eq!(w(&mut r.mem, 0xb0) as i32, lesson.menu_level, "step {step}: menu level");
        assert_eq!(w(&mut r.mem, 0xb4) as i32, lesson.win_pos, "step {step}: window pos");
        assert_eq!(w(&mut r.mem, 0xb8) as i32, lesson.win_top, "step {step}: window top");
        assert_eq!(w(&mut r.mem, 0x4e8) as i32, lesson.trick, "step {step}: trick");
        assert_eq!(w(&mut r.mem, 0x50c) as i32, lesson.page, "step {step}: page");
        assert_eq!(f32::from_bits(w(&mut r.mem, 0x53c)), lesson.panel, "step {step}: panel");
        assert_eq!(r.mem.read_u32(rec), lesson.rec_pos, "step {step}: recording position");
        assert_eq!(r.mem.read_u32(rec + 4), lesson.rec_len, "step {step}: recording length");
        assert_eq!(f32::from_bits(r.mem.read_u32(rider + 0x1c)), env.speed.0, "step {step}: speed a");
        assert_eq!(f32::from_bits(r.mem.read_u32(rider + 0x24)), env.speed.1, "step {step}: speed b");
        assert_eq!(r.mem.read_u32(game + 0x54) != 0, env.done, "step {step}: done");
        if s.pressed != 0 {
            assert_eq!(r.mem.read_u32(dev + 0xc) != 0, env.pressed_called, "step {step}: pad read");
        }
        prev_state = lesson.state as u32;
        seen[lesson.state as usize] += 1;
        let _ = text::ACTIONS;
    }
    for st in 1..=11 {
        assert!(seen[st] > 100, "state {st} seen {} times", seen[st]);
    }
}
