//! `tricky_game::cml` (board row E11i) against the original in the function runner (TRICKY_ELF; skipped without
//! it): three random camera files (linked records of each kind, node and scope trees, names with right and wrong
//! hashes), every lookup's result compared.

use r5900::Runner;
use tricky_game::cml::*;
use tricky_game::pf::Mem;

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

const NAMES: [&str; 9] = ["chase far", "replay cam", "a", "Intro", "intro", "gate cam 2", "x", "manual target", "café über"];
const FILE: u32 = 0x1000;
const REC: u32 = 0x80;
const NREC: u32 = 28;

/// The game's hash, as a reference for building the files (checked against `hash_name` in the runner too)
fn hash(s: &[u8]) -> u32 {
    let mut h = 0u32;
    for &c in s {
        let v = ((h as i32) >> 7) as u32;
        h = (h << 23) ^ v ^ (c as i8 as i32 as u32);
    }
    h
}

struct At {
    b: u32,
}
impl At {
    fn file(&self, i: u32) -> u32 {
        self.b + FILE * i
    }
    fn names(&self) -> u32 {
        self.b + FILE * 3
    }
    fn name(&self, i: u32) -> u32 {
        self.names() + 0x20 * i
    }
    fn len() -> u32 {
        FILE * 3 + 0x20 * NAMES.len() as u32
    }
}

fn regions(a: &At) -> Vec<(u32, u32)> {
    vec![(FILES[0], 0x18), (a.b, At::len())]
}

fn snapshot(r: &mut Runner, a: &At) -> Mem {
    let mut m = Mem::default();
    for (at, n) in regions(a) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

/// A record's offset in a file: the records lie at 0x40 + 0x40 k
fn rec(k: u32) -> u32 {
    0x40 + REC * k
}

fn put_name(r: &mut Runner, at: u32, s: &str) {
    let mut b = s.as_bytes().to_vec();
    b.resize(16, 0);
    r.mem.write(at, &b[..16]).unwrap();
}

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for (i, n) in NAMES.iter().enumerate() {
        put_name(r, a.name(i as u32), n);
    }
    for f in 0..3 {
        let base = a.file(f);
        r.mem.write_u32(FILES[f as usize], if g.below(6) == 0 { 0 } else { base });
        for k in 0..FILE / 4 {
            r.mem.write_u32(base + 4 * k, g.next());
        }
        // the first of each kind (kind 6 the tree's root)
        for kind in 0..7 {
            let v = if g.below(5) == 0 { u32::MAX } else if kind == 6 { rec(g.below(3)) } else { rec(g.below(NREC)) };
            r.mem.write_u32(base + 4 + 4 * kind, v);
        }
        for k in 0..NREC {
            // two halves with the same layout: +0 / +4 / +0x20 links to later records' halves, +8 a B half
            for half in [0u32, 0x40] {
                let p = base + rec(k) + half;
                let link = |g: &mut Rng| if k + 1 >= NREC || g.below(3) == 0 { u32::MAX } else { rec(k + 1 + g.below(NREC - k - 1)) + 0x40 * g.below(2) };
                let l2 = link(g);
                // A halves: a binary tree (children 2k+1, 2k+2, some cut off); B halves: a list (+4 the next B half)
                let child = |g: &mut Rng, c: u32| if c < NREC && g.below(4) != 0 { rec(c) } else { u32::MAX };
                let (l0, l1) = if half == 0 {
                    (child(g, 2 * k + 1), child(g, 2 * k + 2))
                } else {
                    (u32::MAX, if k + 1 >= NREC || g.below(3) == 0 { u32::MAX } else { rec(k + 1 + g.below(NREC - k - 1)) + 0x40 })
                };
                r.mem.write_u32(p, l0);
                r.mem.write_u32(p + 4, l1);
                r.mem.write_u32(p + 8, if g.below(4) == 0 { u32::MAX } else { rec(g.below(NREC)) + 0x40 });
                let ni = g.below(NAMES.len() as u32) as usize;
                let nm = NAMES[ni];
                let h = if g.below(5) == 0 { g.next() } else { hash(nm.as_bytes()) };
                r.mem.write_u32(p + 0xc, h);
                put_name(r, p + 0x10, nm);
                // a node's hash at +0x10 overlaps the name: sometimes make it a real hash
                if g.below(3) == 0 {
                    r.mem.write_u32(p + 0x10, hash(NAMES[g.below(NAMES.len() as u32) as usize].as_bytes()));
                }
                r.mem.write_u32(p + 0x20, l2);
                put_name(r, p + 0x24, NAMES[g.below(NAMES.len() as u32) as usize]);
            }
        }
        // the records' headers 0x20 before each body: bodies are the records' +0x20
        // (NextOfType reads body - 0x20, which is the record itself)
    }
    // the name table keeps its names
    for (i, n) in NAMES.iter().enumerate() {
        put_name(r, a.name(i as u32), n);
    }
}

