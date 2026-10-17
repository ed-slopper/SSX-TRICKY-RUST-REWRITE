//! `tricky_game::animctl` (board row G5a2) against the original in the function runner (TRICKY_ELF; skipped without
//! it): every case a fresh random controller (its three layers with random sequences of real clip ids, often of the
//! layer's current clip, their source trees and headers, the rider's words, the key table), then one of the 23
//! functions with random arguments. The game's own clip table is used, and clips are picked by its end action and
//! update mode so that each comes up. The sequences' functions run as the original; the clip data, the sounds, the
//! pose blend, the motion modes (G5b), delete, the tree maker and the pool are logging stand-ins. The controller's
//! bytes, the key table, the globals, the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animctl as c;
use tricky_game::animseq::{self as a, SeqEnv};
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick(&mut self, v: &[u32]) -> u32 {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x4000;
const CTL: u32 = ARENA;
const RIDER: u32 = ARENA + 0x200;
const SEQS: u32 = ARENA + 0x800;
const NSEQ: u32 = 10;
const POOL_AT: u32 = ARENA + 0x1000;
const TREES: u32 = ARENA + 0x2000;
const NTREE: u32 = 24;
const HDRS: u32 = ARENA + 0x2400;
const NHDR: u32 = 12;
const KEYS: u32 = a::KEY_TABLE;
const NKEY: u32 = 0x584;
const NCLIP: u32 = 0x2ff;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
    pool_next: u32,
}

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

/// The stand-ins' answers (functions of their arguments, so both sides get the same)
fn answer(w: &mut World, f: u32, args: &[u32]) -> u32 {
    match f {
        a::SKEL_ANIM_CREATE => TREES + 0x20 * (args[1] % NTREE),
        a::POOL_ALLOC => {
            let p = POOL_AT + 0x90 * w.pool_next;
            w.pool_next += 1;
            p
        }
        c::RESOLVE => {
            let h = mix(args[0] ^ args[1] << 12);
            if h % 6 == 0 {
                0x585
            } else {
                h % NKEY
            }
        }
        c::GET_CLIP_ID => mix(args[1] ^ 0x77),
        c::GET_MARKER_FRAME => {
            let h = mix(args[0] ^ args[1] << 7 ^ args[2] << 20);
            if h % 3 == 0 {
                h % 50
            } else {
                (h % 4).wrapping_neg()
            }
        }
        c::GET_MARKER_PHASE => ((mix(args[0] ^ args[1] ^ args[2] << 9) % 1000) as f32 / 1000.0).to_bits(),
        _ => 0,
    }
}

struct Ours(World);
impl SeqEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        answer(&mut self.0, func, args)
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

struct Table {
    by_action: Vec<Vec<u32>>,
    by_mode: Vec<Vec<u32>>,
    by_layer: Vec<Vec<u32>>,
}

/// A clip id: by end action, by update mode or any
fn some_clip(g: &mut Rng, t: &Table) -> u32 {
    match g.below(3) {
        0 => {
            let v: Vec<&Vec<u32>> = t.by_action.iter().filter(|v| !v.is_empty()).collect();
            let i = g.below(v.len() as u32) as usize;
            g.pick(v[i])
        }
        1 => {
            let v: Vec<&Vec<u32>> = t.by_mode.iter().filter(|v| !v.is_empty()).collect();
            let i = g.below(v.len() as u32) as usize;
            g.pick(v[i])
        }
        _ => g.below(NCLIP),
    }
}

