//! `Audio_LoadCrowd` 0x2209c8 (with `Sfx_LoadTrickConfig`), `Audio_LoadGlobalBankList` 0x21c510 and
//! `Speech_LoadNarration` 0x230a00 against `tricky_data::audiocfg::{crowd_config, global_bank_list,
//! narration_script}` (board row G10i): the originals run in the function runner on generated config text
//! (TRICKY_ELF; skipped without it), everything they call but the config reader stubbed.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::{Mem, Runner};
use tricky_data::audiocfg::{crowd_config, global_bank_list, narration_script, CrowdSet, Narration};

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


fn base_runner() -> Option<(Runner, Rc<RefCell<Vec<u8>>>)> {
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
    r.stub(0x2fd14c, |c, m| {
        let s = cread(m, c.gpr(5) as u32);
        cwrite(m, c.gpr(4) as u32, &s);
        c.set_gpr(2, c.gpr(4));
    });
    r.stub(0x2fd260, |c, m| {
        let n = cread(m, c.gpr(4) as u32).len();
        c.set_gpr(2, n as u64);
    });
    // sscanf(s, "%d" or "%f", &n)
    r.stub(0x2fcaa0, |c, m| {
        let w = cread(m, c.gpr(4) as u32);
        let fmt = cread(m, c.gpr(5) as u32);
        let s = String::from_utf8_lossy(&w).trim_start().to_string();
        let b = s.as_bytes();
        let mut e = 0;
        if e < b.len() && (b[e] == b'-' || b[e] == b'+') {
            e += 1;
        }
        if fmt == b"%f" {
            let int = b[e..].iter().take_while(|x| x.is_ascii_digit()).count();
            let mut i = e + int;
            let mut frac = 0;
            if i < b.len() && b[i] == b'.' {
                frac = b[i + 1..].iter().take_while(|x| x.is_ascii_digit()).count();
                i += 1 + frac;
            }
            if int + frac == 0 {
                c.set_gpr(2, 0);
                return;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
                    j += 1;
                }
                let ex = b[j..].iter().take_while(|x| x.is_ascii_digit()).count();
                if ex > 0 {
                    i = j + ex;
                }
            }
            let v: f32 = s[..i].parse().unwrap();
            m.write_u32(c.gpr(6) as u32, v.to_bits());
            c.set_gpr(2, 1);
            return;
        }
        assert_eq!(fmt, b"%d");
        let d = b[e..].iter().take_while(|x| x.is_ascii_digit()).count();
        if d > 0 {
            let v: i64 = s[..e + d].parse().unwrap();
            m.write_u32(c.gpr(6) as u32, v as i32 as u32);
        }
        c.set_gpr(2, (d > 0) as u64);
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
    // allocation: fresh zeroed memory; the set / zero-fill calls do nothing more
    r.stub(0x2ccf70, |c, m| {
        let _ = m;
        c.set_gpr(2, 0);
    });
    let text: Rc<RefCell<Vec<u8>>> = Rc::default();
    let t = text.clone();
    let buf = r.mem.alloc(0x4000);
    r.stub(0x221668, move |c, m| {
        let bytes = t.borrow().clone();
        let rd = c.gpr(4) as u32;
        assert!(bytes.len() < 0x3f00);
        cwrite(m, buf, &bytes);
        m.write_u32(rd, bytes.len() as u32);
        m.write_u32(rd + 4, buf);
        m.write_u32(rd + 8, buf);
        c.set_gpr(2, rd as u64);
    });
    r.stub(0x221788, |_, _| {});
    Some((r, text))
}

fn stub_rest(r: &mut Runner, entry: u32, keep: &[u32]) {
    for a in jal_targets(r, entry, 0x600) {
        if !(0x221668..0x222040).contains(&a) && ![0x2fcc98u32, 0x2fd14c, 0x2fd260, 0x2fcaa0, 0x2fca28, 0x2ccf70].contains(&a) && !keep.contains(&a) {
            r.stub(a, |c, _| c.set_gpr(2, 0));
        }
    }
}

fn floatword(g: &mut Rng) -> &'static str {
    ["0.5", "1", "-2.5", "7", "1e2", ".5", "0", "100", "3.25"][g.next() as usize % 9]
}

