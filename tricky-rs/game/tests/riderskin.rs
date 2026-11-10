//! `tricky_game::riderskin` (board row G7r3) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a skin as the game would (its banks, one or two LODs of one to four parts loaded
//! from headers and data of random kinds, bones with parents among earlier bones and attachments to the kinds
//! there, some parts drawn by a +0x48), maybe binds it, then runs one of the row's functions on both sides. The
//! copy, fill, `strncmp`, deletes and the renderer's virtuals are stand-ins on both sides (the copy and fill done
//! for real); the sine pair, `Vec4_Scale` and `Mat4_TransformVec4` are the game's own. The calls and the memory
//! are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bdrload as bl;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::riderskin as rs;
use tricky_game::skinpart as sp;
use tricky_game::textbox::TextBoxEnv;

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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x40000;
const SKIN: u32 = ARENA;
const LIST: u32 = ARENA + 0x4000;
const APPOBJ: u32 = ARENA + 0x4200;
const RENDER: u32 = ARENA + 0x4300;
const RVT: u32 = ARENA + 0x4400;
const HDRS: u32 = ARENA + 0x4800;
const DATAS: u32 = ARENA + 0x5000;
const BASE: u32 = ARENA + 0x8000;
const MATRIX: u32 = ARENA + 0x9000;
/// The port's scratch (the game's stack): not compared
const SCRATCH: u32 = 0x0180_0000;
const BANK0: u32 = ARENA + 0x10000;
const BANK1: u32 = ARENA + 0x30000;
/// The game's own memory the row touches: the skinned matrices, the bind matrix, the app, the skin's strings,
/// and the low memory a missing part's matrices are read from
const GAME: [(u32, u32); 5] = [(rs::SKINNED, 0x800), (rs::BIND_MATRIX, 0x40), (0x337c00, 0x100), (0x39e4f0, 0x50), (0, 0x1000)];
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn w8(&mut self, a: u32, v: u8);
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8; 1];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn inside(a: u32, n: u32) -> bool {
    a >= ARENA && (a as u64 + n as u64) <= (ARENA + SIZE) as u64
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32]) -> (u32, Call) {
    let mut log = Vec::new();
    let answer = match func {
        sp::MEMCPY | bl::MEM_COPY => {
            log.extend(&w[..3]);
            if inside(w[0], w[2]) && inside(w[1], w[2]) {
                let b: Vec<u8> = (0..w[2]).map(|k| io.r8(w[1] + k)).collect();
                for (k, c) in b.into_iter().enumerate() {
                    io.w8(w[0] + k as u32, c);
                }
            }
            w[0]
        }
        rs::MEMSET => {
            // (the table is on the game's stack and in the port's scratch: only its size logged)
            log.extend(&w[1..3]);
            for k in 0..w[2] {
                io.w8(w[0] + k, w[1] as u8);
            }
            0
        }
        bl::STRNCMP => {
            log.extend(&w[..3]);
            let mut d = 0i32;
            for k in 0..w[2] {
                let (a, b) = (io.r8(w[0] + k), io.r8(w[1] + k));
                if a != b {
                    d = a as i32 - b as i32;
                    break;
                }
                if a == 0 {
                    break;
                }
            }
            d as u32
        }
        f if f == rv(0x310) => {
            log.extend([w[0], w[1]]);
            log.extend((0..32).map(|k| io.r32(w[2] + 4 * k)));
            log.extend([w[3], w[4], w[5]]);
            0
        }
        f if f == rv(0x2e0) || f == rv(0x2e8) => {
            log.extend(&w[..3]);
            0
        }
        _ => {
            log.push(w[0]);
            0
        }
    };
    log.push(answer);
    (answer, Call(func, log))
}
const STANDINS: [u32; 6] = [sp::MEMCPY, bl::MEM_COPY, bl::STRNCMP, bl::DELETE, rs::MEMSET, bl::FREE];

