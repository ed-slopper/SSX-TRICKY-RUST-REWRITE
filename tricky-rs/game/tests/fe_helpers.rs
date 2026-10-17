//! `tricky_game::fe_helpers` (board row G9b2a) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random lists (well formed, empty, or with stray links between the nodes), things, screens and key
//! events, every helper run on both sides with the same answers from the calls out (the virtuals, the pad record,
//! the world list and the group's event), the calls logged in order, the answers and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_helpers::*;
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

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) | 1
}

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
/// the list under test (its tail at +0x10) and a second (the finished screens at +0x24 of the first)
const LIST: u32 = ARENA + 0x1000;
const LIST2: u32 = LIST + 0x24;
/// the nodes, 0x100 apart
const NODES: u32 = ARENA + 0x2000;
const N_NODES: u32 = 8;
const SCREEN: u32 = ARENA + 0x3000;
const BAR: u32 = ARENA + 0x3400;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const PADS: u32 = ARENA + 0x7400;
const V_BASE: u32 = 0x7f_0000;

fn node(k: u32) -> u32 {
    NODES + 0x100 * k
}

struct Calls {
    seed: u64,
    n: u32,
    log: Vec<(u32, Vec<u32>)>,
}
type A = Rc<RefCell<Calls>>;

fn arity(target: u32) -> usize {
    match target {
        PAD_RECORD | WORLD_ADD | GROUP_EVENT | EVENT_DTOR => 2,
        t if [8u32, 0x10, 0x28, 0x70, 0x78, 0x80].iter().any(|o| t == V_BASE + o + 4) => 2,
        _ => 1,
    }
}

fn log_call(a: &A, read: &mut dyn FnMut(u32) -> u32, target: u32, mut v: Vec<u32>) -> u32 {
    // the move's vector by value (x, y, z, w)
    if target == V_BASE + 0x70 + 4 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..4).map(|k| read(at + 4 * k)));
    }
    // the event's place is the stack's or ours
    if target == GROUP_EVENT || target == EVENT_DTOR {
        let k = if target == GROUP_EVENT { 1 } else { 0 };
        v[k] = read(v[k]);
    }
    let mut b = a.borrow_mut();
    b.log.push((target, v));
    b.n += 1;
    // (a multiplying mix: xorshift alone keeps successive answers of one call in a fixed relation)
    let mut g = Rng(mix(b.seed ^ (target as u64) << 16 ^ b.n as u64));
    g.next();
    match target {
        PAD_RECORD => PADS + 0x10 * g.below(4),
        _ => g.below(4),
    }
}

struct Ours(A);
impl FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let n = arity(target);
        let v: Vec<u32> = (0..n).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &mut |at| cell.borrow().r32(at), target, v)
    }
}