fn gen_crowd(g: &mut Rng) -> Vec<u8> {
    let names = ["GARI", "SNOW", "ELYSIUM", "MESA", "TRICK", "X"];
    let kinds = ["TRICK", "FALL", "ANTICIPATE", "ATTACK", "DECAY", "MAXBEND", "OTHER", "trick", "Fall"];
    let vals = ["a.bnk", "dir/b.bnk", "", "z", "q.big", "c1"];
    let mut s = String::from("[GLOBAL]\n");
    for sec in 0..(1 + g.next() % 4) {
        if sec > 0 {
            s += &format!("[{}]\n", g.pick(&names));
        }
        for _ in 0..(g.next() % 14) {
            let k = g.pick(&kinds);
            let n = g.next() % 5;
            match k {
                "ATTACK" | "DECAY" | "MAXBEND" => match g.next() % 10 {
                    0 => s += &format!("{k}{n}\n"),
                    _ => s += &format!("{k}{n} = {}\n", floatword(g)),
                },
                _ => match g.next() % 10 {
                    0 => s += &format!("{k}{n}\n"),
                    _ => s += &format!("{k}{n} = \"{}\"\n", g.pick(&vals)),
                },
            }
        }
    }
    s.into_bytes()
}

fn read_ptr_array(r: &mut Runner, at: u32) -> Vec<Vec<u8>> {
    let n = r.mem.read_u32(at);
    assert!(n <= 5, "count {n}");
    (0..n).map(|i| { let p = r.mem.read_u32(at + 4 + 4 * i); cread(&mut r.mem, p) }).collect()
}

#[test]
fn crowd_config_matches_the_original() {
    let Some((mut r, text)) = base_runner() else { return };
    let sys = r.mem.alloc(0x8000);
    let (course_at, path_at) = (r.mem.alloc(0x100), r.mem.alloc(0x100));
    cwrite(&mut r.mem, path_at, b"data/config/crowd.inf");
    // allocation (tag, size, 0) -> fresh memory; the string copy 0x2c8068(s, 0) -> a copy
    let bump = Rc::new(RefCell::new(0u32));
    let _ = bump;
    let heap = r.mem.alloc(0x40000);
    let top = Rc::new(RefCell::new(heap));
    let t1 = top.clone();
    r.stub(0x2ccf70, move |c, m| {
        let _ = m;
        let size = (c.gpr(5) as u32 + 15) & !15;
        let p = *t1.borrow();
        *t1.borrow_mut() = p + size;
        c.set_gpr(2, p as u64);
    });
    let t2 = top.clone();
    r.stub(0x2c8068, move |c, m| {
        let s = cread(m, c.gpr(4) as u32);
        let p = *t2.borrow();
        *t2.borrow_mut() = p + ((s.len() as u32 + 16) & !15);
        cwrite(m, p, &s);
        c.set_gpr(2, p as u64);
    });
    r.stub(0x21edf0, |c, m| m.write_u32(c.gpr(4) as u32, 0));
    r.allow(0x21ed30);
    r.allow(0x220ce8);
    stub_rest(&mut r, 0x2209c8, &[0x21ed30, 0x220ce8, 0x2c8068, 0x21edf0]);
    let mut g = Rng(0x1111_2222_3333_4445);
    let courses = ["gari", "SNOW", "Elysium", "mesa", "trick", "nosuch", "x"];
    let mut seen = (0, 0, 0);
    let mut skipped = 0;
    for case in 0..20000 {
        let inf = gen_crowd(&mut g);
        let course = courses[g.next() as usize % courses.len()];
        let (ours, stale) = crowd_config(&inf, course.as_bytes());
        if stale {
            skipped += 1;
            continue;
        }
        *text.borrow_mut() = inf.clone();
        cwrite(&mut r.mem, course_at, course.as_bytes());
        *top.borrow_mut() = heap;
        for k in 0..0x2000 / 4 {
            r.mem.write_u32(sys + 0x2f00 + 4 * k, 0x7777_7777);
        }
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, path_at as u64);
        r.cpu.set_gpr(6, course_at as u64);
        r.cpu.set_gpr(7, (g.next() % 3) as u64);
        r.cpu.set_gpr(8, 3);
        r.call(0x2209c8).unwrap_or_else(|e| panic!("case {case}: {e}\n{}", String::from_utf8_lossy(&inf)));
        let theirs: Vec<CrowdSet> = (0..3u32)
            .map(|i| {
                let b = sys + 0x3730 + 0x54 * i;
                CrowdSet {
                    trick: read_ptr_array(&mut r, b),
                    fall: read_ptr_array(&mut r, b + 0x18),
                    anticipate: read_ptr_array(&mut r, b + 0x30),
                    attack: f32::from_bits(r.mem.read_u32(b + 0x48)),
                    decay: f32::from_bits(r.mem.read_u32(b + 0x4c)),
                    max_bend: f32::from_bits(r.mem.read_u32(b + 0x50)),
                }
            })
            .collect();
        for i in 0..3 {
            let (a, b) = (&theirs[i], &ours[i]);
            assert_eq!((&a.trick, &a.fall, &a.anticipate), (&b.trick, &b.fall, &b.anticipate), "case {case} set {i} course {course}\n{}", String::from_utf8_lossy(&inf));
            assert_eq!((a.attack.to_bits(), a.decay.to_bits(), a.max_bend.to_bits()), (b.attack.to_bits(), b.decay.to_bits(), b.max_bend.to_bits()), "case {case} set {i} floats\n{}", String::from_utf8_lossy(&inf));
        }
        seen.0 += ours.iter().any(|s| s.trick.len() >= 2) as u32;
        seen.1 += ours.iter().any(|s| s.attack != 0.0) as u32;
        seen.2 += ours.iter().any(|s| s.trick.len() == 5 || s.fall.len() == 5) as u32;
    }
    eprintln!("crowd cases with 2+ tricks / an attack / a full list: {seen:?}, {skipped} skipped");
    assert!(seen.0 > 300 && seen.1 > 300, "{seen:?}");
}

