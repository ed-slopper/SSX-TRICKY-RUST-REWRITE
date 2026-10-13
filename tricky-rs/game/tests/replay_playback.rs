//! The replay's playback control and the manager's life (board row G12g) against the game in the function runner
//! (TRICKY_ELF; skipped without it): random managers (keyframes, highlights, pins, playlists, camera tracks, pad
//! streams, the race's riders through gApp) built the same in the runner and in our memory, then random calls of
//! `Replay_Update`, `_DoCommand`, `_Step`, playback start / stop / abort, recording, status, clear and the pad
//! streams; every result, every world / HUD / camera call and all the memory compared after each. The manager's
//! set-up (`ReplayMgr_Init`, `Replay_Init`, `_FreeAll`) has its own test.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::{Error, Runner};
use tricky_game::replaymgr::{self as rm, hl, kf, pin, CamEnv, Mem, ReplayEnv};

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

const MGR_LEN: u32 = 0x4100;
const IN_LEN: u32 = 0x800c;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Chase,
    Preset,
    Select(u32),
    Set([u32; 5], u32),
    Mode(u32, u32),
    Menu,
    Broad,
    Contact,
    Save(u32, u32, u32),
    Restore(u32),
    Hud(u32),
    Renders,
    Refresh,
    Fade,
    ScriptStop,
    Cancel,
    Delete(u32),
    ResetA,
    ResetInstances,
    Alloc(u32),
    Free(u32),
    MemDelete(u32),
}

/// What the world answers, the same on both sides
#[derive(Default, Clone)]
struct World {
    n: u32,
    menu: bool,
    overflow: u32,
    alloc_at: u32,
    log: Vec<Call>,
}
impl World {
    fn chase(&mut self) -> [u32; 5] {
        self.n += 1;
        let n = self.n;
        [n * 11, n * 13 + 1, n * 17 + 2, 0x4000_0000 | n, n ^ 0x55]
    }
}

/// Our side: the world plus the memory writes the stand-ins make (as the runner's stubs do)
struct Ours(World);
impl CamEnv for Ours {
    fn chase_params(&mut self) -> [u32; 5] {
        self.0.log.push(Call::Chase);
        self.0.chase()
    }
    fn preset(&mut self) -> u32 {
        self.0.log.push(Call::Preset);
        self.0.n += 1;
        self.0.n % 9
    }
    fn select_preset(&mut self, p: u32) {
        self.0.log.push(Call::Select(p));
    }
    fn set_chase_params(&mut self, c: [u32; 5], snap: u32) {
        self.0.log.push(Call::Set(c, snap));
    }
    fn set_replay_mode(&mut self, on: u32, rider: u32) {
        self.0.log.push(Call::Mode(on, rider));
    }
}
impl ReplayEnv for Ours {
    fn menu_active(&mut self) -> bool {
        self.0.log.push(Call::Menu);
        self.0.menu
    }
    fn broadphase_sort(&mut self) {
        self.0.log.push(Call::Broad);
    }
    fn contact_sort(&mut self) {
        self.0.log.push(Call::Contact);
    }
    fn save_keyframe(&mut self, m: &mut Mem, mgr: u32, k: u32, frame: u32, flag: u32) {
        self.0.log.push(Call::Save(k, frame, flag));
        m.w32(k + kf::FRAME, frame);
        m.w32(mgr + 0x58, self.0.overflow);
    }
    fn restore_keyframe(&mut self, _m: &mut Mem, _mgr: u32, k: u32) {
        self.0.log.push(Call::Restore(k));
    }
    fn hud_hide(&mut self, item: u32) {
        self.0.log.push(Call::Hud(item));
    }
    fn reset_rider_renders(&mut self) {
        self.0.log.push(Call::Renders);
    }
    fn refresh_riders(&mut self) {
        self.0.log.push(Call::Refresh);
    }
    fn start_cut_fade(&mut self) {
        self.0.log.push(Call::Fade);
    }
    fn cam_script_stop(&mut self) {
        self.0.log.push(Call::ScriptStop);
    }
    fn cancel_race_events(&mut self) {
        self.0.log.push(Call::Cancel);
    }
    fn delete_nodes(&mut self, list: u32) {
        self.0.log.push(Call::Delete(list));
    }
    fn world_reset_a(&mut self) {
        self.0.log.push(Call::ResetA);
    }
    fn world_reset_instances(&mut self) {
        self.0.log.push(Call::ResetInstances);
    }
    fn alloc(&mut self, _m: &mut Mem, size: u32) -> u32 {
        self.0.log.push(Call::Alloc(size));
        self.0.alloc_at
    }
    fn free(&mut self, _m: &mut Mem, p: u32) {
        self.0.log.push(Call::Free(p));
    }
    fn delete(&mut self, _m: &mut Mem, p: u32) {
        self.0.log.push(Call::MemDelete(p));
    }
}

