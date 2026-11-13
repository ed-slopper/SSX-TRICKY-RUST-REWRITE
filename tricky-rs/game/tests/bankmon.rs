//! `tricky_game::bankmon` (board row E1b2g7d5e) against the original in the function runner (TRICKY_ELF; skipped
//! without it): a random monitor (its sound queue, arrays and flags) and 64 random instances (engines, handles,
//! states, distances, volumes, keys), every function run on both sides with the same answers from the calls out (the
//! sound library's handles and MIDI players, the streams, the monitor's position virtual), the calls logged in order
//! and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bankmon::*;
use tricky_game::musicsong::SongEnv;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const MON: u32 = ARENA;
const MVT: u32 = ARENA + 0x900;
const ARRAYS: u32 = ARENA + 0xa00;
const REC: u32 = ARENA + 0xd00;
const INST: u32 = ARENA + 0x1010;
const POS: u32 = ARENA + 0x3c80;
const SCALE: u32 = ARENA + 0x3d00;
const NAME: u32 = ARENA + 0x3e00;
/// The monitor's position virtual
const V_LOCATE: u32 = 0x7f_0000;

/// The leaves the runner stubs, with their integer argument counts
const LEAVES: &[(u32, usize)] = &[
    (SND_LOCK, 0), (SND_UNLOCK, 0), (HANDLE_STATE, 1), (HANDLE_FADE, 3), (HANDLE_VOLUME, 2), (HANDLE_PITCH, 2),
    (HANDLE_STOP, 1), (HANDLE_PAN, 3), (PLAY_BANK_SOUND, 3), (MIDI_STOP, 1), (MIDI_PLAY, 4), (MIDI_PITCH, 2),
    (MIDI_PAN, 2), (MIDI_PAUSE, 1), (MIDI_RESUME, 1), (MIDI_VOLUME, 2), (MIDI_FADE, 3), (STREAM_PAN, 3),
    (STREAM_VOLUME, 2), (STREAM_PITCH, 2), (STREAM_STOP, 1), (VOICE_IS_DONE, 1), (VOICE_SET_VOLUME, 2),
    (MEM_DELETE, 1), (MEM_FREE, 1), (EAMEM_ALLOC, 3), (EAMEM_SET, 3), (MONITOR_CTOR, 2), (MONITOR_DTOR, 2),
    (DEBUG_PRINTF, 1), (V_LOCATE, 2), (HANDLE_PITCH_BEND, 2),
];

const ADDR: [u32; 36] = [
    0x20a888, 0x20a8c0, 0x20a9d8, 0x20aa20, 0x20aab8, 0x20ab28, 0x20ab48, 0x20aba8, 0x20abf8, 0x20abb8, 0x20ac08,
    0x20ac50, 0x20ad38, 0x20b0b8, 0x20b158, 0x20b688, 0x20b700, 0x20b948, 0x20b9b0, 0x20ba00, 0x20bb98, 0x20bc60,
    0x20bce8, 0x20bd68, 0x20bfe8, 0x20c040, 0x20c098, 0x20c0e0, 0x20c0f8, 0x20c148, 0x20c200, 0x20c2a0, 0x20c308,
    0x20c380, 0x20c3e8, 0x21e7e0,
];

struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    /// One call's answer: v0 and the words it gives ([f0, the four words it writes] for the position virtual, [v0,
    /// the state] for `SndHandle_GetState`)
    fn reply(&mut self, target: u32) -> (u32, Vec<u32>) {
        let n = match self.count.iter_mut().find(|c| c.0 == target) {
            Some(c) => {
                c.1 += 1;
                c.1
            }
            None => {
                self.count.push((target, 0));
                0
            }
        };
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..3 {
            g.next();
        }
        let v = match target {
            VOICE_IS_DONE => g.below(2),
            PLAY_BANK_SOUND | MIDI_PLAY => g.pick(&[0u32, 1, 2, u32::MAX, 0x1234]),
            EAMEM_ALLOC => g.pick(&[INST - 0x10, INST - 0x10, 0]),
            _ => g.next(),
        };
        let words = match target {
            V_LOCATE => {
                let d2 = g.pick(&[0.0f32, 0.2, 25.0, 900.0, 1e4, 2.5e5, 1e7, -4.0]);
                vec![d2.to_bits(), g.next(), g.next(), g.next(), g.next()]
            }
            HANDLE_STATE => vec![g.pick(&[u32::MAX, 0, 1, 2]), g.pick(&[0u32, 1, 2, 3])],
            _ => vec![],
        };
        (v, words)
    }
}
type A = Rc<RefCell<Answers>>;