fn runner() -> Option<Runner> {
    Runner::from_env()
}

#[test]
fn camera_file_lookups_match_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(At::len()) };
    for f in [0x16df80, 0x16dfd0, 0x16dfe0, 0x16e098, 0x16e168, 0x16e1d8, 0x16e248, 0x16e318, 0x16e408, 0x16e468, 0x16e518, 0x16e588] {
        r.allow(f);
    }
    // strcmp (the runner lacks its PSUBW): a byte compare, the callers only test for zero
    r.stub(0x2fd008, |c, m| {
        let (mut a, mut b) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let v = loop {
            let (mut x, mut y) = ([0u8], [0u8]);
            m.read(a, &mut x).unwrap();
            m.read(b, &mut y).unwrap();
            if x[0] != y[0] {
                break x[0] as i64 - y[0] as i64;
            }
            if x[0] == 0 {
                break 0;
            }
            a += 1;
            b += 1;
        };
        c.set_gpr(2, v as u64);
    });
    r.step_limit = 2_000_000;
    let mut g = Rng(0x16df_80cc_e11b_0001);
    for case in 0..20_000u32 {
        build(&mut g, &mut r, &a);
        let m = snapshot(&mut r, &a);
        let ni = g.below(NAMES.len() as u32);
        let name = a.name(ni);
        let si = g.below(NAMES.len() as u32);
        let scope = a.name(si);
        let f = FILES[g.below(3) as usize];
        let kind = g.below(7);
        let call = |r: &mut Runner, addr: u32, args: &[u32]| -> u32 {
            for (k, v) in args.iter().enumerate() {
                r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
            }
            r.call(addr).unwrap_or_else(|e| panic!("case {case} {addr:#x}: {e}"));
            r.cpu.gpr(2) as u32
        };
        let n = [0, 1, 3, 20][g.below(4) as usize];
        assert_eq!(call(&mut r, 0x16df80, &[name, n]), hash_name(&m, name, n), "case {case} hash");
        // the tree searches don't check for a missing file (the game only asks loaded ones)
        if m.r32(f) != 0 {
            assert_eq!(call(&mut r, 0x16e168, &[f, name]), find_node(&m, f, name), "case {case} node");
            assert_eq!(call(&mut r, 0x16e1d8, &[f, name]), find_scope(&m, f, name), "case {case} scope");
        }
        assert_eq!(call(&mut r, 0x16e248, &[f, name, kind]), file_find_object(&m, f, name, kind), "case {case} object");
        assert_eq!(call(&mut r, 0x16e318, &[f, name, scope]), file_find_object_in_scope(&m, f, name, scope), "case {case} in scope");
        let first = call(&mut r, 0x16e408, &[f, kind]);
        assert_eq!(first, first_of_type(&m, f, kind), "case {case} first");
        if first != 0 && m.r32(f) != 0 {
            let mut body = first;
            for step in 0..NREC {
                // a body may lie outside the file (a random +8): stop there
                let off = body.wrapping_sub(m.r32(f));
                if off < 0x80 || off >= rec(NREC) || (off - 0x40) % REC != 0x40 {
                    break;
                }
                let nx = call(&mut r, 0x16e468, &[f, body]);
                assert_eq!(nx, next_of_type(&m, f, body), "case {case} next {step}");
                if nx == 0 {
                    break;
                }
                body = nx;
            }
        }
        assert_eq!(call(&mut r, 0x16e518, &[0, name, kind]), find_object(&m, name, kind), "case {case} cml object");
        assert_eq!(call(&mut r, 0x16e588, &[0, name, scope, kind]), find_object_in_scope(&m, name, scope, kind), "case {case} cml in scope");
    }
}
