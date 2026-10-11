//! The config-file reader against the original's (board row E5b): the game's `IniReader_*` functions 0x221668..
//! run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it) on every
//! `DATA/CONFIG/*.INF` file (TRICKY_CONFIG_DIR: the disc's files; skipped without it), driven through the same walk
//! as `tricky_data::ini::IniReader`: section searches, then key by key the typed read the value calls for, with
//! some keys that do not match; every answer, the token buffer and the cursor compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_data::ini::IniReader;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

/// The game's reader in the runner's memory: +0 size, +4 the text, +8 the cursor, +0xc the token buffer.
struct Game {
    r: Runner,
    rd: u32,
    text: u32,
    strs: u32,
}

impl Game {
    fn cstr(&mut self, at: u32) -> Vec<u8> {
        let mut v = Vec::new();
        let mut a = at;
        loop {
            let c = { let mut b = [0u8]; self.r.mem.read(a, &mut b); b[0] };
            if c == 0 {
                return v;
            }
            v.push(c);
            a += 1;
        }
    }
    fn put_str(&mut self, s: &[u8]) -> u32 {
        for (k, c) in s.iter().enumerate() {
            self.r.mem.write_u8(self.strs + k as u32, *c);
        }
        self.r.mem.write_u8(self.strs + s.len() as u32, 0);
        self.strs
    }
    fn call(&mut self, f: u32, a1: u32, a2: u32) -> u64 {
        self.r.cpu.set_gpr(4, self.rd as u64);
        self.r.cpu.set_gpr(5, a1 as u64);
        self.r.cpu.set_gpr(6, a2 as u64);
        self.r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e}"));
        self.r.cpu.gpr(2)
    }
    fn pos(&mut self) -> usize {
        (self.r.mem.read_u32(self.rd + 8) - self.text) as usize
    }
    fn token(&mut self) -> Vec<u8> {
        self.cstr(self.rd + 0xc)
    }
}

