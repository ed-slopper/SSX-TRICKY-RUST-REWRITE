//! The race-flow glue (board row G4k1) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `cCountdownHandler::Enter` / `Update`, the race menu, `Race_UpdateLookBack`, `cAI::vf12` / `vf18` / `vf19`, on
//! random careers, clocks, replays and pads: every call, the clock, the latches and the handler's state compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE};
use tricky_game::prerace::{self, EndRace, GlueEnv, RaceView};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Script(String),
    Entered,
    State(u32),
    Vf94(u64),
    Tutorial,
    Hud(u32),
    Start(usize, bool),
    Go,
    Beep,
    Pressed(usize),
    Held(usize),
    Look(usize, bool),
    Abort,
    Freeze,
    CamStop,
    CamStart(String),
    Reset,
    ClearFlags,
    Prepare,
    Finish,
}

#[derive(Clone, Default)]
struct World {
    tutorial: bool,
    replay: Option<(i32, u32)>,
    pressed: Vec<u32>,
    held: Vec<bool>,
    state: i32,
    log: Vec<Ev>,
}

impl GlueEnv for World {
    fn run_world_script(&mut self, name: &str) {
        self.log.push(Ev::Script(name.into()));
    }
    fn race_entered(&mut self) {
        self.log.push(Ev::Entered);
    }
    fn set_state(&mut self, state: u32) {
        self.log.push(Ev::State(state));
    }
    fn race_vf94(&mut self, arg: u64) {
        self.log.push(Ev::Vf94(arg));
    }
    fn is_tutorial(&mut self) -> bool {
        self.log.push(Ev::Tutorial);
        self.tutorial
    }
    fn hud_show(&mut self, item: u32) {
        self.log.push(Ev::Hud(item));
    }
    fn set_start_state(&mut self, rider: usize, gate: bool) {
        self.log.push(Ev::Start(rider, gate));
    }
    fn replay(&mut self) -> Option<(i32, u32)> {
        self.replay
    }
    fn on_race_go(&mut self) {
        self.log.push(Ev::Go);
    }
    fn countdown_beep(&mut self) {
        self.log.push(Ev::Beep);
    }
    fn combo_pressed(&mut self, k: usize) -> u32 {
        self.log.push(Ev::Pressed(k));
        self.pressed[k]
    }
    fn combo_held(&mut self, k: usize) -> bool {
        self.log.push(Ev::Held(k));
        self.held[k]
    }
    fn set_look_back(&mut self, k: usize, on: bool) {
        self.log.push(Ev::Look(k, on));
    }
    fn replay_abort(&mut self) {
        self.log.push(Ev::Abort);
    }
    fn replay_freeze(&mut self) {
        self.log.push(Ev::Freeze);
    }
    fn cam_preload(&mut self) {
        self.log.push(Ev::CamStop);
        self.log.push(Ev::CamStart("PreLoad".into()));
    }
    fn reset_cameras(&mut self) {
        self.log.push(Ev::Reset);
    }
    fn clear_all_player_flags(&mut self) {
        self.log.push(Ev::ClearFlags);
    }
    fn prepare_race(&mut self) {
        self.log.push(Ev::Prepare);
    }
    fn finish_race(&mut self) {
        self.log.push(Ev::Finish);
    }
    fn show_results(&mut self) {}
    fn race_state(&mut self) -> i32 {
        self.state
    }
}

