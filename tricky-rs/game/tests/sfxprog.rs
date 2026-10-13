//! `run_snow_program` (`FUN_0021ae90`, board row G10a2) against the original in the function runner
//! (TRICKY_ELF; skipped without it).

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::sfxcalc::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

const IN: [u32; 5] = [0x21a6a8, 0x21a6f0, 0x21a710, 0x21a740, 0x21a788];

fn operand(g: &mut Rng) -> i32 {
    match g.next() % 6 {
        0 => 0,
        1 => (g.next() % 20) as i32 - 10,
        2 => (g.next() % 3000) as i32 - 1500,
        3 => g.next() as i32,
        4 => 1,
        _ => (g.next() % 200) as i32,
    }
}

fn program(g: &mut Rng) -> Vec<u8> {
    let mut p = Vec::new();
    let mut depth = 0;
    let n = 1 + g.next() % 30;
    for _ in 0..n {
        let op = match g.next() % 24 {
            0 | 1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            5 => 4,
            6 => 5,
            7 => 6,
            8 => 7,
            9 => 8,
            10 => 9,
            11 => 10,
            12 => 11,
            13 => 12,
            14 => 13,
            15 => 14,
            16 => 15 + g.next() % 4,
            17 | 18 => 0x13,
            19 | 20 => 0x14,
            21 => 0x15 + g.next() % 0x30,
            22 => 0x80 + g.next() % 0x80,
            _ => 0,
        };
        match op {
            13 if depth >= 16 => continue,
            14 if depth < 1 => continue,
            15..=18 if depth < 2 => continue,
            _ => {}
        }
        p.push(op as u8);
        match op {
            0 => p.push((g.next() % 7) as u8),
            1 | 6..=9 => p.extend(operand(g).to_le_bytes()),
            2 => {
                let mut v = operand(g);
                if g.next() % 10 != 0 && v == 0 {
                    v = 100;
                }
                p.extend(v.to_le_bytes());
            }
            12 => {
                p.extend(operand(g).to_le_bytes());
                p.extend(operand(g).to_le_bytes());
            }
            13 => depth += 1,
            14 => depth -= 1,
            15..=18 => depth -= 1,
            _ => {}
        }
    }
    p.push(0x7f);
    p
}

#[test]
fn snow_program_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let oracle: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log: Rc<RefCell<Vec<(u32, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    for (k, a) in IN.iter().enumerate() {
        let (o, l) = (oracle.clone(), log.clone());
        let a = *a;
        r.stub(a, move |c, _| {
            l.borrow_mut().push((k as u32, c.gpr(4) as u32));
            let v = o.borrow_mut().remove(0);
            c.set_gpr(2, v as i32 as i64 as u64);
        });
    }
    let st = r.mem.alloc(0x100);
    let obj = r.mem.alloc(0x40);
    let out = r.mem.alloc(0x10);
    let pm = r.mem.alloc(0x400);
    let mut g = Rng(0x7a3c_91e5_2b68_d4f1);
    let (mut traps, mut done) = (0, 0);
    for case in 0..30_000 {
        let p = program(&mut g);
        for (k, b) in p.iter().enumerate() {
            r.mem.write_u8(pm + k as u32, *b);
        }
        let mut w: Vec<i32> = (0..0x14).map(|_| operand(&mut g)).collect();
        w[0] = pm as i32;
        for (k, x) in w.iter().enumerate() {
            r.mem.write_u32(st + 4 * k as u32, *x as u32);
        }
        let (v0, b0) = (operand(&mut g), operand(&mut g));
        r.mem.write_u32(out, v0 as u32);
        r.mem.write_u32(out + 4, b0 as u32);
        let inputs: Vec<u32> = (0..64).map(|_| operand(&mut g) as u32).collect();
        *oracle.borrow_mut() = inputs.clone();
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, st as u64);
        r.cpu.set_gpr(5, obj as u64);
        r.cpu.set_gpr(6, out as u64);
        r.cpu.set_gpr(7, (out + 4) as u64);
        let orig = r.call(0x21ae90);
        let orig_log = log.borrow().clone();

        let mut next = 0;
        let mut rust_log = Vec::new();
        let (mut vol, mut bend) = (v0, b0);
        let res = run_snow_program(&p, &mut w, &mut |n| {
            rust_log.push((n, obj));
            next += 1;
            inputs[next - 1] as i32
        }, &mut vol, &mut bend);
        assert_eq!(orig.is_err(), res.is_err(), "case {case} trap {p:02x?}");
        assert_eq!(orig_log, rust_log, "case {case} calls {p:02x?}");
        if res.is_err() {
            traps += 1;
            continue;
        }
        done += 1;
        for k in 1..0x14 {
            assert_eq!(r.mem.read_u32(st + 4 * k as u32) as i32, w[k], "case {case} word {k} {p:02x?}");
        }
        assert_eq!(r.mem.read_u32(out) as i32, vol, "case {case} vol {p:02x?}");
        assert_eq!(r.mem.read_u32(out + 4) as i32, bend, "case {case} bend {p:02x?}");
    }
    assert!(done > 15_000 && traps > 100, "{done} {traps}");
}
