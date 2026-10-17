//! `tricky_game::skinpart` (board row G7r2) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is one of the row's functions on fresh random memory. Parts are loaded from a header
//! and data laid out as the game's: bones, points, materials, bone ids, the two lists and batches with sub-batches
//! and layer records, by offsets the load turns into pointers. Hierarchies have parents only among earlier bones;
//! angles, offsets and parent matrices are random; LODs have parts with and without headers and material lists
//! that repeat. `memcpy`, `strncmp` and the deletes are stand-ins on both sides (the copy done for real); the sine
//! pair is the game's own. The calls, the answer and the memory are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bdrload as bl;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
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
const SIZE: u32 = 0x20000;
const PART: u32 = ARENA;
const LIST: u32 = ARENA + 0x200;
const BANKS: u32 = ARENA + 0x400;
const HDR: u32 = ARENA + 0x500;
const MATRIX: u32 = ARENA + 0x600;
const LOD: u32 = ARENA + 0x10000;
const LISTS: u32 = ARENA + 0xf40;
const DATA: u32 = ARENA + 0x2000;
const BANKMEM: u32 = ARENA + 0x6000;
const ANGLES: u32 = ARENA + 0xb000;
const OUT: u32 = ARENA + 0xc000;
/// The skin's strings (`boot` among them)
const STRINGS: (u32, u32) = (0x39e4f0, 0x50);

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
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
        _ => {
            log.push(w[0]);
            0
        }
    };
    log.push(answer);
    (answer, Call(func, log))
}
const STANDINS: [u32; 4] = [sp::MEMCPY, bl::MEM_COPY, bl::STRNCMP, bl::DELETE];

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

/// A header at `hdr` and its data at `data` (offsets into it): `nb` bones, `np` points, `nm` materials (32 bytes:
/// names, the two flags, the strength), the 8-byte list, the 12-byte matrix lists, the batches (sub-batches,
/// layer records)
fn part_data(g: &mut Rng, m: &mut Mem, hdr: u32, data: u32) {
    let (nb, np, nm, n1, n2, n3) = (g.below(6), g.below(5), g.below(4) + 1, g.below(4), g.below(4), g.below(4));
    let (bones, points, mats, ids, list, mlists, batches, subs, recs) = (0x80u32, 0x230, 0x0, 0x2e0, 0x300, 0x320, 0x360, 0x3c0, 0x430);
    m.w32(hdr + 0x14, g.pick(&[0x600u32, 0x500, 0x5f9]));
    for (o, v) in [(0x18u32, bones), (0x1c, points), (0x20, batches), (0x28, ids), (0x2c, list), (0x30, mlists)] {
        m.w32(hdr + o, v);
    }
    for (o, v) in [(0x3c, n2), (0x3e, n1), (0x40, n3), (0x42, nb), (0x44, nm), (0x46, np)] {
        m.w16(hdr + o, v as u16);
    }
    m.w8(hdr + 0x4a, g.pick(&[0u8, 1, 2]));
    for b in 0..nb {
        let at = data + bones + 0x54 * b;
        m.w16(at + 0x12, if b == 0 { 0xffff } else { g.below(b) as u16 });
        for k in 0..3 {
            wf(m, at + 0x18 + 4 * k, g.f(-20.0, 20.0));
        }
    }
    for k in 0..np * 4 {
        wf(m, data + points + 4 * k, g.f(-50.0, 50.0));
    }
    let names: [&[u8; 4]; 4] = [b"boot", b"head", b"suit", b"boo\0"];
    for k in 0..nm {
        let at = data + mats + 32 * k;
        for j in 0..5u32 {
            let nmx = names[g.below(4) as usize];
            for (i, &c) in nmx.iter().enumerate() {
                m.w8(at + 4 * j + i as u32, c);
            }
        }
        m.w8(at + 0xc, g.below(2) as u8);
        m.w8(at + 0x10, g.below(2) as u8);
        wf(m, at + 0x14, { let v = g.f(-2.0, 2.0); g.pick(&[0.0f32, 1.0, 0.5, v]) });
    }
    for k in 0..4 {
        m.w32(data + list + 8 * k + 4, 0x40 + 8 * k);
        m.w32(data + mlists + 12 * k, g.below(3));
        m.w32(data + mlists + 12 * k + 4, 0x80 + 4 * g.below(3));
        let b = data + batches + 0x14 * k;
        m.w32(b + 4, g.below(nm));
        m.w32(b + 0xc, g.below(3));
        m.w32(b + 0x10, subs + 0x18 * k);
        for s in 0..3u32 {
            let sub = data + subs + 0x18 * k + 8 * s;
            m.w32(sub, recs + 0x24 * (3 * k + s));
            m.w32(sub + 4, g.below(3));
            for r in 0..3u32 {
                let q = data + recs + 0x24 * (3 * k + s) + 12 * r;
                m.w32(q, g.below(0x100));
                m.w32(q + 4, g.below(0x40));
            }
        }
    }
    let _ = (ids, n1, n2, n3);
}