struct Addrs {
    mgr: u32,
    pins: u32,
    app: u32,
    game: u32,
    race: u32,
    players: u32,
    out: u32,
    inputs: u32,
    streams: u32,
}

const N_INPUTS: u32 = 6;
const STREAMS_LEN: u32 = 0x400;

fn regions(a: &Addrs) -> Vec<(u32, u32)> {
    vec![
        (0, 0x8010),
        (a.mgr, MGR_LEN),
        (a.pins, 0x70),
        (a.app, 0x800),
        (a.game, 0x100),
        (a.race, 0x100),
        (a.players, 0x2000),
        (a.out, 0x78),
        (a.inputs, IN_LEN * N_INPUTS),
        (a.streams, STREAMS_LEN),
        (rm::GAPP, 4),
        (rm::CUTS_ON, 4),
    ]
}

fn snapshot(r: &mut Runner, a: &Addrs) -> Mem {
    let mut m = Mem::default();
    for (at, n) in regions(a) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn build(g: &mut Rng, r: &mut Runner, a: &Addrs) -> Mem {
    let mut m = snapshot(r, a);
    for (base, v) in m.regions.iter_mut() {
        if *base == a.mgr || *base == a.pins || *base == a.out || *base == a.streams {
            v.iter_mut().for_each(|b| *b = 0);
        }
    }
    let mgr = a.mgr;
    m.w32(rm::GAPP, a.app);
    m.w32(a.app + 0x730, a.game);
    m.w32(a.game + 0x40, mgr);
    m.w32(a.game + 0x1c, a.race);
    m.w32(a.game + 0x68, [0u32, 1, 2, 3, u32::MAX][g.below(5) as usize]);
    for i in 0..4u32 {
        let p = a.players + 0x10 * i;
        let b = a.players + 0x100 + 0x500 * i;
        m.w32(a.race + 0x8c + 4 * i, if g.below(6) == 0 { 0 } else { p });
        m.w32(p, b);
        m.w32(b + 0x460, 0x1000 + i);
    }
    m.w32(rm::CUTS_ON, (g.below(4) != 0) as u32);
    // keyframes
    let slot = |i: u32| mgr + 0x3e68 + 0x24 * i;
    m.w32(mgr + 8, 16);
    m.w32(mgr + 0xc, 0x8000);
    let mut order: Vec<u32> = (0..16).collect();
    for i in (1..16).rev() {
        order.swap(i, g.below(i as u32 + 1) as usize);
    }
    let n = 2 + g.below(15) as usize;
    let (list, free) = order.split_at(n);
    let mut frame = g.below(50);
    for (j, &i) in list.iter().enumerate() {
        let k = slot(i);
        m.w32(k + kf::FRAME, frame);
        frame += 1 + [0, 10, 30, 60, 200][g.below(5) as usize];
        m.w32(k + kf::PREV, if j > 0 { slot(list[j - 1]) } else { 0 });
        m.w32(k + kf::NEXT, if j + 1 < n { slot(list[j + 1]) } else { 0 });
        m.w32(k + kf::PINS, g.below(2));
        m.w32(k + kf::USED, (g.below(5) == 0) as u32);
        m.w32(k, a.streams + 0x40 * g.below(16));
    }
    for (j, &i) in free.iter().enumerate() {
        m.w32(slot(i) + kf::NEXT, if j + 1 < free.len() { slot(free[j + 1]) } else { 0 });
        m.w32(slot(i), if g.below(3) == 0 { 0 } else { a.streams + 0x40 * g.below(16) });
    }
    m.w32(mgr, slot(list[0]));
    m.w32(mgr + 4, if free.is_empty() || g.below(8) == 0 { 0 } else { slot(free[0]) });
    m.w32(mgr + 0x10, if g.below(8) == 0 { 0 } else { slot(list[n - 1]) });
    m.w32(mgr + rm::WRITING, if g.below(2) == 0 { 0 } else { slot(list[g.below(n as u32) as usize]) });
    m.w32(mgr + rm::PLAYING, slot(list[g.below(n as u32) as usize]));
    // highlights and pins
    let rec = |i: u32| mgr + 0x3968 + 0x1c * i;
    let mut recs: Vec<u32> = (0..40).collect();
    for i in (1..40).rev() {
        recs.swap(i, g.below(i as u32 + 1) as usize);
    }
    let mut used = 0usize;
    let mut by_rider: Vec<Vec<u32>> = vec![vec![]; 4];
    for rider in 0..4u32 {
        let mut kfs: Vec<u32> = list.iter().map(|&i| slot(i)).filter(|_| g.below(3) == 0).collect();
        kfs.truncate(g.below(6) as usize);
        for (i, k) in kfs.iter().enumerate() {
            let h = rec(recs[used]);
            used += 1;
            m.w32(h + hl::KEYFRAME, *k);
            m.w32(h + hl::LEN, [0, 5, 30, 100, 400][g.below(5) as usize]);
            m.w32(h + hl::KIND, g.below(3));
            m.w32(h + hl::SCORE1, ((g.below(200) as f32 - 50.0) / 7.0).to_bits());
            m.w32(h + hl::SCORE2, ((g.below(200) as f32 - 50.0) / 3.0).to_bits());
            m.w32(h + hl::RIDER, rider);
            m.w32(mgr + rm::HIGHLIGHTS + rider * 0x28 + 4 * i as u32, h);
            by_rider[rider as usize].push(h);
        }
        m.w32(mgr + rm::HIGHLIGHT_COUNT + 4 * rider, kfs.len() as u32);
    }
    let rest = &recs[used..];
    for (j, &i) in rest.iter().enumerate() {
        m.w32(rec(i) + hl::NEXT_FREE, if j + 1 < rest.len() { rec(rest[j + 1]) } else { 0 });
    }
    m.w32(mgr + rm::HL_FREE, if rest.is_empty() { 0 } else { rec(rest[0]) });
    let all: Vec<u32> = by_rider.iter().flatten().copied().collect();
    for rider in 0..4u32 {
        if g.below(4) == 0 {
            continue;
        }
        let p = a.pins + 0x1c * rider;
        m.w32(mgr + rm::PINS + 4 * rider, p);
        m.w32(p + pin::RIDER, rider);
        let mine = &by_rider[rider as usize];
        for s in 0..4 {
            let h = if mine.is_empty() || g.below(3) == 0 { 0 } else { mine[g.below(mine.len() as u32) as usize] };
            m.w32(p + pin::SLOTS + 4 * s, h);
        }
        m.w32(p + pin::KEYFRAME, if g.below(2) == 0 { 0 } else { slot(list[g.below(n as u32) as usize]) });
    }
    m.w32(mgr + rm::SLOT_COUNT, [2, 4][g.below(2) as usize]);
    m.w32(mgr + rm::QUOTA1, 1 + g.below(3));
    m.w32(mgr + rm::QUOTA2, 1 + g.below(2));
    // the playlist
    let pl = mgr + 0x38ec;
    if !all.is_empty() {
        let c = 1 + g.below(4);
        m.w32(pl, c);
        m.w32(pl + 4, g.below(c));
        for i in 0..c {
            m.w32(pl + 8 + 4 * i, all[g.below(all.len() as u32) as usize]);
        }
    }
    m.w32(mgr + rm::PLAYLIST, if !all.is_empty() && g.below(3) == 0 { pl } else { 0 });
    // camera tracks
    for t in 0..4 {
        let tr = mgr + rm::CAM_TRACKS + t * rm::CAM_TRACK_SIZE;
        let c = [0u32, 1, 3, 10][g.below(4) as usize];
        let mut f = g.below(20);
        for i in 0..c {
            let at = tr + 0x24 + 0x1c * i;
            for k in 0..6 {
                m.w32(at + 4 * k, g.next());
            }
            m.w32(at + 0x18, f);
            f += [0, 1, 30, 200][g.below(4) as usize];
        }
        for k in 0..6 {
            m.w32(tr + 8 + 4 * k, g.next());
        }
        m.w32(tr, c);
        m.w32(tr + 4, if c == 0 { u32::MAX } else { g.below(c) });
    }
    // pad streams
    for rider in 0..4u32 {
        let at = a.inputs + IN_LEN * rider;
        m.w32(mgr + rm::INPUTS + 4 * rider, if g.below(3) == 0 { 0 } else { at });
        let c = g.below(20);
        m.w32(at + 0x8000, c);
        for i in 0..c.max(1) {
            m.w8(at + 0x6664 + i, g.below(256) as u8);
        }
    }
    // the rest of the manager
    m.w32(mgr + rm::MODE, g.below(15));
    m.w32(mgr + 0x28, g.below(12));
    m.w32(mgr + rm::RIDER, g.below(4));
    m.w32(mgr + rm::END, frame + g.below(100));
    m.w32(mgr + rm::FRAME, g.below(frame + 120));
    m.w32(mgr + 0x2c, [0, 60, 120, 240, 600][g.below(5) as usize] + [0, 0, 10, 20, 3][g.below(5) as usize]);
    m.w32(mgr + 0x38, [0u32, 120, 300, 3600][(g.below(16) % 4).max(if g.below(30) == 0 { 0 } else { 1 }) as usize]);
    m.w32(mgr + 0x3c, g.below(3));
    m.w32(mgr + 0x40, (g.below(6) == 0) as u32);
    m.w32(mgr + 0x4c, (g.below(6) == 0) as u32);
    m.w32(mgr + 0x50, (g.below(6) == 0) as u32);
    m.w32(mgr + 0x54, g.below(2));
    let rider = m.r32(mgr + rm::RIDER) as usize;
    let cur = if by_rider[rider].is_empty() || g.below(2) == 0 { 0 } else { by_rider[rider][0] };
    m.w32(mgr + rm::RECORDING_HL, cur);
    m.w32(mgr + rm::RECORDING_FROM, if cur != 0 { m.r32(m.r32(cur) + kf::FRAME) } else { 0 });
    for (at, v) in &m.regions {
        r.mem.write(*at, v).unwrap();
    }
    m
}

fn stub_world(r: &mut Runner, w: &Rc<RefCell<World>>) {
    macro_rules! log {
        ($addr:expr, |$c:ident, $m:ident, $x:ident| $body:expr) => {{
            let w = w.clone();
            r.stub($addr, move |$c, $m| {
                let mut $x = w.borrow_mut();
                $body
            });
        }};
    }
    log!(0x178968, |c, m, x| {
        x.log.push(Call::Chase);
        let v = x.chase();
        for (k, reg) in [4usize, 5, 6, 7, 8].iter().enumerate() {
            m.write_u32(c.gpr(*reg) as u32, v[k]);
        }
    });
    log!(0x1786c0, |c, _m, x| {
        x.log.push(Call::Preset);
        x.n += 1;
        let v = x.n % 9;
        c.set_gpr(2, v as u64)
    });
    log!(0x1786d0, |c, _m, x| x.log.push(Call::Select(c.gpr(4) as u32)));
    log!(0x178a28, |c, _m, x| {
        let v = [c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits(), c.f(15).to_bits(), c.gpr(4) as u32];
        x.log.push(Call::Set(v, c.gpr(5) as u32))
    });
    log!(0x1785c0, |c, _m, x| x.log.push(Call::Mode(c.gpr(4) as u32, c.gpr(5) as u32)));
    log!(0x17f0f8, |c, _m, x| {
        x.log.push(Call::Menu);
        let v = x.menu;
        c.set_gpr(2, v as u64)
    });
    log!(0x14fe18, |_c, _m, x| x.log.push(Call::Broad));
    log!(0x150428, |_c, _m, x| x.log.push(Call::Contact));
    log!(0x18f448, |c, m, x| {
        let (mgr, k, frame, flag) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32);
        x.log.push(Call::Save(k, frame, flag));
        m.write_u32(k + kf::FRAME, frame);
        m.write_u32(mgr + 0x58, x.overflow);
    });
    log!(0x18f5c8, |c, _m, x| x.log.push(Call::Restore(c.gpr(5) as u32)));
    log!(0x1a7860, |c, _m, x| x.log.push(Call::Hud(c.gpr(5) as u32)));
    log!(0x1128b8, |_c, _m, x| x.log.push(Call::Renders));
    log!(0x1147f8, |_c, _m, x| x.log.push(Call::Refresh));
    log!(0x174c30, |_c, _m, x| x.log.push(Call::Fade));
    log!(0x175eb8, |_c, _m, x| x.log.push(Call::ScriptStop));
    log!(0x210b50, |_c, _m, x| x.log.push(Call::Cancel));
    log!(0x17da50, |c, _m, x| x.log.push(Call::Delete(c.gpr(5) as u32)));
    log!(0x115df0, |_c, _m, x| x.log.push(Call::ResetA));
    log!(0x115fe8, |_c, _m, x| x.log.push(Call::ResetInstances));
    log!(0x2ccf70, |c, _m, x| {
        x.log.push(Call::Alloc(c.gpr(5) as u32));
        let v = x.alloc_at;
        c.set_gpr(2, v as u64)
    });
    log!(0x14a040, |c, _m, x| x.log.push(Call::Free(c.gpr(4) as u32)));
    log!(0x114f80, |c, _m, x| x.log.push(Call::MemDelete(c.gpr(4) as u32)));
}

