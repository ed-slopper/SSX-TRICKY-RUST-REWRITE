//! The BANKS.INF half of `Audio_LoadInGameConfig` 0x211098 and `Audio_LoadFEConfig` 0x20f160 against
//! `tricky_data::audiocfg::{ingame_banks, fe_banks}` (board row G10h): the originals run in the function runner on
//! generated config text (TRICKY_ELF; skipped without it), everything they call but the config reader stubbed,
//! the bank loads and the swap list compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::audiocfg::{fe_banks_checked, ingame_banks_checked, BankLoad};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.next() as usize % v.len()]
    }
}

fn cread(m: &mut Mem, at: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let mut a = at;
    loop {
        let mut b = [0u8];
        m.read(a, &mut b);
        if b[0] == 0 {
            return v;
        }
        v.push(b[0]);
        a += 1;
    }
}

fn cwrite(m: &mut Mem, at: u32, s: &[u8]) {
    for (i, b) in s.iter().chain(std::iter::once(&0u8)).enumerate() {
        m.write(at + i as u32, &[*b]);
    }
}

fn jal_targets(r: &mut Runner, at: u32, len: u32) -> Vec<u32> {
    let mut out = std::collections::BTreeSet::new();
    for k in 0..len / 4 {
        let w = r.mem.read_u32(at + 4 * k);
        if w >> 26 == 3 {
            out.insert((w & 0x03ff_ffff) << 2);
        }
    }
    out.into_iter().collect()
}

/// A generated BANKS.INF: a `GLOBAL` section with a base path, then sections for the courses with the keys in
/// random order (some repeated, some unknown, some with the key names as values).
fn gen_inf(g: &mut Rng) -> Vec<u8> {
    let names = ["GARI", "SNOW", "ELYSIUM", "MESA", "MERQUER", "ALOHA", "PIPE", "UNTRACK", "MEGAPLE", "TRICK", "ALASKA", "SNOWDREAM", "MESA_TWO", "snow", "FE", "X"];
    let keys = ["MAIN", "BOARD", "BANK", "CROWD", "AUX", "TRICKY", "SWAP", "BASEPATH", "BASEPATH", "OTHER", "main", "Swap", "CHARSPCH0", "CHARSPCH1", "CHARSPCH2", "CHARSPCH12", "CHARSPCH1"];
    let vals = ["a.bnk", "dir/b.bnk", "data/x/", "MAIN", "SWAP", "BASEPATH", "", "z", "CHARSPCH1", "q.big"];
    let mut s = String::new();
    s += "[GLOBAL]\n";
    if g.next() % 8 != 0 {
        s += &format!("BASEPATH = \"{}\"\n", g.pick(&["data/audio/", "d/", "", "x/y/"]));
    } else {
        s += "BASEPATH = \"base/\"\n";
    }
    if g.next() % 3 == 0 {
        s += &format!("{} = \"{}\"\n", g.pick(&keys), g.pick(&vals));
    }
    for _ in 0..(g.next() % 6) {
        s += &format!("[{}]\n", g.pick(&names));
        for _ in 0..(g.next() % 9) {
            let k = g.pick(&keys);
            match g.next() % 12 {
                0 => s += &format!("{k}\n"),
                _ => s += &format!("{k} = \"{}\"\n", g.pick(&vals)),
            }
        }
    }
    s.into_bytes()
}

fn runner(root: u32) -> Option<(Runner, Rc<RefCell<Vec<BankLoad>>>, Rc<RefCell<Vec<u8>>>)> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x2217d8u32, 0x2217f0, 0x2218c0, 0x221988, 0x221a00, 0x221af0, 0x221b60, 0x221918, 0x221be8, 0x221c90, 0x221d58, 0x221da0, 0x221de8, 0x221e90, 0x221ef8, 0x221f70, 0x221fd8] {
        r.allow(a);
    }
    r.stub(0x2fcc98, |c, m| {
        let (a, b) = (cread(m, c.gpr(4) as u32), cread(m, c.gpr(5) as u32));
        let eq = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase());
        c.set_gpr(2, if eq { 0 } else { 1 });
    });
    r.stub(0x2fd7f0, |c, m| {
        let (h, n) = (cread(m, c.gpr(4) as u32), cread(m, c.gpr(5) as u32));
        let at = if n.is_empty() { Some(0) } else { h.windows(n.len()).position(|w| w == n.as_slice()) };
        c.set_gpr(2, at.map(|i| c.gpr(4) + i as u64).unwrap_or(0));
    });
    r.stub(0x2fd14c, |c, m| {
        let s = cread(m, c.gpr(5) as u32);
        cwrite(m, c.gpr(4) as u32, &s);
        c.set_gpr(2, c.gpr(4));
    });
    // sprintf(out, fmt, ...) for %s and %d
    r.stub(0x2fca28, |c, m| {
        let fmt = cread(m, c.gpr(5) as u32);
        let mut out = Vec::new();
        let mut arg = 6;
        let mut i = 0;
        while i < fmt.len() {
            if fmt[i] == b'%' && i + 1 < fmt.len() {
                match fmt[i + 1] {
                    b's' => out.extend(cread(m, c.gpr(arg) as u32)),
                    b'd' => out.extend((c.gpr(arg) as u32 as i32).to_string().bytes()),
                    other => panic!("format %{}", other as char),
                }
                arg += 1;
                i += 2;
            } else {
                out.push(fmt[i]);
                i += 1;
            }
        }
        cwrite(m, c.gpr(4) as u32, &out);
        c.set_gpr(2, out.len() as u64);
    });
    let loads: Rc<RefCell<Vec<BankLoad>>> = Rc::default();
    let l = loads.clone();
    r.stub(0x20a460, move |c, m| {
        assert_eq!(c.gpr(4) as u32, root);
        l.borrow_mut().push(BankLoad { slot: c.gpr(5) as u32, path: cread(m, c.gpr(6) as u32) });
    });
    let text: Rc<RefCell<Vec<u8>>> = Rc::default();
    let t = text.clone();
    let buf = r.mem.alloc(0x2000);
    r.stub(0x221668, move |c, m| {
        let path = cread(m, c.gpr(5) as u32);
        let bytes = if path.to_ascii_lowercase().ends_with(b"banks.inf") { t.borrow().clone() } else { Vec::new() };
        let rd = c.gpr(4) as u32;
        let at = if bytes.is_empty() { buf + 0x1000 } else { buf };
        assert!(bytes.len() < 0xf00);
        cwrite(m, at, &bytes);
        m.write_u32(rd, bytes.len() as u32);
        m.write_u32(rd + 4, at);
        m.write_u32(rd + 8, at);
        c.set_gpr(2, rd as u64);
    });
    Some((r, loads, text))
}