#[test]
fn global_bank_list_matches_the_original() {
    let Some((mut r, text)) = base_runner() else { return };
    let sys = r.mem.alloc(0x8000);
    let path_at = r.mem.alloc(0x100);
    cwrite(&mut r.mem, path_at, b"data/config/chant.inf");
    stub_rest(&mut r, 0x21c510, &[]);
    let mut g = Rng(0x7777_8888_9999_aaa1);
    let vals = ["a.bnk", "dir/b.bnk", "", "z", "q.big", "chant_1.bnk"];
    let keys = ["BANK1", "x", "B2", "MAIN", "q"];
    let mut seen = (0, 0);
    let mut skipped = 0;
    for case in 0..20000 {
        let mut s = String::new();
        for sec in 0..(g.next() % 3) {
            s += &format!("[{}]\n", if sec == 0 && g.next() % 4 != 0 { "GLOBAL" } else { g.pick(&["OTHER", "global", "GLOBAL"]) });
            for _ in 0..(g.next() % 6) {
                if g.next() % 9 == 0 {
                    s += &format!("{}\n", g.pick(&keys));
                } else {
                    s += &format!("{} = \"{}\"\n", g.pick(&keys), g.pick(&vals));
                }
            }
        }
        let inf = s.into_bytes();
        let (ours, stale) = global_bank_list(&inf);
        if stale {
            skipped += 1;
            continue;
        }
        *text.borrow_mut() = inf.clone();
        let before = g.next() % 4;
        for i in 0..before {
            cwrite(&mut r.mem, sys + 0x38e0 + 0x40 * i, b"old");
        }
        r.mem.write_u32(sys + 0x38dc, before);
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, path_at as u64);
        r.call(0x21c510).unwrap_or_else(|e| panic!("case {case}: {e}\n{}", String::from_utf8_lossy(&inf)));
        let n = r.mem.read_u32(sys + 0x38dc);
        assert_eq!(n, before + ours.len() as u32, "case {case} count\n{}", String::from_utf8_lossy(&inf));
        let theirs: Vec<Vec<u8>> = (before..n).map(|i| cread(&mut r.mem, sys + 0x38e0 + 0x40 * i)).collect();
        assert_eq!(theirs, ours, "case {case}\n{}", String::from_utf8_lossy(&inf));
        seen.0 += (!ours.is_empty()) as u32;
        seen.1 += (ours.len() > 2) as u32;
    }
    eprintln!("chant cases with entries / 3+: {seen:?}, {skipped} skipped");
    assert!(seen.0 > 1000 && seen.1 > 100, "{seen:?}");
}