/// Every replay function the calls may reach runs as the game's (the index's entries from 0x18e850 to 0x195710 and
/// `EAMem_Set`), but the stand-ins
fn allow_replay(r: &mut Runner) {
    let csv = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/function-index.csv")).unwrap();
    for l in csv.lines().skip(1) {
        let Some(a) = l.split(',').next().and_then(|a| u32::from_str_radix(a.trim_start_matches("0x"), 16).ok()) else { continue };
        if (0x18e850..0x195710).contains(&a) && a != 0x18f448 && a != 0x18f5c8 {
            r.allow(a);
        }
    }
    r.allow(0x2cc808);
}

fn compare(r: &mut Runner, a: &Addrs, m: &Mem, what: &str) {
    let theirs = snapshot(r, a);
    for ((at, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

fn addrs(r: &mut Runner) -> Addrs {
    Addrs {
        mgr: r.mem.alloc(MGR_LEN),
        pins: r.mem.alloc(0x70),
        app: r.mem.alloc(0x800),
        game: r.mem.alloc(0x100),
        race: r.mem.alloc(0x100),
        players: r.mem.alloc(0x2000),
        out: r.mem.alloc(0x78),
        inputs: r.mem.alloc(IN_LEN * N_INPUTS),
        streams: r.mem.alloc(STREAMS_LEN),
    }
}

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