/// Stub everything the function calls that is not ours (return 0).
fn stub_rest(r: &mut Runner, entry: u32, keep: &[u32]) {
    for a in jal_targets(r, entry, 0x900) {
        if !(0x221668..0x222040).contains(&a) && ![0x2fcc98u32, 0x2fd7f0, 0x2fd14c, 0x2fca28, 0x20a460, 0x186350].contains(&a) && !keep.contains(&a) {
            r.stub(a, |c, _| c.set_gpr(2, 0));
        }
    }
    r.stub(0x221788, |_, _| {});
}

#[test]
fn ingame_banks_match_the_original() {
    let sys = 0x0040_0000u32;
    let Some((mut r, loads, text)) = runner(sys) else { return };
    let (app, game, name) = (r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(0x100));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.stub(0x186350, move |c, _| c.set_gpr(2, name as u64));
    stub_rest(&mut r, 0x211098, &[]);
    let mut g = Rng(0x2468_ace0_1357_9bdf);
    let courses = ["gari", "snow", "Elysium", "mesa", "pipe", "trick", "nosuch", "", "ALASKA", "snowdream", "x", "MESA_TWO"];
    let mut seen = (0, 0, 0);
    let mut skipped = 0;
    for case in 0..20000 {
        let inf = gen_inf(&mut g);
        let course = courses[g.next() as usize % courses.len()];
        *text.borrow_mut() = inf.clone();
        cwrite(&mut r.mem, name, course.as_bytes());
        loads.borrow_mut().clear();
        r.cpu.set_gpr(4, sys as u64);
        r.mem.write_u32(sys + 0x38dc, 0x7777);
        r.call(0x211098).unwrap_or_else(|e| panic!("case {case}: {e}\n{}", String::from_utf8_lossy(&inf)));
        let theirs = loads.borrow().clone();
        let (ours, stale) = ingame_banks_checked(&inf, course.as_bytes());
        if stale {
            skipped += 1;
            continue;
        }
        let n = r.mem.read_u32(sys + 0x38dc) as usize;
        let swaps: Vec<Vec<u8>> = (0..n).map(|i| cread(&mut r.mem, sys + 0x38e0 + 0x40 * i as u32)).collect();
        assert_eq!(theirs, ours.loads, "case {case} course {course:?}\n{}", String::from_utf8_lossy(&inf));
        assert_eq!(swaps, ours.swaps, "case {case} swaps course {course:?}\n{}", String::from_utf8_lossy(&inf));
        seen.0 += (!ours.loads.is_empty()) as u32;
        seen.1 += (!ours.swaps.is_empty()) as u32;
        seen.2 += (ours.loads.len() > 3) as u32;
    }
    eprintln!("ingame cases with loads / swaps / more than 3 loads: {seen:?}, {skipped} on the game's stale stack skipped");
    assert!(seen.0 > 500 && seen.1 > 100 && seen.2 > 50, "{seen:?}");
}

#[test]
fn fe_banks_match_the_original() {
    let sys = 0x0040_0000u32;
    let Some((mut r, loads, text)) = runner(sys) else { return };
    stub_rest(&mut r, 0x20f160, &[]);
    let mut g = Rng(0x1f2e_3d4c_5b6a_7988);
    let mut seen = (0, 0);
    let mut skipped = 0;
    for case in 0..20000 {
        let inf = gen_inf(&mut g);
        let speech = [0, 1, 2, 12, 1, 1][g.next() as usize % 6];
        *text.borrow_mut() = inf.clone();
        loads.borrow_mut().clear();
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, speech as u64);
        r.call(0x20f160).unwrap_or_else(|e| panic!("case {case}: {e}\n{}", String::from_utf8_lossy(&inf)));
        let theirs = loads.borrow().clone();
        let (ours, stale) = fe_banks_checked(&inf, speech);
        if stale {
            skipped += 1;
            continue;
        }
        assert_eq!(theirs, ours, "case {case} speech {speech}\n{}", String::from_utf8_lossy(&inf));
        seen.0 += (!ours.is_empty()) as u32;
        seen.1 += ours.iter().any(|l| l.slot == 7) as u32;
    }
    eprintln!("fe cases with loads / a speech bank: {seen:?}, {skipped} skipped");
    assert!(seen.0 > 300 && seen.1 > 100, "{seen:?}");
}