fn banks(g: &mut Rng, m: &mut Mem) {
    for k in 0..2 {
        let b = BANKS + 0x10 * k;
        m.w32(b, BANKMEM + 0x2000 * k);
        m.w32(b + 4, BANKMEM + 0x2000 * k + 0x80 * g.below(4));
        m.w32(b + 8, 0);
    }
}

#[test]
fn skin_parts_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x205018u32, 0x205068, 0x205080, 0x2054c8, 0x2054f0, 0x2055c0, 0x205670, 0x2057a0, 0x206ac0, 0x206b60, 0x206b88, 0x206c10,
        0x206d68, 0x206e30, 0x206ea0, 0x206f10,
    ];
    for f in funcs.into_iter().chain([0x203f60, 0x2044c0, 0x204598, 0x250c10]) {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in STANDINS {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (a, call) = standin(mem, func, &w);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let mut strings = vec![0u8; STRINGS.1 as usize];
    r.mem.read(STRINGS.0, &mut strings).unwrap();
    // (the low memory a part with no header reads as its header)
    let mut low = vec![0u8; 0x100];
    r.mem.read(0, &mut low).unwrap();
    let mut g = Rng(0x5ce1_1000_0000_0001);
    let (mut loaded, mut children, mut collected) = (0usize, 0usize, 0usize);
    for case in 0..16 * 900u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (STRINGS.0, strings.clone()), (0, low.clone())] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        banks(&mut g, &mut m);
        // a part loaded already: its data at DATA, its header a copy
        part_data(&mut g, &mut m, HDR, DATA);
        m.w32(PART + 0x40, BANKS);
        m.w32(PART + 0x44, BANKS + 0x10);
        // (a part loaded already: by the port's load, so its offsets are pointers)
        sp::part_load(&mut m, &mut Ours(Vec::new()), PART, HDR, DATA);
        m.w32(PART + 0x3c, g.pick(&[0u32, 0, 1]));
        m.w32(PART, ANGLES);
        m.w32(PART + 4, OUT);
        for k in 0..6 * 3 {
            wf(&mut m, ANGLES + 4 * k, g.f(-3.5, 3.5));
            wf(&mut m, ANGLES + 0x108 + 4 * k, g.f(-30.0, 30.0));
        }
        for k in 0..16 {
            wf(&mut m, MATRIX + 4 * k, g.f(-2.0, 2.0));
        }
        // the shape list for the names
        m.w32(LIST, g.below(4));
        // the LOD: its parts (the first, or a run of copies with and without headers), its material lists
        let nparts = g.pick(&[0u32, 1, 3]);
        m.w32(LOD, nparts);
        m.w32(LOD + 4, PART);
        // (all three places copies, whatever the count: a kind's slot may name one past it)
        for k in 1..4u32 {
            let p = PART + 0x48 * k;
            for o in (0..0x48).step_by(4) {
                let v = m.r32(PART + o);
                m.w32(p + o, v);
            }
            if g.below(3) == 0 {
                m.w32(p + 0xc, 0);
            }
        }
        for k in 0..17 {
            m.w32(LOD + 8 + 4 * k, g.pick(&[0xffff_ffffu32, 0, 1]));
        }
        // its material lists so far (a count of 0 to 2, materials of random halfwords, some repeating)
        m.w32(LOD + 0x50, g.below(3));
        for k in 0..2u32 {
            m.w32(LOD + 0x54 + 12 * k, g.below(3));
            m.w32(LOD + 0x58 + 12 * k, LISTS + 8 * g.below(2));
        }
        match g.below(3) {
            0 => {
                let v = m.r32(LISTS);
                m.w32(LISTS + 4, v);
            }
            1 => {
                // (the second halfword the same, the first not)
                let v = m.r16(LISTS + 2);
                m.w16(LISTS + 6, v);
            }
            _ => {}
        }
        let s = F({ let v = g.f(0.1, 3.0); g.pick(&[1.0f32, 0.5, 2.0, v]) });
        let (flags, i, parent) = (g.below(4), g.below(6), g.pick(&[0xffff_ffffu32, 0, 1, 2]));
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut o = Ours(Vec::new());
        let which = (case % 16) as usize;
        let func = funcs[which];
        let mut fregs: Vec<f32> = vec![];
        let (regs, ours): (Vec<u32>, u32) = match which {
            0 => {
                sp::part_delete(&mut m, &mut o, PART, flags);
                (vec![PART, flags], 0)
            }
            1 => {
                sp::part_reset(&mut m, PART);
                (vec![PART], 0)
            }
            2 => {
                sp::part_load(&mut m, &mut o, PART, HDR, DATA);
                (vec![PART, HDR, DATA], 0)
            }
            3 => (vec![PART, i, ARENA + 0xf00], sp::bone(&mut m, PART, i, ARENA + 0xf00)),
            4 => {
                sp::part_scale(&mut m, PART, s);
                fregs.push(s.0);
                (vec![PART], 0)
            }
            5 => {
                sp::part_resolve_names(&mut m, &mut o, PART, LIST);
                (vec![PART, LIST], 0)
            }
            6 => {
                sp::part_resolve_materials(&mut m, &mut o, PART, LIST);
                (vec![PART, LIST], 0)
            }
            7 => {
                sp::set_child_matrices(&mut m, PART, parent, MATRIX);
                (vec![PART, parent, MATRIX], 0)
            }
            8 => {
                sp::lod_add_part(&mut m, &mut o, LOD, HDR, DATA);
                (vec![LOD, HDR, DATA], 0)
            }
            9 => {
                let b = LISTS + 4 * g.below(2);
                (vec![LISTS, b], sp::material_equals(&m, LISTS, b) as u32)
            }
            10 => (vec![LOD + 0x54, LOD + 0x60], sp::material_list_equals(&m, LOD + 0x54, LOD + 0x60) as u32),
            11 => {
                sp::collect_materials(&mut m, LOD);
                (vec![LOD], 0)
            }
            12 => {
                m.w32(LOD + 0x58f4, BANKS);
                m.w32(LOD + 0x58f8, BANKS + 0x10);
                let n = g.below(4);
                sp::alloc_parts(&mut m, LOD, n);
                (vec![LOD, n], 0)
            }
            13 => {
                sp::lod_scale(&mut m, LOD, s);
                fregs.push(s.0);
                (vec![LOD], 0)
            }
            14 => {
                sp::lod_resolve_names(&mut m, &mut o, LOD, LIST);
                (vec![LOD, LIST], 0)
            }
            _ => {
                sp::lod_resolve_materials(&mut m, &mut o, LOD, LIST);
                (vec![LOD, LIST], 0)
            }
        };
        if which == 12 {
            r.mem.write_u32(LOD + 0x58f4, BANKS);
            r.mem.write_u32(LOD + 0x58f8, BANKS + 0x10);
        }
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
        let answer = r.cpu.gpr(2) as u32;
        let returns = matches!(which, 3 | 9 | 10);
        assert!(!returns || answer == ours, "case {case} {func:#x}: answer theirs {answer:#x} ours {ours:#x}");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        loaded += (matches!(which, 2 | 8)) as usize;
        children += (which == 7 && (0..6u32).any(|b| m.r32(OUT + 0x40 * b) != 0)) as usize;
        collected += (matches!(which, 8 | 11) && m.r32(LOD + 0x50) > 0) as usize;
    }
    println!("{loaded} loads, {children} hierarchies, {collected} material collections");
    assert!(loaded > 1000 && children > 300 && collected > 300);
}