const REGIONS: [(u32, u32); 3] = [(ARENA, 0x8000), (0x337c40, 0x40), (0, 0x600)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

/// A list at `l` holding `nodes` in order (well formed)
fn build_list(r: &mut Runner, l: u32, nodes: &[u32]) {
    let t = l + 0x10;
    r.mem.write_u32(l, u32::MAX);
    r.mem.write_u32(t, u32::MAX);
    r.mem.write_u32(l + 4, l);
    r.mem.write_u32(t + 8, t);
    let mut prev = l;
    for &n in nodes {
        r.mem.write_u32(prev + 8, n);
        r.mem.write_u32(n + 4, prev);
        prev = n;
    }
    r.mem.write_u32(prev + 8, t);
    r.mem.write_u32(t + 4, prev);
}

#[test]
fn front_end_helpers_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Calls { seed: 0, n: 0, log: vec![] }));
    let mut leaves = vec![PAD_RECORD, WORLD_ADD, GROUP_EVENT, EVENT_DTOR];
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| 4 + k;
            let v: Vec<u32> = (0..arity(f)).map(|k| c.gpr(reg(k)) as u32).collect();
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &mut |at| cell.borrow_mut().read_u32(at), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut methods: Vec<u32> = vec![
        0x2abf10, 0x2abec0, 0x2abf38, 0x2abf80, 0x2ac160, 0x2abfb0, 0x2ac018, 0x2ac030, 0x2ac048, 0x2ac0c0, 0x2ac120,
        0x2ac140, 0x2ac178, 0x2ac1e8, 0x2abe60, 0x2ac348, 0x2ac3b0, 0x2acb20, 0x275a70, 0x275ac8, 0x275b10, 0x275b90,
        0x275c10, 0x275c60, 0x271868, 0x271888, 0x2718c0, 0x271848, 0x2718f0, 0x26cef8, 0x26cf28, 0x2763f0,
    ];
    methods.extend(KEY_QUERIES.iter().map(|q| q.0));
    // (the helpers call each other)
    for &f in &methods {
        r.allow(f);
    }
    let mut g = Rng(0x2abf_0000_0000_0001);
    let mut counts = vec![0u32; methods.len()];
    let mut endless = 0u32;
    for case in 0..40_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        r.mem.write(0, &[0u8; 0x600]).unwrap();
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x24, PADS);
        for off in (0..0x170).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        for k in 0..N_NODES {
            let n = node(k);
            r.mem.write_u32(n, [g.below(4), 1 << g.below(4), g.next()][g.below(3) as usize]);
            r.mem.write_u32(n + 0xc, VT);
            r.mem.write_u32(n + 0x10, g.below(2));
            r.mem.write_u32(n + 0x40, g.below(4));
            // a loose node points at itself, or at another node
            r.mem.write_u32(n + 4, [n, node(g.below(N_NODES)), node(g.below(N_NODES)), 0][g.below(4) as usize]);
            r.mem.write_u32(n + 8, [n, node(g.below(N_NODES)), node(g.below(N_NODES)), 0][g.below(4) as usize]);
            for o in [0x80u32, 0x84, 0x88] {
                r.mem.write_f32(n + o, g.below(400) as f32 - 50.0);
            }
        }
        // the lists: a well-formed list of some of the nodes, or an empty one
        let mut order: Vec<u32> = (0..N_NODES).collect();
        for k in (1..order.len()).rev() {
            order.swap(k, g.below(k as u32 + 1) as usize);
        }
        let n1 = g.below(5) as usize;
        let n2 = g.below(3) as usize;
        let in_list: Vec<u32> = order[..n1].iter().map(|&k| node(k)).collect();
        let second: Vec<u32> = order[n1..n1 + n2].iter().map(|&k| node(k)).collect();
        build_list(&mut r, LIST, &in_list);
        build_list(&mut r, LIST2, &second);
        // (both list nodes carry the test's vtable for the screens' virtuals)
        r.mem.write_u32(SCREEN + 0x48, BAR);
        r.mem.write_u32(SCREEN + 0x34, LIST);
        r.mem.write_u32(SCREEN + 0x10, g.below(2));
        r.mem.write_u32(SCREEN + 0xc, VT);
        r.mem.write_u8(SCREEN + 0x2d, g.next() as u8);
        r.mem.write_u32(BAR + 0xc, VT);
        build_list(&mut r, BAR + 0x118, &in_list);
        let which = g.below(methods.len() as u32) as usize;
        let addr = methods[which];
        counts[which] += 1;
        let pick = [node(g.below(N_NODES)), LIST, LIST + 0x10][g.below(3) as usize];
        let (a0, a1, a2, a3, t0) = match addr {
            0x2abf10 | 0x2abec0 | 0x2abf38 => (pick, 0, 0, 0, 0),
            // (an object whose vtable is at +0)
            0x2abf80 => {
                r.mem.write_u32(node(1), VT);
                (pick, node(1), 0, 0, 0)
            }
            0x2ac048 => (LIST, [0u32, 1, 2, 3, 5, u32::MAX][g.below(6) as usize], 0, 0, 0),
            0x2ac0c0 => (LIST, [g.below(4), 1 << g.below(4), 7][g.below(3) as usize], 0, 0, 0),
            // (a node not on the list)
            0x2ac120 | 0x2ac140 => (LIST, node(order[7] as u32), 0, 0, 0),
            0x2abe60 => (SCREEN, BUF, 0, 0, 0),
            0x2ac348 | 0x2ac3b0 => {
                let v = [BUF, BUF + 4][g.below(2) as usize];
                for k in 0..8 {
                    r.mem.write_u32(BUF + 4 * k, g.next());
                }
                (SCREEN, [0u32, 1, 3, 5, 6, 7, u32::MAX][g.below(7) as usize], v, g.below(100), g.below(3))
            }
            0x2acb20 => {
                for k in 0..4 {
                    r.mem.write_f32(BUF + 4 * k, g.below(400) as f32 - 50.0);
                }
                // (a position at any address: the VU loads it from the address taken down to 16)
                (node(g.below(N_NODES)), [BUF, BUF + 4][g.below(2) as usize], 0, 0, 0)
            }
            0x275b10 => (LIST, node(order[7] as u32), 0, 0, 0),
            // (the game pops only a stack with a screen on it)
            0x275b90 | 0x275c10 if in_list.is_empty() => {
                build_list(&mut r, LIST, &[node(order[7] as u32)]);
                (LIST, 0, 0, 0, 0)
            }
            0x271868 | 0x271888 | 0x2718c0 => (SCREEN, [0u32, 1, 3, 7, 8, 0xff, 0x80, 33][g.below(8) as usize], 0, 0, 0),
            0x271848 => (SCREEN, [g.below(4), 1 << g.below(4)][g.below(2) as usize], 0, 0, 0),
            0x2718f0 => (SCREEN, g.below(16), 0, 0, 0),
            0x26cef8 | 0x26cf28 => (node(g.below(N_NODES)), g.next(), 0, 0, 0),
            0x2763f0 => (SCREEN, node(g.below(N_NODES)), 0, 0, 0),
            t if KEY_QUERIES.iter().any(|q| q.0 == t) => {
                r.mem.write_u32(BUF, [0x64u32, 0xd, 0x62, 0x1b, 0x61, 0x63, 0x4800, 0x5000, 0x4b00, 0x4d00, 0x72, 0x52, 0x6c, 0x4c, 0][g.below(15) as usize]);
                (BUF, 0, 0, 0, 0)
            }
            _ => (LIST, 0, 0, 0, 0),
        };
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.n = 0;
            b.log.clear();
        }
        let mut m = snap(&mut r);
        for (k, v) in [a0, a1, a2, a3].iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.cpu.set_gpr(8, t0 as i32 as i64 as u64);
        // (a list looped with no end walks forever in the game too: no answer to compare)
        if let Err(e) = r.call(addr) {
            if e.to_string().contains("step limit") {
                endless += 1;
                continue;
            }
            panic!("case {case}: {addr:#x}: {e}");
        }
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().n = 0;
        let e = &mut Ours(a.clone());
        let mine: Option<u32> = match addr {
            0x2abf10 => Some(is_sentinel(&m, a0)),
            0x2abec0 => Some(index(&m, a0)),
            0x2abf38 => {
                unlink(&mut m, a0);
                None
            }
            0x2abf80 => Some(node_vf2(&mut m, e, a0, a1)),
            0x2ac160 => {
                reset(&mut m, a0);
                None
            }
            0x2abfb0 => Some(init(&mut m, a0)),
            0x2ac018 => Some(first(&m, a0)),
            0x2ac030 => Some(last(&m, a0)),
            0x2ac048 => Some(nth(&m, a0, a1)),
            0x2ac0c0 => Some(find_by_id(&m, a0, a1)),
            0x2ac120 => {
                push_front(&mut m, a0, a1);
                None
            }
            0x2ac140 => {
                push_back(&mut m, a0, a1);
                None
            }
            0x2ac178 => {
                delete_all(&mut m, e, a0);
                None
            }
            0x2ac1e8 => Some(count(&m, a0)),
            0x2abe60 => {
                group_vf12(&mut m, e, a0, BUF + 0x100);
                None
            }
            0x2ac348 => {
                tween_start(&mut m, a0, a1, a2);
                None
            }
            0x2ac3b0 => {
                tween(&mut m, a0, a1, a2, a3, t0);
                None
            }
            0x2acb20 => {
                place(&mut m, e, a0, a1, BUF + 0x100);
                None
            }
            0x275a70 => {
                stack_each(&mut m, e, a0);
                None
            }
            0x275ac8 => {
                stack_close(&mut m, e, a0);
                None
            }
            0x275b10 => {
                push(&mut m, e, a0, a1);
                None
            }
            0x275b90 => Some(pop(&mut m, e, a0)),
            0x275c10 => {
                finish(&mut m, e, a0);
                None
            }
            0x275c60 => {
                stack_clear(&mut m, e, a0);
                None
            }
            0x271868 => Some(has_player(&m, a0, a1)),
            0x271888 => {
                add_player(&mut m, a0, a1);
                None
            }
            0x2718c0 => {
                remove_player(&mut m, a0, a1);
                None
            }
            0x271848 => Some(find_button(&m, a0, a1)),
            0x2718f0 => {
                set_buttons(&mut m, e, a0, a1);
                None
            }
            0x26cef8 => Some(pad_connected(&mut m, e, a0)),
            0x26cf28 => {
                set_pad_word(&mut m, e, a0, a1);
                None
            }
            0x2763f0 => {
                add_widget(&mut m, e, a0, a1);
                None
            }
            t => {
                let q = KEY_QUERIES.iter().find(|q| q.0 == t).unwrap();
                Some(key_is(&m, a0, q.1))
            }
        };
        let mine_log = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine_log, "case {case}: {addr:#x} calls");
        if let Some(v) = mine {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF + 0x100..BUF + 0x200).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by function: {counts:?}; endless lists skipped: {endless}");
}