struct Ours(Vec<Call>);
impl TextBoxEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], _fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, func, &w);
        self.0.push(c);
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// A header and its data (materials first, bones, points, lists, batches) of a part of kind `kind`
fn part_data(g: &mut Rng, m: &mut Mem, hdr: u32, data: u32, kind: u8, kinds: u32) {
    let (nb, np, nm) = (g.below(5) + 1, g.below(4), g.below(3) + 1);
    let (bones, points, mats, ids, list, mlists, batches) = (0x80u32, 0x230, 0x0, 0x2e0, 0x300, 0x320, 0x360);
    m.w32(hdr + 0x14, 0x400);
    for (o, v) in [(0x18u32, bones), (0x1c, points), (0x20, batches), (0x28, ids), (0x2c, list), (0x30, mlists)] {
        m.w32(hdr + o, v);
    }
    for (o, v) in [(0x3c, g.below(3)), (0x3e, g.below(3)), (0x40, 0), (0x42, nb), (0x44, nm), (0x46, np)] {
        m.w16(hdr + o, v as u16);
    }
    m.w16(hdr + 0x48, g.pick(&[0u16, 0, 0, 3]));
    m.w8(hdr + 0x4a, kind);
    for b in 0..nb {
        let at = data + bones + 0x54 * b;
        m.w16(at + 0x12, if b == 0 { 0xffff } else { g.below(b) as u16 });
        // attached to a kind there (or none; never the root, whose parent is -1), its parent among the bones before
        m.w16(at + 0x10, if b == 0 || g.below(2) == 0 { 0xffff } else { g.below(kinds) as u16 });
        for k in 0..3 {
            wf(m, at + 0x18 + 4 * k, g.f(-20.0, 20.0));
            wf(m, at + 0x24 + 4 * k, g.f(-3.5, 3.5));
        }
    }
    for k in 0..np * 4 {
        wf(m, data + points + 4 * k, g.f(-50.0, 50.0));
    }
    let names: [&[u8; 4]; 3] = [b"boot", b"head", b"suit"];
    for k in 0..nm {
        let at = data + mats + 32 * k;
        for j in 0..5u32 {
            let nmx = names[g.below(3) as usize];
            for (i, &c) in nmx.iter().enumerate() {
                m.w8(at + 4 * j + i as u32, c);
            }
        }
        wf(m, at + 0x14, g.f(-2.0, 2.0));
    }
    for k in 0..3 {
        m.w32(data + mlists + 12 * k, g.below(2));
        m.w32(data + mlists + 12 * k + 4, 0x40 + 4 * k);
        m.w32(data + list + 8 * k + 4, 0x40);
    }
}