fn make_world(g: &mut Rng, m: &mut Mem, none: u32, t: &Table) {
    for k in 0..SIZE / 4 {
        m.w32(ARENA + 4 * k, g.next());
    }
    for h in 0..NHDR {
        let at = HDRS + 0x30 * h;
        m.w8(at + 9, g.below(3) as u8);
        let len = 1 + g.below(40);
        m.w16(at + 0xa, len as u16);
        for k in 0..8 {
            let v = if g.below(3) == 0 { 0xffff } else { g.below(len + 2) };
            m.w16(at + 0xc + 2 * k, v as u16);
        }
    }
    for tr in 0..NTREE {
        let at = TREES + 0x20 * tr;
        m.w32(at + 8, HDRS + 0x30 * g.below(NHDR));
        for k in 0..2 {
            let later = NTREE - 1 - tr;
            let child = if later == 0 || g.below(3) == 0 { 0 } else { TREES + 0x20 * (tr + 1 + g.below(later)) };
            m.w32(at + 0xc + 4 * k, child);
        }
    }
    for k in 0..NKEY {
        let v = if g.below(3) == 0 { none } else { g.below(40) | g.below(3) << 16 };
        m.w32(KEYS + 4 * k, v);
    }
    // the controller
    for k in 0..3 {
        let l = c::layer(CTL, k);
        m.w32(l + 0x34, a::SKEL_VTABLE);
        m.w32(l + 0x38 + 0x34, a::SKEL_VTABLE);
        a::list_init(m, l);
    }
    wf(m, CTL + 0x18, if g.below(3) == 0 { 1.0 } else { g.f(-2.0, 2.0) });
    wf(m, CTL + 0x20, g.f(-3.0, 3.0));
    m.w32(CTL + 0x24, RIDER);
    m.w32(RIDER + 0x420, if g.below(2) == 0 { 2 } else { g.below(7) });
    // the sequences: a layer each, of clips on that layer (or any), some the layer's current clip
    for s in 0..NSEQ {
        let at = SEQS + 0x90 * s;
        m.w32(at + 0x34, a::BDR_VTABLE);
        let (w, rate) = match g.below(5) {
            0 => (0.5, 0.5),
            1 => (0.25, -0.25),
            _ => (g.f(-0.2, 1.2), g.f(-0.3, 0.3)),
        };
        wf(m, at + 8, w);
        wf(m, at + 0xc, rate);
        wf(m, at + 0x10, [1.0, -1.0, 0.5, g.f(-2.0, 2.0)][g.below(4) as usize]);
        wf(m, at + 0x14, g.f(-4.0, 4.0));
        m.w32(at + 0x20, if g.below(4) == 0 { 1 } else { 0 });
        m.w32(at + 0x24, g.below(2) * (1 + g.below(3)));
        m.w32(at + 0x44, 1 + g.below(3));
        for i in 0..3 {
            let tt = match g.below(3) {
                0 => g.below(45) as f32,
                1 => 0.0,
                _ => g.f(-3.0, 45.0),
            };
            wf(m, at + 0x48 + 4 * i, tt);
            m.w32(at + 0x78 + 4 * i, TREES + 0x20 * g.below(NTREE));
            m.w32(at + 0x84 + 4 * i, if g.below(4) == 0 { none } else { g.below(40) | g.below(3) << 16 });
        }
        m.w32(at + 0x60, if g.below(2) == 0 { u32::MAX } else { g.below(45) });
        m.w32(at + 0x68, g.below(2));
        m.w32(at + 0x6c, g.below(50).wrapping_sub(5));
        wf(m, at + 0x70, g.f(-0.5, 0.0));
        m.w8(at + 0x77, g.next() as u8);
        m.w8(at + 0x74, g.next() as u8);
        m.w8(at + 0x75, g.next() as u8);
        let k = g.below(3);
        let clip = if g.below(2) == 0 { g.pick(&t.by_layer[k as usize]) } else { some_clip(g, t) };
        m.w32(at + 0x40, clip);
        a::list_insert(m, c::layer(CTL, k), at);
    }
    // each layer's current clip: one of its sequences' (newest or not), none, or any
    for k in 0..3 {
        let l = c::layer(CTL, k);
        let mut on = Vec::new();
        let mut n = m.r32(l + 0x30);
        while n != l + 0x38 {
            on.push(m.r32(n + 0x40));
            n = m.r32(n + 0x30);
        }
        let cur = match g.below(4) {
            0 => c::NONE,
            1 => g.below(NCLIP),
            _ if !on.is_empty() => g.pick(&on),
            _ => c::NONE,
        };
        m.w32(CTL + 8 + 4 * k, cur);
    }
}