fn string_at(m: &mut r5900::Mem, p: u32) -> String {
    let mut s = String::new();
    for k in 0.. {
        let b = (m.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
        if b == 0 {
            break;
        }
        s.push(b as char);
    }
    s
}

const ENTERED: u32 = 0x01f0_0000;
const SET_STATE: u32 = 0x01f0_0010;
const VF94: u32 = 0x01f0_0020;

#[test]
fn race_glue_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let w = Rc::new(RefCell::new(World::default()));
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x100)).collect();
    let devices: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x40)).collect();
    macro_rules! s {
        ($addr:expr, |$c:ident, $m:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, $m| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    s!(0x13bad8, |c, m, x| x.log.push(Ev::Script(string_at(m, c.gpr(4) as u32))));
    s!(ENTERED, |_c, _m, x| x.log.push(Ev::Entered));
    s!(SET_STATE, |c, _m, x| x.log.push(Ev::State(c.gpr(5) as u32)));
    s!(VF94, |c, _m, x| x.log.push(Ev::Vf94(c.gpr(5))));
    s!(0x1148d0, |c, _m, x| {
        x.log.push(Ev::Tutorial);
        let v = x.tutorial;
        c.set_gpr(2, v as u64)
    });
    s!(0x1a7848, |c, _m, x| x.log.push(Ev::Hud(c.gpr(5) as u32)));
    let rs = riders.clone();
    s!(0x11a7d0, |c, _m, x| x.log.push(Ev::Start(rs.iter().position(|a| *a == c.gpr(4) as u32).unwrap(), c.gpr(5) != 0)));
    s!(0x215288, |_c, _m, x| x.log.push(Ev::Go));
    s!(0x216930, |_c, _m, x| x.log.push(Ev::Beep));
    let ds = devices.clone();
    s!(0x1797a8, |c, _m, x| {
        let k = ds.iter().position(|a| *a == c.gpr(4) as u32).unwrap();
        assert_eq!(c.gpr(5), 0x1f);
        x.log.push(Ev::Pressed(k));
        let v = x.pressed[k];
        c.set_gpr(2, v as u64)
    });
    let ds = devices.clone();
    s!(0x179758, |c, _m, x| {
        let k = ds.iter().position(|a| *a == c.gpr(4) as u32).unwrap();
        x.log.push(Ev::Held(k));
        let v = x.held[k];
        c.set_gpr(2, v as u64)
    });
    // Boarder_GetCamera: the boarder itself stands for its camera
    r.stub(0x11ca88, |c, _| {
        let v = c.gpr(4);
        c.set_gpr(2, v)
    });
    let rs = riders.clone();
    s!(0x1776f0, |c, _m, x| x.log.push(Ev::Look(rs.iter().position(|a| *a == c.gpr(4) as u32).unwrap(), c.gpr(5) != 0)));
    s!(0x193c98, |_c, _m, x| x.log.push(Ev::Abort));
    s!(0x191230, |_c, _m, x| x.log.push(Ev::Freeze));
    s!(0x175eb8, |_c, _m, x| x.log.push(Ev::CamStop));
    s!(0x175de0, |c, m, x| x.log.push(Ev::CamStart(string_at(m, c.gpr(5) as u32))));
    s!(0x180b58, |_c, _m, x| x.log.push(Ev::Reset));
    s!(0x161840, |_c, _m, x| x.log.push(Ev::ClearFlags));
    s!(0x163980, |_c, _m, x| x.log.push(Ev::Prepare));
    s!(0x163ab0, |_c, _m, x| x.log.push(Ev::Finish));
    r.allow(0x112848);
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x100);
    let racectl = r.mem.alloc(0x80);
    let gstate = r.mem.alloc(0x40);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, gstate);
    let hud = r.mem.alloc(0x10);
    r.mem.write_u32(game + 0x38, hud);
    let cam = r.mem.alloc(0x10);
    r.mem.write_u32(game + 0xa0, cam);
    let race = r.mem.alloc(0x200);
    let vt = r.mem.alloc(0x100);
    r.mem.write_u32(race + 8, vt);
    r.mem.write_u32(vt + 0x6c, ENTERED);
    r.mem.write_u32(vt + 0x54, SET_STATE);
    r.mem.write_u32(vt + 0x94, VF94);
    for (k, a) in riders.iter().enumerate() {
        r.mem.write_u32(race + 0xc4 + 4 * k as u32, *a);
    }
    let players: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x900)).collect();
    for k in 0..4 {
        r.mem.write_u32(race + 0x8c + 4 * k as u32, players[k]);
        r.mem.write_u32(players[k], riders[k]);
        r.mem.write_u32(players[k] + 0x21c * 4, devices[k]);
    }
    // the post-race handler sits at the race menu's +0x38: the forwards 0x113bb0..0x113be0 pass it on
    let end = r.mem.alloc(0x80) + 0x40;
    let mut g = Rng(0x61_0e_0000_0001);
    let mut seen = [0u32; 9];
    for case in 0..40_000 {
        let mut c = Career::default();
        c.w32(0x2afc, g.below(11));
        c.w32(0x2af8, g.below(12));
        c.w32(0x3260, [2u32, 4][g.below(2) as usize]);
        r.mem.write(CAREER_BASE, &c.bytes).unwrap();
        let view = RaceView { riders: 1 + g.below(6) as i32, players: 1 + g.below(4) as i32, ..Default::default() };
        r.mem.write_u32(race + 0x88, view.riders as u32);
        r.mem.write_u32(race + 0x7c, view.players as u32);
        let mut clock = [-300.0f32, -2.0, -60.0, -61.0, -120.0, -4.0, 10.0][g.below(7) as usize];
        r.mem.write_f32(race + 0x14, clock);
        let mut latches: Vec<u32> = (0..4).map(|_| g.below(2)).collect();
        for (k, v) in latches.iter().enumerate() {
            r.mem.write_u32(race + 0xe4 + 4 * k as u32, *v);
        }
        let replay = if g.below(2) == 0 { None } else { Some(([0i32, 1, 9, 10, -1][g.below(5) as usize], g.below(2))) };
        r.mem.write_u32(game + 0x40, if replay.is_some() { racectl } else { 0 });
        if let Some((n, s)) = replay {
            r.mem.write_u32(racectl + 0x20, n as u32);
            r.mem.write_u32(racectl + 0x4c, s);
        }
        let state = [4i32, 3][g.below(2) as usize];
        r.mem.write_u32(gstate + 0x1c, state as u32);
        let mut ours = World {
            tutorial: g.below(3) == 0,
            replay,
            pressed: (0..4).map(|_| g.below(2)).collect(),
            held: (0..4).map(|_| g.below(2) == 0).collect(),
            state,
            log: vec![],
        };
        *w.borrow_mut() = ours.clone();
        let mut handler = EndRace { state: g.below(16), ..Default::default() };
        r.mem.write_u32(end + 4, handler.state);
        let op = g.below(9) as usize;
        seen[op] += 1;
        let arg = g.next() as u64;
        let call = |r: &mut Runner, f: u32, args: &[u64]| {
            for (k, a) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + k, *a);
            }
            r.call(f).unwrap();
            r.cpu.gpr(2)
        };
        let fwd = g.below(2) == 0;
        let menu = |f: u32, ff: u32| if fwd { (ff, end - 0x38) } else { (f, end) };
        let mut ret = None;
        match op {
            0 => {
                call(&mut r, 0x113520, &[0, race as u64]);
                prerace::countdown_enter(&mut ours, &c, &view, &mut clock);
            }
            1 => {
                call(&mut r, 0x113600, &[0, race as u64]);
                prerace::countdown_update(&mut ours, &c, &mut clock);
            }
            2 => {
                call(&mut r, 0x113a88, &[race as u64]);
                prerace::update_look_back(&mut ours, view.players as usize, &mut latches);
            }
            3 => {
                call(&mut r, 0x112550, &[]);
                ours.run_world_script(prerace::start_script(&c));
            }
            4 => {
                let obj = race;
                call(&mut r, 0x111708, &[obj as u64]);
                prerace::race_to_state_2(&mut ours);
            }
            5 => {
                call(&mut r, 0x111738, &[race as u64, arg]);
                prerace::race_freeze(&mut ours, arg);
            }
            6 => {
                let st = [0xbu32, 0xc][g.below(2) as usize];
                let (f, a) = if st == 0xb { menu(0x113bf0, 0x113bc0) } else { menu(0x113ce8, 0x113be0) };
                call(&mut r, f, &[a as u64]);
                prerace::menu_to_state(&mut ours, &mut handler, st);
            }
            7 => {
                let (f, a) = menu(0x113c60, 0x113bd0);
                call(&mut r, f, &[a as u64]);
                prerace::menu_restart(&mut ours);
            }
            _ => {
                let (f, a) = menu(0x113d58, 0x113bb0);
                call(&mut r, f, &[a as u64]);
                prerace::menu_end_race(&mut ours, &mut handler, &c);
                ret = Some(r.mem.read_u32(game + 0x54));
                r.mem.write_u32(game + 0x54, 0);
            }
        }
        let what = format!("case {case} op {op}");
        assert_eq!(w.borrow().log, ours.log, "{what}: calls");
        assert_eq!(r.mem.read_u32(race + 0x14), clock.to_bits(), "{what}: clock");
        for (k, v) in latches.iter().enumerate() {
            assert_eq!(r.mem.read_u32(race + 0xe4 + 4 * k as u32), *v, "{what}: latch {k}");
        }
        assert_eq!(r.mem.read_u32(end + 4), handler.state, "{what}: handler state");
        if let Some(shown) = ret {
            assert_eq!(shown, (c.r32(0x3260) != 4) as u32, "{what}: results shown");
        }
    }
    eprintln!("ops {seen:?}");
}