#[test]
fn rider_skin_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [0x206f80u32, 0x207040, 0x2070a8, 0x2070e8, 0x207198, 0x207b58, 0x207e50, 0x208010, 0x208220, 0x2082b8, 0x208328, 0x2083d8, 0x2083e8];
    for f in funcs.into_iter().chain([0x203e88, 0x203f08, 0x203f60, 0x203fa0, 0x205068, 0x2057a0, 0x206e30, 0x2054f0, 0x206ea0, 0x2055c0, 0x206f10, 0x205670, 0x2044c0, 0x204598, 0x250c10, 0x102f50, 0x178078]) {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in STANDINS.into_iter().chain([rv(0x310), rv(0x2e0), rv(0x2e8)]) {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (a, call) = standin(mem, func, &w);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let game: Vec<Vec<u8>> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            b
        })
        .collect();
    let mut g = Rng(0x5ce1_2000_0000_0001);
    let (mut bound, mut attached, mut fe) = (0usize, 0usize, 0usize);
    for case in 0..13 * 500u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (SCRATCH, vec![0; 0x80])] };
        for (k, &(at, _)) in GAME.iter().enumerate() {
            m.regions.push((at, game[k].clone()));
        }
        // (the banks too: what the skin leaves alone must show; and the bind matrix, else the inverse pose is all 0)
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..16 {
            wf(&mut m, rs::BIND_MATRIX + 4 * k, g.f(-2.0, 2.0));
        }
        // the renderer
        m.w32(0x337c58, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in [0x2e0u32, 0x2e8, 0x310] {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for k in 0..16 {
            wf(&mut m, MATRIX + 4 * k, g.f(-2.0, 2.0));
        }
        for k in 0..0x200 {
            wf(&mut m, BASE + 4 * k, g.f(-2.0, 2.0));
        }
        m.w32(LIST, g.below(4));
        // the skin, built by the port's own functions
        let mut setup = Ours(Vec::new());
        rs::init(&mut m, SKIN);
        m.w32(SKIN, BANK0);
        m.w32(SKIN + 0x10, BANK1);
        bl::align(&mut m, SKIN);
        bl::align(&mut m, SKIN + 0x10);
        let nl = g.pick(&[1u32, 1, 2]);
        rs::alloc_lods(&mut m, SKIN, nl);
        let mut slot = 0u32;
        for li in 0..nl {
            let lod = m.r32(SKIN + 0x3f18) + 0x5900 * li;
            let np = g.below(4) + 1;
            sp::alloc_parts(&mut m, lod, np);
            for _ in 0..np {
                let (hdr, data) = (HDRS + 0x60 * slot, DATAS + 0x400 * (slot % 8));
                slot += 1;
                let kind = g.below(3) as u8;
                part_data(&mut g, &mut m, hdr, data, kind, 3);
                sp::lod_add_part(&mut m, &mut setup, lod, hdr, data);
            }
        }
        // some parts scaled already
        for li in 0..nl {
            let lod = m.r32(SKIN + 0x3f18) + 0x5900 * li;
            for pi in 0..m.r32(lod) {
                if g.below(4) == 0 {
                    m.w32(m.r32(lod + 4) + 0x48 * pi + 0x3c, 1);
                }
            }
        }
        let which = (case % 13) as usize;
        let s = F({ let v = g.f(0.2, 3.0); g.pick(&[1.0f32, 0.5, v]) });
        if which != 4 || g.below(2) == 0 {
            rs::bind(&mut m, SKIN, BASE, s);
        }
        let (flags, n, li) = (g.below(4), g.below(3), g.below(nl));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut o = Ours(Vec::new());
        let func = funcs[which];
        let mut fregs: Vec<f32> = vec![];
        let regs: Vec<u32> = match which {
            0 => {
                rs::init(&mut m, SKIN);
                vec![SKIN]
            }
            1 => {
                rs::delete(&mut m, &mut o, SKIN, flags);
                vec![SKIN, flags]
            }
            2 => {
                rs::reset(&mut m, SKIN);
                vec![SKIN]
            }
            3 => {
                rs::clear_parts(&mut m, SKIN);
                vec![SKIN]
            }
            4 => {
                rs::bind(&mut m, SKIN, BASE, s);
                fregs.push(s.0);
                vec![SKIN, BASE]
            }
            5 => {
                rs::alloc_lods(&mut m, SKIN, n);
                vec![SKIN, n]
            }
            6 => {
                rs::draw_lod(&mut m, &mut o, SKIN, li);
                vec![SKIN, li]
            }
            7 => {
                let skip = g.pick(&[0u32, 0, 1]);
                rs::skin_matrices_fe(&mut m, &mut o, SKIN, skip, SCRATCH);
                vec![SKIN, skip]
            }
            8 => {
                rs::scale(&mut m, SKIN, s);
                fregs.push(s.0);
                vec![SKIN]
            }
            9 => {
                rs::resolve_names(&mut m, &mut o, SKIN, LIST);
                vec![SKIN, LIST]
            }
            10 => {
                rs::resolve_materials(&mut m, &mut o, SKIN, LIST);
                vec![SKIN, LIST]
            }
            11 => {
                rs::set_root_matrix(&mut m, SKIN, MATRIX);
                vec![SKIN, MATRIX]
            }
            _ => {
                rs::set_skin_child_matrices(&mut m, SKIN, MATRIX);
                vec![SKIN, MATRIX]
            }
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        for (k, v) in fregs.into_iter().enumerate() {
            r.cpu.set_f(12 + k, v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != o.0 {
            let i = theirs.iter().zip(&o.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.0.len()));
            let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(12)])).unwrap_or_default();
            panic!("case {case} {func:#x}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.0.len(), show(theirs.get(i)), show(o.0.get(i)));
        }
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        bound += (which == 4) as usize;
        attached += (which == 4 && m.r32(SKIN + 0x20) > 1) as usize;
        fe += (which == 7 && o.0.iter().any(|c| c.0 == rv(0x310))) as usize;
    }
    println!("{bound} binds, {attached} with two entries or more, {fe} front-end skinnings");
    assert!(bound > 400 && attached > 200 && fe > 200);
}
