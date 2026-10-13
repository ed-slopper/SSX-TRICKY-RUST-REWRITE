//! The replay's camera tracks (board row G12f) against the game in the function runner (TRICKY_ELF; skipped
//! without it): random tracks (up to the 128 cuts), the cuts flag on and off, random calls; every result, every
//! camera call and all the memory compared after each.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::replaymgr::{self as rm, CamEnv, Mem};

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

const MGR_LEN: u32 = 0x40a8;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Chase,
    Preset,
    Select(u32),
    Set([u32; 5], u32),
    Mode(u32, u32),
}

/// The camera as both sides see it: the n-th read of the chase words or the preset gives values from n
#[derive(Default)]
struct Cam {
    n: u32,
    log: Vec<Call>,
}
impl Cam {
    fn chase(&mut self) -> [u32; 5] {
        self.n += 1;
        let n = self.n;
        [n * 11, n * 13 + 1, n * 17 + 2, 0x4000_0000 | n, n ^ 0x55]
    }
    fn preset_value(&mut self) -> u32 {
        self.n += 1;
        self.n % 9
    }
}
impl CamEnv for Cam {
    fn chase_params(&mut self) -> [u32; 5] {
        self.log.push(Call::Chase);
        self.chase()
    }
    fn preset(&mut self) -> u32 {
        self.log.push(Call::Preset);
        self.preset_value()
    }
    fn select_preset(&mut self, p: u32) {
        self.log.push(Call::Select(p));
    }
    fn set_chase_params(&mut self, c: [u32; 5], snap: u32) {
        self.log.push(Call::Set(c, snap));
    }
    fn set_replay_mode(&mut self, on: u32, rider: u32) {
        self.log.push(Call::Mode(on, rider));
    }
}

fn call(r: &mut Runner, f: u32, args: &[u32]) -> u32 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, *a as i32 as i64 as u64);
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e:?}"));
    r.cpu.gpr(2) as u32
}