fn gen_nascript(g: &mut Rng, course: &str, lang_suffix: &str) -> Vec<u8> {
    let words = ["12", "-3", "b", "B", "bb", "x", "7", "0", "100", "5x", "+4"];
    let mut s = String::new();
    let mut entries = 0;
    for _ in 0..(g.next() % 5) {
        let name = [course.to_string(), format!("{course}{lang_suffix}"), "OTHER".to_string(), course.to_lowercase()][g.next() as usize % 4].clone();
        s += &format!("[{name}]\n");
        for _ in 0..(g.next() % 5) {
            if entries >= 14 {
                break;
            }
            entries += 1;
            let id = g.pick(&["1", "23", "x7", "-5", "400", "9"]);
            if g.next() % 12 == 0 {
                s += &format!("{id}\n");
                continue;
            }
            let mut v = String::new();
            for k in 0..(g.next() % 8) {
                if k > 0 {
                    v += [",", ", ", " , ", ",\t"][g.next() as usize % 4];
                }
                v += g.pick(&words);
            }
            if g.next() % 5 == 0 {
                v += ",";
            }
            s += &format!("{id} = \"{v}\"\n");
        }
    }
    s.into_bytes()
}

#[test]
fn narration_script_matches_the_original() {
    let Some((mut r, text)) = base_runner() else { return };
    let sys = r.mem.alloc(0x9000);
    let (course_at, app) = (r.mem.alloc(0x100), r.mem.alloc(0x800));
    r.mem.write_u32(0x337c58, app);
    let heap = r.mem.alloc(0x4000);
    let top = Rc::new(RefCell::new(heap));
    let t1 = top.clone();
    r.stub(0x2ccf70, move |c, _| {
        let size = (c.gpr(5) as u32 + 15) & !15;
        let p = *t1.borrow();
        *t1.borrow_mut() = p + size;
        c.set_gpr(2, p as u64);
    });
    r.stub(0x2cc938, |c, m| {
        for k in 0..c.gpr(5) as u32 {
            m.write(c.gpr(4) as u32 + k, &[0]);
        }
    });
    stub_rest(&mut r, 0x230a00, &[0x2cc938]);
    let mut g = Rng(0x4242_4343_4444_4545);
    let suffixes = ["_ENG", "_ENG", "_GER", "_FRE", "_JAP"];
    let courses = ["GARI", "Snow", "ELYSIUM", "mesa", "X"];
    let mut seen = (0, 0, 0);
    let mut skipped = 0;
    for case in 0..20000 {
        let course = courses[g.next() as usize % courses.len()];
        let lang = g.next() as usize % 5;
        let inf = gen_nascript(&mut g, course, suffixes[lang]);
        let (ours, stale) = narration_script(&inf, course.as_bytes(), lang);
        // (the game's pool holds 15 lines; a malformed line can run the reader on past that)
        if stale || ours.len() > 15 || ours.iter().any(|e| e.words.len() > 29) {
            skipped += 1;
            continue;
        }
        *text.borrow_mut() = inf.clone();
        cwrite(&mut r.mem, course_at, course.as_bytes());
        r.mem.write_u32(app + 0x58, lang as u32);
        *top.borrow_mut() = heap;
        r.cpu.set_gpr(4, sys as u64);
        r.cpu.set_gpr(5, course_at as u64);
        r.call(0x230a00).unwrap_or_else(|e| panic!("case {case}: {e}\n{}", String::from_utf8_lossy(&inf)));
        let n = r.mem.read_u32(sys + 0x7dc4) as usize;
        assert_eq!(n, ours.len(), "case {case} count\n{}", String::from_utf8_lossy(&inf));
        let pool = r.mem.read_u32(sys + 0x7dc0);
        let theirs: Vec<Vec<u32>> = (0..n).map(|i| (0..31).map(|k| r.mem.read_u32(pool + 0x7c * i as u32 + 4 * k)).collect()).collect();
        let want: Vec<Vec<u32>> = ours
            .iter()
            .map(|e: &Narration| {
                let mut v = vec![e.id as u32];
                v.extend(e.words.iter().map(|w| *w as u32));
                v.resize(31, 0);
                v
            })
            .collect();
        for (i, (a, b)) in theirs.iter().zip(&want).enumerate() {
            let trim = |v: &Vec<u32>| { let mut v = v.clone(); while v.last() == Some(&0) { v.pop(); } v.iter().map(|x| *x as i32).collect::<Vec<i32>>() };
            assert_eq!(a, b, "case {case} entry {i} lang {lang} course {course}: game {:?} ours {:?}\n{:?}", trim(a), trim(b), String::from_utf8_lossy(&inf));
        }
        seen.0 += (n > 0) as u32;
        seen.1 += ours.iter().any(|e| e.words.contains(&-1)) as u32;
        seen.2 += (n > 2) as u32;
    }
    eprintln!("narration cases with entries / a b word / 3+ entries: {seen:?}, {skipped} skipped");
    assert!(seen.0 > 1000 && seen.1 > 300 && seen.2 > 100, "{seen:?}");
}