#[test]
fn config_reader_matches_the_original() {
    let Some(dir) = std::env::var_os("TRICKY_CONFIG_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("inf"))).collect();
    files.sort();
    assert!(!files.is_empty());
    let mut g = Rng(0x2f61_c9d0_847e_b35a);
    let mut counts = [0u32; 6];
    // besides the disc's files, a few written to reach what they do not use: `;` comments, CRLF, tabs, empty
    // sections, signs and exponents, keys in other cases
    let synthetic: [&[u8]; 3] = [
        b"; a comment\r\n[ALPHA]\r\n\tName = \"one two\"\r\n  count = -12 ; trailing\r\n\tSCALE = 2.5e1\r\n[EMPTY]\r\n[BETA]\r\n name = \"x\"\r\n",
        b"#top\n[One]\n;x\n[Two]\nA = 1\nB = \"q\"\n# mid\nC = +7\n[Three]\nD = .5\nE = 3.\n",
        b"[S1]\n\tK = \"a\"\t\n\n\n[S2] ; note\n K2 = 0\n[S3]\n",
    ];
    let scratch = std::env::temp_dir().join("tricky_ini_synthetic");
    std::fs::create_dir_all(&scratch).unwrap();
    for (k, t) in synthetic.iter().enumerate() {
        let p = scratch.join(format!("SYN{k}.INF"));
        std::fs::write(&p, t).unwrap();
        files.push(p);
    }
    for path in files {
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        for a in [0x2217d8u32, 0x2217f0, 0x2218c0, 0x221988, 0x221a00, 0x221af0, 0x221b60, 0x221918, 0x221be8, 0x221c90, 0x221d58, 0x221da0, 0x221de8, 0x221e90, 0x221ef8, 0x221f70, 0x221fd8] {
            r.allow(a);
        }
        // the C library calls answered as C answers them
        let read = |m: &mut r5900::Mem, at: u32| {
            let mut v = Vec::new();
            let mut a = at;
            loop {
                let c = { let mut b = [0u8]; m.read(a, &mut b); b[0] };
                if c == 0 {
                    return v;
                }
                v.push(c);
                a += 1;
            }
        };
        r.stub(0x2fcc98, move |c, m| {
            let (a, b) = (read(m, c.gpr(4) as u32), read(m, c.gpr(5) as u32));
            let la: Vec<u8> = a.iter().map(|x| x.to_ascii_lowercase()).collect();
            let lb: Vec<u8> = b.iter().map(|x| x.to_ascii_lowercase()).collect();
            c.set_gpr(2, if la == lb { 0 } else { 1 });
        });
        r.stub(0x2fd7f0, move |c, m| {
            let (h, n) = (read(m, c.gpr(4) as u32), read(m, c.gpr(5) as u32));
            let at = if n.is_empty() { Some(0) } else { h.windows(n.len()).position(|w| w == n.as_slice()) };
            c.set_gpr(2, at.map(|i| c.gpr(4) + i as u64).unwrap_or(0));
        });
        r.stub(0x2fd14c, move |c, m| {
            let s = read(m, c.gpr(5) as u32);
            let d = c.gpr(4) as u32;
            for (k, x) in s.iter().enumerate() {
                m.write_u8(d + k as u32, *x);
            }
            m.write_u8(d + s.len() as u32, 0);
            c.set_gpr(2, d as u64);
        });
        let scanned: Rc<RefCell<Vec<Vec<u8>>>> = Rc::default();
        let sc = scanned.clone();
        r.stub(0x2fcaa0, move |c, m| {
            // record what sscanf was given; the number is compared through our reader's own parse
            sc.borrow_mut().push(read(m, c.gpr(4) as u32));
            c.set_gpr(2, 1);
        });
        let bytes = std::fs::read(&path).unwrap();
        let rd = r.mem.alloc(0x900);
        let text = r.mem.alloc(bytes.len() as u32 + 16);
        for (k, b) in bytes.iter().enumerate() {
            r.mem.write_u8(text + k as u32, *b);
        }
        r.mem.write_u8(text + bytes.len() as u32, 0);
        let strs = r.mem.alloc(0x100);
        let out = r.mem.alloc(0x400);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // the section names, for searches
        let mut names = Vec::new();
        {
            let mut probe = IniReader::new(&bytes);
            while let Some(n) = probe.next_section() {
                names.push(n);
            }
        }
        for round in 0..40 {
            let mut game = Game { r, rd, text, strs };
            game.r.mem.write_u32(rd, bytes.len() as u32);
            game.r.mem.write_u32(rd + 4, text);
            game.r.mem.write_u32(rd + 8, text);
            game.r.mem.write_u8(rd + 0xc, 0);
            let mut ours = IniReader::new(&bytes);
            let ctx = |what: &str, round: i32| format!("{name} round {round}: {what}");
            // find a section: by name (any case), by a prefix, or the next one
            let found = match round % 3 {
                0 if !names.is_empty() => {
                    let n = if g.next() % 5 == 0 { b"NOSUCHSECTION".to_vec() } else { names[g.next() as usize % names.len()].clone() };
                    let n2: Vec<u8> = if g.next() % 2 == 0 { n.iter().map(|c| c.to_ascii_lowercase()).collect() } else { n };
                    let p = game.put_str(&n2);
                    let a = game.call(0x221af0, p, 0) != 0;
                    let b = ours.find_section(&n2);
                    assert_eq!(a, b, "{}", ctx("find section", round));
                    a
                }
                1 if !names.is_empty() => {
                    let n = names[g.next() as usize % names.len()].clone();
                    let s: Vec<u8> = match g.next() % 3 {
                        0 => n.clone(),
                        1 => n[..(n.len() / 2).max(1)].to_vec(),
                        _ => [n.as_slice(), b"_EXTRA"].concat(),
                    };
                    let p = game.put_str(&s);
                    let a = game.call(0x221b60, p, 0) != 0;
                    let b = ours.find_section_matching(&s);
                    assert_eq!(a, b, "{}", ctx("find section matching", round));
                    a
                }
                _ => {
                    let a = game.call(0x221a00, 0, 0);
                    let b = ours.next_section();
                    let ga = if a == 0 { None } else { Some(game.cstr(a as u32)) };
                    assert_eq!(ga, b, "{}", ctx("next section", round));
                    ga.is_some()
                }
            };
            assert_eq!((game.pos(), game.token()), (ours.pos(), ours.token().to_vec()), "{}", ctx("after the search", round));
            if found {
                game.call(0x221918, 0, 0);
                ours.skip_to_next_section();
                assert_eq!(game.pos(), ours.pos(), "{}", ctx("skip to next section", round));
                counts[0] += 1;
                // key by key, as a loader reads them
                for step in 0..200 {
                    let a = game.call(0x221e90, 0, 0) != 0;
                    let b = ours.at_section_end();
                    assert_eq!(a, b, "{}", ctx(&format!("at section end, step {step}"), round));
                    if a {
                        break;
                    }
                    let a = game.call(0x221c90, 0, 0);
                    let tok = ours.read_token();
                    assert_eq!(game.cstr(a as u32), tok, "{}", ctx(&format!("token, step {step}"), round));
                    // what the value is: look ahead with a copy of ours
                    let quoted = {
                        let mut l = ours.clone();
                        !l.expect(b"=") && l.read_token().first() == Some(&b'"')
                    };
                    let key = if g.next() % 6 == 0 { b"NOTAKEY".to_vec() } else { tok.clone() };
                    let p = game.put_str(&key);
                    if quoted {
                        let a = game.call(0x221ef8, p, out) != 0;
                        let b = ours.read_string(&key);
                        assert_eq!(a, b.is_some(), "{}", ctx(&format!("read string {key:?}, step {step}"), round));
                        if let Some(s) = b {
                            assert_eq!(game.cstr(out), s, "{}", ctx("the string", round));
                            counts[1] += 1;
                        }
                    } else if g.next() % 2 == 0 {
                        scanned.borrow_mut().clear();
                        let a = game.call(0x221fd8, p, out) != 0;
                        let b = ours.read_int(&key);
                        assert_eq!(a, b.is_some(), "{}", ctx(&format!("read int {key:?}, step {step}"), round));
                        if b.is_some() {
                            assert_eq!(*scanned.borrow(), vec![ours.token().to_vec()], "{}", ctx("the int's word", round));
                            counts[2] += 1;
                        }
                    } else {
                        scanned.borrow_mut().clear();
                        let a = game.call(0x221f70, p, out) != 0;
                        let b = ours.read_float(&key);
                        assert_eq!(a, b.is_some(), "{}", ctx(&format!("read float {key:?}, step {step}"), round));
                        if b.is_some() {
                            assert_eq!(*scanned.borrow(), vec![ours.token().to_vec()], "{}", ctx("the float's word", round));
                            counts[3] += 1;
                        }
                    }
                    if key != tok {
                        counts[4] += 1;
                    }
                    assert_eq!((game.pos(), game.token()), (ours.pos(), ours.token().to_vec()), "{}", ctx(&format!("after step {step}"), round));
                }
            } else {
                counts[5] += 1;
            }
            r = game.r;
        }
        println!("{name}: checked");
    }
    println!("sections, strings, ints, floats, missed keys, not found: {counts:?}");
    assert!(counts.iter().all(|&c| c > 10), "{counts:?}");
}

#[test]
fn music_config_matches_the_original() {
    // `MusicSys_Init` 0x224a98 on MUSIC.INF and `MusicSys_LoadMusicMap` 0x224cf8 on MUSICMAP.INF for every course
    // name (and some that are not), against `tricky_data::music`
    use tricky_data::music::{course_songs, music_table};
    let Some(dir) = std::env::var_os("TRICKY_CONFIG_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    let (music, map) = (std::fs::read(dir.join("MUSIC.INF")).unwrap(), std::fs::read(dir.join("MUSICMAP.INF")).unwrap());
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x2217d8u32, 0x2217f0, 0x2218c0, 0x221988, 0x221a00, 0x221af0, 0x221b60, 0x221918, 0x221be8, 0x221c90, 0x221d58, 0x221da0, 0x221de8, 0x221e90, 0x221ef8, 0x221f70, 0x221fd8, 0x225588] {
        r.allow(a);
    }
    fn read(m: &mut r5900::Mem, at: u32) -> Vec<u8> {
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
    fn write(m: &mut r5900::Mem, at: u32, s: &[u8]) {
        for (k, c) in s.iter().enumerate() {
            m.write_u8(at + k as u32, *c);
        }
        m.write_u8(at + s.len() as u32, 0);
    }
    r.stub(0x2fcc98, |c, m| {
        let (a, b) = (read(m, c.gpr(4) as u32), read(m, c.gpr(5) as u32));
        let eq = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase());
        c.set_gpr(2, if eq { 0 } else { 1 });
    });
    r.stub(0x2fd7f0, |c, m| {
        let (h, n) = (read(m, c.gpr(4) as u32), read(m, c.gpr(5) as u32));
        let at = if n.is_empty() { Some(0) } else { h.windows(n.len()).position(|w| w == n.as_slice()) };
        c.set_gpr(2, at.map(|i| c.gpr(4) + i as u64).unwrap_or(0));
    });
    r.stub(0x2fd14c, |c, m| {
        let s = read(m, c.gpr(5) as u32);
        write(m, c.gpr(4) as u32, &s);
        c.set_gpr(2, c.gpr(4));
    });
    r.stub(0x2fd260, |c, m| {
        let n = read(m, c.gpr(4) as u32).len();
        c.set_gpr(2, n as u64);
    });
    // the files: IniReader_Open answered from the disc's bytes
    let (mt, mp) = (r.mem.alloc(music.len() as u32 + 16), r.mem.alloc(map.len() as u32 + 16));
    write(&mut r.mem, mt, &music);
    write(&mut r.mem, mp, &map);
    let (ml, pl) = (music.len() as u32, map.len() as u32);
    r.stub(0x221668, move |c, m| {
        let path = read(m, c.gpr(5) as u32);
        let (buf, len) = if path.ends_with(b"musicmap.inf") { (mp, pl) } else { (mt, ml) };
        let rd = c.gpr(4) as u32;
        m.write_u32(rd, len);
        m.write_u32(rd + 4, buf);
        m.write_u32(rd + 8, buf);
        c.set_gpr(2, rd as u64);
    });
    for a in [0x221788u32, 0x229890, 0x225660, 0x224368] {
        r.stub(a, |_, _| {});
    }
    let obj = r.mem.alloc(0x3800);
    r.cpu.set_gpr(4, obj as u64);
    r.call(0x224a98).expect("MusicSys_Init");
    let t = music_table(&music);
    let m = &mut r.mem;
    assert_eq!(read(m, obj + 0x1260), t.base, "base path");
    assert_eq!((m.read_u32(obj + 0x36e0) as usize, m.read_u32(obj + 0x36ec)), (t.songs.len(), t.mask), "count, mask");
    for (i, s) in t.songs.iter().enumerate() {
        assert_eq!(read(m, obj + 0x12e0 + 0x20 * i as u32), s.name, "song {i} name");
        if let Some(p) = &s.loop_path {
            assert_eq!(&read(m, obj + 0x16e0 + 0x100 * i as u32), p, "song {i} path");
        }
    }
    println!("{} songs, base {:?}", t.songs.len(), String::from_utf8_lossy(&t.base));
    // every course: the map's own sections, the courses' names, prefixes and names that are not there
    let mut keys: Vec<Vec<u8>> = Vec::new();
    {
        let mut probe = tricky_data::ini::IniReader::new(&map);
        while let Some(n) = probe.next_section() {
            keys.push(n);
        }
    }
    for c in ["GARI", "SNOW", "ELYSIUM", "MESA", "MERQUER", "ALOHA", "PIPE", "UNTRACK", "MEGAPLE", "TRICK", "ALASKA", "NOSUCH", "", "SNOWDREAM", "snow"] {
        keys.push(c.as_bytes().to_vec());
    }
    let key_at = r.mem.alloc(0x100);
    for k in &keys {
        write(&mut r.mem, key_at, k);
        r.mem.write_u32(obj + 0x36e8, 0x1234);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, key_at as u64);
        r.call(0x224cf8).expect("MusicSys_LoadMusicMap");
        let (mask, count) = course_songs(&map, &t, k);
        let m = &mut r.mem;
        assert_eq!((m.read_u32(obj + 0x36ec), m.read_u32(obj + 0x36e4), m.read_u32(obj + 0x36e8)), (mask, count, 0xffff_ffff), "course {:?}", String::from_utf8_lossy(k));
        let names: Vec<String> = (0..t.songs.len()).filter(|i| mask & (1 << i) != 0).map(|i| String::from_utf8_lossy(&t.songs[i].name).to_string()).collect();
        println!("{:>10}: {names:?}", String::from_utf8_lossy(k));
    }
}