#[test]
fn the_controller_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x203a40u32, 0x203ae0, 0x203b00, 0x203b48, 0x203b60, 0x203b98, 0x203bd8, 0x203c20, 0x203cb0, 0x203cf8, 0x203d38,
        0x159a80, 0x159b08, 0x159b50, 0x159ba8, 0x159bb8, 0x159d88, 0x159e38, 0x159e80, 0x159ee8, 0x159ff8, 0x15a030,
        0x15a330, 0x15a3a8, 0x15a600, 0x15a658, 0x15a690, 0x15a6c8, 0x15a700, 0x15a760, 0x2cc808, 0x161340, 0x161348,
        0x161350, 0x161358, 0x15ed68, 0x15ee30, 0x15eec0, 0x15eef8, 0x15ef48, 0x15ef98, 0x15f020, 0x15f050, 0x15f060,
        0x15f300, 0x15f390, 0x15f440, 0x15f5b8, 0x15f700, 0x15fb08, 0x15fbb8, 0x15fcf8, 0x15fdd0, 0x15fde0, 0x15fe70,
        0x15fec8, 0x15ff40, 0x15ffb8, 0x160008, 0x161360, 0x1613a0, 0x161400, 0x161528, 0x161568, 0x1615a8, 0x1615e8,
        0x161640,
    ] {
        r.allow(f);
    }
    let world = Rc::new(RefCell::new(World::default()));
    // (the stand-ins: their integer arguments, whether f12 is logged too)
    let mut stand_ins = vec![
        (a::DELETE, 1usize, false),
        (a::SKEL_ANIM_CREATE, 2, false),
        (a::POOL_ALLOC, 6, false),
        (c::RESOLVE, 2, false),
        (c::GET_CLIP_ID, 2, false),
        (c::GET_MARKER_FRAME, 3, false),
        (c::SET_MARKER, 4, false),
        (c::GET_MARKER_PHASE, 3, false),
        (c::MARKER_SOUND, 3, false),
        (c::CLIP_STARTED, 2, false),
        (c::IDS_INIT, 0, false),
        (c::BLEND_POSE, 1, false),
    ];
    for f in &c::MODES[4..] {
        stand_ins.push((*f, 2, true));
    }
    for (f, n, fl) in stand_ins {
        let w = world.clone();
        r.stub(f, move |cpu, _| {
            let regs = [4usize, 5, 6, 7, 8, 9];
            let mut args: Vec<u32> = regs[..n].iter().map(|&k| cpu.gpr(k) as u32).collect();
            if fl {
                args.push(cpu.f(12).to_bits());
            }
            let mut w = w.borrow_mut();
            w.log.push(Call(f, args.clone()));
            let v = answer(&mut w, f, &args);
            if f == c::GET_MARKER_PHASE {
                cpu.set_f(0, f32::from_bits(v));
            } else {
                cpu.set_gpr(2, v as i32 as i64 as u64);
            }
        });
    }
    let mut consts = Vec::new();
    for (at, len) in [(0x379018u32, 0x38u32), (0x39e468, 0x40), (c::CLIP_TABLE, 0x1c * NCLIP)] {
        let mut b = vec![0u8; len as usize];
        r.mem.read(at, &mut b).unwrap();
        consts.push((at, b));
    }
    let none = u32::from_le_bytes(consts[1].1[0..4].try_into().unwrap());
    let word = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut t = Table { by_action: vec![Vec::new(); 9], by_mode: vec![Vec::new(); 13], by_layer: vec![Vec::new(); 3] };
    for clip in 0..NCLIP {
        let o = (0x1c * clip) as usize;
        let (mode, action, layer) = (word(&consts[2].1, o + 8), word(&consts[2].1, o + 0xc), word(&consts[2].1, o + 0x10));
        t.by_action[action.min(8) as usize].push(clip);
        t.by_mode[mode.min(12) as usize].push(clip);
        t.by_layer[layer.min(2) as usize].push(clip);
    }
    let mut g = Rng(0x15f0_0000_0000_0001);
    let mut counts = [0usize; 23];
    for case in 0..40_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (KEYS, vec![0; 4 * NKEY as usize])] };
        m.regions.extend(consts.iter().cloned());
        m.regions.push((c::IDS_DONE, vec![0; 4]));
        m.regions.push((c::AUDIO, vec![0; 4]));
        make_world(&mut g, &mut m, none, &t);
        m.w32(c::IDS_DONE, g.below(2));
        m.w32(c::AUDIO, g.next());
        for (at, b) in m.regions.iter().filter(|(at, _)| ![0x379018, 0x39e468, c::CLIP_TABLE].contains(at)) {
            r.mem.write(*at, b).unwrap();
        }
        let k = g.below(3);
        let seq = SEQS + 0x90 * g.below(NSEQ);
        let kind = g.below(23) as usize;
        world.borrow_mut().log.clear();
        world.borrow_mut().pool_next = 0;
        let mut ours = Ours(World::default());
        let env = &mut ours;
        let mut f12 = None;
        enum Ans {
            No,
            W(u32),
            B(bool),
            Fl(F),
        }
        let (f, args, mine) = match kind {
            0 => {
                // a fresh controller over random bytes, its layers' ends made by the constructor
                let ctl = POOL_AT + 0x800;
                let ans = c::construct(&mut m, env, ctl);
                (0x15ed68, vec![ctl], Ans::W(ans))
            }
            1 => {
                c::reset(&mut m, env, CTL);
                (0x15ee30, vec![CTL], Ans::No)
            }
            2 => (0x15eec0, vec![CTL, k], Ans::W(c::clip_type(&m, CTL, k))),
            3 => {
                let v = g.f(-3.0, 3.0);
                f12 = Some(v);
                c::turn_all(&mut m, CTL, F(v));
                (0x15eef8, vec![CTL], Ans::No)
            }
            4 => {
                let v = g.next();
                c::set_stance_all(&mut m, CTL, v);
                (0x15ef48, vec![CTL, v], Ans::No)
            }
            5 => {
                let cat = if g.below(5) == 0 { 0x1c } else { g.below(0x1e) };
                c::stop_category(&mut m, CTL, cat);
                (0x15ef98, vec![CTL, cat], Ans::No)
            }
            6 => {
                c::clear_clips(&mut m, env, CTL);
                (0x15f020, vec![CTL], Ans::No)
            }
            7 => {
                c::blend_out_all(&mut m, CTL);
                (0x15f050, vec![CTL], Ans::No)
            }
            8 => {
                // a fresh sequence begun on a layer
                let s = a::seq_create(&mut m, env);
                env.0.log.clear();
                env.0.pool_next = 0;
                for (at, b) in &m.regions[..1] {
                    r.mem.write(*at, b).unwrap();
                }
                let clip = some_clip(&mut g, &t);
                let (frame, marker) = (g.below(12).wrapping_sub(4), g.below(8));
                let l = c::layer(CTL, g.below(3));
                let ans = c::begin_clip(&mut m, env, CTL, s, l, clip, frame, marker);
                (0x15f060, vec![CTL, s, l, clip, frame, marker], Ans::W(ans))
            }
            9 | 10 => {
                let dt = [1.0, 2.0, 0.5, g.f(0.0, 3.0)][g.below(4) as usize];
                f12 = Some(dt);
                if kind == 9 {
                    c::update_clip(&mut m, env, CTL, seq, F(dt));
                    (0x15f440, vec![CTL, seq], Ans::No)
                } else {
                    c::update_layers(&mut m, env, CTL, F(dt));
                    (0x15f390, vec![CTL], Ans::No)
                }
            }
            11 => {
                let clip = some_clip(&mut g, &t);
                let (frame, marker) = (g.below(12).wrapping_sub(4), g.below(8));
                let ans = c::play_clip(&mut m, env, CTL, clip, frame, marker);
                (0x15fcf8, vec![CTL, clip, frame, marker], Ans::W(ans))
            }
            12 => {
                c::handle_end(&mut m, env, CTL, seq);
                (0x15f5b8, vec![CTL, seq], Ans::No)
            }
            13 => {
                c::flush_ended(&mut m, env, CTL);
                (0x15f700, vec![CTL], Ans::No)
            }
            14 => {
                let clip = g.below(NCLIP);
                (0x15fb08, vec![CTL, clip], Ans::W(c::first_marker_frame(&mut m, env, CTL, clip) as u32))
            }
            15 => {
                let clip = g.below(NCLIP);
                let frame = if g.below(3) == 0 { u32::MAX } else { g.below(50) };
                c::set_clip_marker(&mut m, env, CTL, clip, frame);
                (0x15fbb8, vec![CTL, clip, frame], Ans::No)
            }
            16 => (0x15fdd0, vec![CTL, k], Ans::W(c::clip(&m, CTL, k))),
            17 => {
                let n = g.below(5);
                (0x15fde0, vec![CTL, n, k], Ans::Fl(c::marker_phase(&mut m, env, CTL, n, k)))
            }
            18 => (0x15fe70, vec![CTL, k], Ans::Fl(c::layer_phase(&m, CTL, k))),
            19 => (0x15fec8, vec![CTL, k], Ans::Fl(c::clip_duration(&m, CTL, k))),
            20 => (0x15ff40, vec![CTL, k], Ans::B(c::at_end(&m, CTL, k))),
            _ => {
                if a::layer_find(&m, c::layer(CTL, k), c::clip(&m, CTL, k)) == 0 {
                    continue;
                }
                let flag = 1 + g.below(8);
                if kind == 21 {
                    (0x15ffb8, vec![CTL, flag, k], Ans::B(c::test_flag(&m, CTL, flag, k)))
                } else {
                    (0x160008, vec![CTL, flag, k], Ans::B(c::test_bit(&m, CTL, flag, k)))
                }
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        if let Some(v) = f12 {
            r.cpu.set_f(12, v);
        }
        r.call(f).unwrap_or_else(|e| panic!("case {case}: {f:#x}: {e}"));
        match mine {
            Ans::No => {}
            Ans::W(v) => assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {f:#x} answer"),
            Ans::B(v) => assert_eq!(r.cpu.gpr(2) as u32, v as u32, "case {case}: {f:#x} answer"),
            Ans::Fl(v) => assert_eq!(r.cpu.f(0).to_bits(), v.0.to_bits(), "case {case}: {f:#x} answer"),
        }
        assert_eq!(world.borrow().log, ours.0.log, "case {case}: {f:#x} calls");
        for (at, b) in m.regions.iter().filter(|(at, _)| ![0x379018, 0x39e468, c::CLIP_TABLE].contains(at)) {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {f:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}");
}