struct Ours(A);
impl SongEnv for Ours {
    fn call(&mut self, _m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        self.0.borrow_mut().reply(target).0
    }
    fn call_f(&mut self, _m: &mut Mem, _target: u32, _args: &[u32]) -> f32 {
        unreachable!()
    }
    fn call_out(&mut self, _m: &mut Mem, target: u32, args: &[u32], _words: usize) -> Vec<u32> {
        self.0.borrow_mut().log.push((target, args.to_vec()));
        self.0.borrow_mut().reply(target).1
    }
}

const REGIONS: [(u32, u32); 3] = [(ARENA, 0x4000), (0, 0x2c20), (0x39e600, 0x200)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn fl(v: f32) -> u32 {
    v.to_bits()
}

#[test]
fn bank_monitor_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    for &(f, n) in LEAVES {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let v: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            let (ans, words) = a2.borrow_mut().reply(f);
            match f {
                V_LOCATE => {
                    let buf = c.gpr(6) as u32;
                    for k in 0..4 {
                        mem.write_u32(buf + 4 * k, words[1 + k as usize]);
                    }
                    c.fpr[0] = words[0];
                }
                HANDLE_STATE => {
                    mem.write_u32(c.gpr(5) as u32, words[1]);
                }
                _ => {}
            }
            a2.borrow_mut().log.push((f, v));
            let ans = if f == HANDLE_STATE { words[0] } else { ans };
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    for &f in ADDR.iter().chain(&[0x2fd14c, 0x2fb468]) {
        r.allow(f);
    }
    let mut g = Rng(0x20b7_00ba_4c00_0001);
    let mut seen = [0u32; 36];
    let dists = [0.0f32, 0.3, 0.5, 1.0, 10.0, 29.9, 30.0, 100.0, 1000.0, -1.0];
    for case in 0..60_000u32 {
        for k in 0..0x4000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.next() });
        }
        for k in 0..0x2c20 / 4 {
            r.mem.write_u32(4 * k, 0);
        }
        // the monitor: its virtual, flags, queue and arrays
        r.mem.write_u32(MON + 4, MVT);
        r.mem.write_u16(MVT + 0x48, g.pick(&[0u16, 8, 0xfffc]));
        r.mem.write_u32(MVT + 0x4c, V_LOCATE);
        r.mem.write_u32(MON + 0x20, g.below(3));
        r.mem.write_u32(MON + 0x24, REC);
        for k in 0..4 {
            r.mem.write_u32(MON + 0x28 + 4 * k, g.pick(&[0, POS, POS + 4, POS + 0x10]));
        }
        for (k, o) in [0x38u32, 0x3c, 0x40, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x7c].iter().enumerate() {
            let arr = ARRAYS + 0x40 * k as u32;
            r.mem.write_u32(MON + o, arr);
            for i in 0..16 {
                let v = match o {
                    0x38 | 0x78 | 0x7c => g.below(2),
                    0x3c => g.pick(&[0u32, 50, 127, 200, u32::MAX - 4]),
                    0x40 => g.pick(&[0, SCALE, SCALE + 4, SCALE + 8]),
                    0x6c | 0x70 => fl(g.pick(&[0.0f32, 10.0, 30.0, 500.0])),
                    _ => g.next(),
                };
                r.mem.write_u32(arr + 4 * i, v);
            }
        }
        for (k, v) in [0.5f32, 1.0, 0.0, 2.0].iter().enumerate() {
            r.mem.write_u32(SCALE + 4 * k as u32, fl(*v));
        }
        r.mem.write_u32(MON + 0x8d0, g.below(4));
        r.mem.write_u32(MON + 0x8d4, g.below(2));
        r.mem.write_u32(MON + 0x8d8, g.below(2));
        r.mem.write_u32(MON + 0x8dc, g.below(2));
        r.mem.write_u32(MON + 0x8e0, if g.below(20) == 0 { 0 } else { INST });
        r.mem.write_u32(INST - 0x10, 64);
        let handles = [0u32, 1, 2, u32::MAX];
        for k in 0..64 {
            let s = INST + 0xb0 * k;
            r.mem.write_u32(s, g.pick(&[0u32, 1, 2, 3, 0, 1, 1, 2]));
            r.mem.write_u32(s + 4, g.pick(&handles));
            r.mem.write_u32(s + 8, g.below(2));
            r.mem.write_u32(s + 0x54, if g.below(4) == 0 { 1 } else { 0 });
            r.mem.write_u32(s + 0x58, g.below(2));
            r.mem.write_u32(s + 0x5c, fl(g.pick(&[-1.0f32, 0.0, 5.0, 31.0, 200.0])));
            r.mem.write_u32(s + 0x60, g.pick(&[0, SCALE, SCALE + 4, SCALE + 8]));
            r.mem.write_u32(s + 0x64, g.pick(&[0u32, 50, 127, 128, 200, u32::MAX - 2]));
            r.mem.write_u32(s + 0x68, g.pick(&[0, POS, s + 0x70]));
            r.mem.write_u32(s + 0x88, fl(g.pick(&[0.0f32, 30.0, 500.0])));
            r.mem.write_u32(s + 0x8c, fl(g.pick(&[0.0f32, 10.0, 1000.0])));
            r.mem.write_u32(s + 0x94, g.below(2));
            r.mem.write_u32(s + 0x98, g.below(2));
            r.mem.write_u32(s + 0x9c, g.pick(&[0u32, 0x0001_0000, 0xffff_0000]));
            r.mem.write_u32(s + 0xa0, g.pick(&[0u32, 1, 0x0001_0001, 0xffff_ffff]));
            r.mem.write_u32(s + 0xa4, g.pick(&[0u32, 0x1000, 0x8000, 0x10ff, 0x3fff]));
            r.mem.write_u32(s + 0xac, g.pick(&[0u32, 0xffff, 0x200, 0xffff_8ad0]));
        }
        let name = b"stream1";
        r.mem.write(NAME, name).unwrap();
        r.mem.write_u8(NAME + name.len() as u32, 0);
        let which = g.below(36);
        seen[which as usize] += 1;
        let k = g.below(64);
        let s = INST + 0xb0 * k;
        let w = [g.next(), g.next(), g.next()];
        let d = g.pick(&dists);
        let far = g.pick(&[0.0f32, 30.0, 50.0, -1.0, 1000.0]);
        let vol = g.pick(&[0u32, 127, 100, 255, u32::MAX - 2, u32::MAX]);
        let fade = g.pick(&[0.0f32, 0.5, 2.0]);
        let handle = g.pick(&handles);
        let kind = g.pick(&[0u32, 1, 2, 3]);
        let rnd = g.next();
        let pan = g.pick(&[0u32, 0xffff, 0x100, 0x200, 0x3ff, rnd]);
        let pitch = g.pick(&[0u32, 0x1000, 0x4000, 0x3fff, u32::MAX, 0x80, rnd]);
        let closed = g.pick(&[0.0f32, 10.0, -10.0, 1000.0, -1e5, 1e5]);
        let dt = g.pick(&[1.0f32 / 60.0, 1.0 / 30.0, 1.0, 0.0]);
        let key = (g.pick(&[0u64, 1, 0xffff]) << 16) | (g.pick(&[0u64, 1, 0x0001_0001, 0xffff_ffff]) << 32) | g.below(0x10000) as u64;
        // (integer arguments, float arguments in f12 up)
        let (args, fargs): (Vec<u32>, Vec<f32>) = match which {
            0 | 2 | 3 | 13 | 17 | 18 => (vec![s], vec![]),
            1 => (vec![w[0], g.below(4)], vec![]),
            4 => (vec![vol], vec![d, far]),
            5 => (vec![s, vol], vec![d, far]),
            6 => (vec![], vec![closed, dt]),
            7 | 8 => (vec![s, w[0], w[1]], vec![]),
            9 => (vec![s, w[0], NAME, w[1]], vec![]),
            10 | 14 => (vec![s, MON], vec![]),
            11 => (vec![s, pan, w[0]], vec![]),
            12 => (vec![s, pitch], vec![]),
            15 => (vec![s, MON], vec![d]),
            16 => (vec![s, MON], vec![g.pick(&[1.0f32 / 60.0, 1.0 / 30.0, 0.1])]),
            19 => (vec![s, vol, g.below(2)], vec![fade]),
            20 => (vec![MON, w[0]], vec![]),
            21 => (vec![MON, g.below(4)], vec![]),
            22 => (vec![MON, g.pick(&[0u32, 1, 2, 5, 64])], vec![]),
            23 => (vec![MON, handle, vol, g.pick(&[0u32, 1, 2, 3])], vec![]),
            24 | 25 | 28 | 32 | 33 | 34 => (vec![MON], vec![]),
            26 | 31 => (vec![MON, handle, kind], vec![fade]),
            27 => (vec![MON, k], vec![]),
            35 => (vec![s, pitch], vec![]),
            29 => (vec![MON], vec![]),
            _ => (vec![MON, handle, kind], vec![]),
        };
        // (only the destructor and the constructor take a monitor without its instances)
        if which >= 22 && r.mem.read_u32(MON + 0x8e0) == 0 {
            r.mem.write_u32(MON + 0x8e0, INST);
        }
        let seed = g.next() as u64 | (g.next() as u64) << 32;
        *a.borrow_mut() = Answers { seed, count: vec![], log: vec![] };
        let mut m = snap(&mut r);
        let addr = ADDR[which as usize];
        for (k, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        if which == 29 {
            r.cpu.set_gpr(5, key);
        }
        for (k, v) in fargs.iter().enumerate() {
            r.cpu.fpr[12 + k] = v.to_bits();
        }
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which} {addr:#x}): {e}"));
        let (v0, f0) = (r.cpu.gpr(2) as u32, r.cpu.fpr[0]);
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        *a.borrow_mut() = Answers { seed, count: vec![], log: vec![] };
        let mut e = Ours(a.clone());
        let x = |k: usize| args[k];
        let y = |k: usize| F(fargs[k]);
        let mut ours_f0 = None;
        let ours_v0: Option<u32> = match which {
            0 => Some(init(&mut m, x(0))),
            1 => { delete_if(&mut m, &mut e, x(0), x(1)); None }
            2 => { reset(&mut m, &mut e, x(0)); None }
            3 => Some(is_free(&mut m, &mut e, x(0)) as u32),
            4 => Some(distance_volume(x(0), y(0), y(1))),
            5 => Some(instance_distance_volume(x(1), y(0), y(1))),
            6 => { ours_f0 = Some(doppler_pitch(y(0), y(1)).0.to_bits()); None }
            7 | 8 => { set_handle(&mut m, x(0), x(1), x(2)); None }
            9 => { set_stream(&mut m, x(0), x(1), x(2), x(3)); None }
            10 => { compute_distance(&mut m, &mut e, x(0), x(1)); None }
            11 => { set_pan(&mut m, &mut e, x(0), x(1), x(2)); None }
            12 => { set_pitch(&mut m, &mut e, x(0), x(1)); None }
            13 => { voice_stop(&mut m, &mut e, x(0)); None }
            14 => { start(&mut m, &mut e, x(0), x(1)); None }
            15 => Some(cull_by_distance(&mut m, &mut e, x(0), x(1), y(0)) as u32),
            16 => { update_positional(&mut m, &mut e, x(0), x(1), y(0)); None }
            17 => { pause(&mut m, &mut e, x(0)); None }
            18 => { resume(&mut m, &mut e, x(0)); None }
            19 => { stop(&mut m, &mut e, x(0), x(1), x(2), y(0)); None }
            20 => Some(monitor_init(&mut m, &mut e, x(0), x(1))),
            21 => { monitor_destroy(&mut m, &mut e, x(0), x(1)); None }
            22 => Some(find_free(&mut m, &mut e, x(0), x(1)) as u32),
            23 => Some(add_instance(&mut m, &mut e, x(0), x(1), x(2), x(3))),
            24 => { pause_all(&mut m, &mut e, x(0)); None }
            25 => { resume_all(&mut m, &mut e, x(0)); None }
            26 => { stop_bank_sound_soft(&mut m, &mut e, x(0), x(1), x(2), y(0)); None }
            27 => { reset_instance(&mut m, &mut e, x(0), x(1)); None }
            28 => { reset_all(&mut m, &mut e, x(0)); None }
            29 => { reset_key(&mut m, &mut e, x(0), key); None }
            30 => Some(find(&mut m, &mut e, x(0), x(1), x(2))),
            31 => { stop_bank_sound(&mut m, &mut e, x(0), x(1), x(2), y(0)); None }
            32 => Some(count_playing(&mut m, &mut e, x(0))),
            33 => Some(count_used(&mut m, &mut e, x(0))),
            34 => { reset_distances(&mut m, x(0)); None }
            _ => { set_pitch_bend(&mut m, &mut e, x(0), x(1)); None }
        };
        let ours = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(ours, theirs, "case {case} ({which} {addr:#x}): the calls");
        if let Some(v) = ours_v0 {
            assert_eq!(v, v0, "case {case} ({which} {addr:#x}): v0");
        }
        if let Some(v) = ours_f0 {
            assert_eq!(v, f0, "case {case} ({which} {addr:#x}): f0 {} vs {}", f32::from_bits(f0), f32::from_bits(v));
        }
        let after = snap(&mut r);
        for ((at, b), (_, ob)) in after.regions.iter().zip(m.regions.iter()) {
            if b != ob {
                let i = b.iter().zip(ob.iter()).position(|(x, y)| x != y).unwrap();
                panic!("case {case} ({which} {addr:#x}): memory differs at {:#x}: {:#x} (original) vs {:#x}", at + i as u32, b[i], ob[i]);
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 1000), "{seen:?}");
}