#[test]
fn replay_camera_tracks_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x194d10, 0x194d78, 0x1952d8, 0x195340, 0x1953c8, 0x1950b8, 0x1951d0, 0x1951f8, 0x194d18, 0x190c28, 0x2cc808] {
        r.allow(a);
    }
    let cam = Rc::new(RefCell::new(Cam::default()));
    let c = cam.clone();
    r.stub(0x178968, move |cpu, m| {
        let v = c.borrow_mut().chase_params();
        for (k, reg) in [4usize, 5, 6, 7, 8].iter().enumerate() {
            m.write_u32(cpu.gpr(*reg) as u32, v[k]);
        }
    });
    let c = cam.clone();
    r.stub(0x1786c0, move |cpu, _| {
        let v = c.borrow_mut().preset();
        cpu.set_gpr(2, v as u64)
    });
    let c = cam.clone();
    r.stub(0x1786d0, move |cpu, _| c.borrow_mut().select_preset(cpu.gpr(4) as u32));
    let c = cam.clone();
    r.stub(0x178a28, move |cpu, _| {
        // the first four words come in f12..f15 (moved, never computed on), the fifth in a0, the snap flag in a1
        let v = [cpu.f(12).to_bits(), cpu.f(13).to_bits(), cpu.f(14).to_bits(), cpu.f(15).to_bits(), cpu.gpr(4) as u32];
        c.borrow_mut().set_chase_params(v, cpu.gpr(5) as u32)
    });
    let c = cam.clone();
    r.stub(0x1785c0, move |cpu, _| c.borrow_mut().set_replay_mode(cpu.gpr(4) as u32, cpu.gpr(5) as u32));
    let mgr = r.mem.alloc(MGR_LEN);
    let regions = [(mgr, MGR_LEN), (rm::CUTS_ON, 4)];
    let snapshot = |r: &mut Runner| {
        let mut m = Mem::default();
        for (a, n) in regions {
            let mut b = vec![0u8; n as usize];
            r.mem.read(a, &mut b).unwrap();
            m.regions.push((a, b));
        }
        m
    };
    let mut g = Rng(0x6361_6d74_7261_0f01);
    let mut seen = [0u32; 16];
    for trial in 0..2000 {
        // random tracks: cuts in frame order (some frames shared), a cut playing
        let mut m = snapshot(&mut r);
        for (_, v) in m.regions.iter_mut() {
            v.iter_mut().for_each(|b| *b = 0);
        }
        for t in 0..4 {
            let tr = mgr + rm::CAM_TRACKS + t * rm::CAM_TRACK_SIZE;
            let n = [0u32, 1, 2, 5, 20, 127, 128][g.below(7) as usize];
            let mut f = g.below(20);
            for i in 0..n {
                let at = tr + 0x24 + 0x1c * i;
                for k in 0..6 {
                    m.w32(at + 4 * k, g.next());
                }
                m.w32(at + 0x18, f);
                f += [0, 1, 5, 60][g.below(4) as usize];
            }
            for k in 0..6 {
                m.w32(tr + 8 + 4 * k, g.next());
            }
            m.w32(tr, n);
            m.w32(tr + 4, if n == 0 { u32::MAX } else { g.below(n) });
        }
        m.w32(mgr + rm::RIDER, g.below(4));
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        for step in 0..30 {
            let op = g.below(16) as usize;
            let what = format!("trial {trial} step {step} op {op}");
            let on = (g.below(4) != 0) as u32;
            m.w32(rm::CUTS_ON, on);
            r.mem.write_u32(rm::CUTS_ON, on);
            let t = mgr + rm::CAM_TRACKS + g.below(4) * rm::CAM_TRACK_SIZE;
            let tf = m.r32(t + 0x24 + 0x1c * g.below(m.r32(t).max(1)) + 0x18);
            let frame = match g.below(4) {
                0 => g.below(4000),
                1 => tf,
                2 => tf.wrapping_add(1),
                _ => tf.wrapping_sub(1),
            };
            let rider = g.below(4);
            let i = g.below(130);
            let mut ours_cam = Cam::default();
            let v: Option<u32> = {
                let (m, e) = (&mut m, &mut ours_cam);
                match op {
                    0 => {
                        rm::cam_track_init(m, t);
                        None
                    }
                    1 => {
                        rm::cam_track_generate(m, e, t, rider);
                        None
                    }
                    2 => Some(rm::cam_track_insert(m, e, t, frame) as u32),
                    3 => {
                        rm::cam_track_update_current(m, e, t, frame);
                        None
                    }
                    4 => Some(rm::cam_track_can_insert(m, t) as u32),
                    5 => Some(rm::cam_track_remove_at(m, t, frame) as u32),
                    6 => {
                        rm::cam_track_seek_frame(m, t, frame);
                        None
                    }
                    7 => Some(rm::cam_track_advance(m, t, frame) as u32),
                    8 => Some(rm::cam_track_cut_frame(m, t, i)),
                    9 => {
                        rm::cam_track_apply(m, e, t);
                        None
                    }
                    10 => {
                        rm::apply_camera_cut(m, e, mgr);
                        None
                    }
                    11 => Some(rm::insert_cut(m, e, mgr, frame) as u32),
                    12 => {
                        rm::update_cam_track(m, e, mgr, frame);
                        None
                    }
                    13 => Some(rm::can_insert_cut(m, mgr) as u32),
                    14 => Some(rm::next_cut(m, e, mgr, frame) as u32),
                    _ => {
                        rm::generate_camera_tracks(m, e, mgr);
                        None
                    }
                }
            };
            seen[op] += 1;
            let (f, args): (u32, Vec<u32>) = match op {
                0 => (0x194cc0, vec![t]),
                1 => (0x194d18, vec![t, rider]),
                2 => (0x194d78, vec![t, frame]),
                3 => (0x1950b8, vec![t, frame]),
                4 => (0x1951d0, vec![t]),
                5 => (0x1951f8, vec![t, frame]),
                6 => (0x1952d8, vec![t, frame]),
                7 => (0x195340, vec![t, frame]),
                8 => (0x1953a0, vec![t, i]),
                9 => (0x1953c8, vec![t]),
                10 => (0x190c28, vec![mgr]),
                11 => (0x190c40, vec![mgr, frame]),
                12 => (0x190c70, vec![mgr, frame]),
                13 => (0x190c88, vec![mgr]),
                14 => (0x190cb8, vec![mgr, frame]),
                _ => (0x190d40, vec![mgr]),
            };
            *cam.borrow_mut() = Cam::default();
            let ret = call(&mut r, f, &args);
            if let Some(v) = v {
                assert_eq!(ret, v, "{what}: result");
            }
            assert_eq!(cam.borrow().log, ours_cam.log, "{what}: camera calls");
            let theirs = snapshot(&mut r);
            for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
                if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                    let i = i & !3;
                    panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", a + i as u32, &x[i..i + 4], &y[i..i + 4]);
                }
            }
        }
    }
    eprintln!("ops {seen:?}");
}
