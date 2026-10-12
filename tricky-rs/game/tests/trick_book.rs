//! `Score_TrickBookCheck` 0x1579e0 (row C15) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the player's slots, books and flags shaken, trick tables scripted (ids chosen to match often),
//! the lesson and mode gates varied; the career block, the lesson flag and the calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::career::{BookEnv, Career, CAREER_BASE, CAREER_SIZE};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

struct Env {
    mode: bool,
    player: i8,
    lesson: Option<u32>,
    table: Vec<[u32; 2]>,
    done: u32,
    msgs: u32,
}
impl BookEnv for Env {
    fn mode_allows(&self) -> bool {
        self.mode
    }
    fn player(&self) -> i8 {
        self.player
    }
    fn lesson_trick(&self) -> Option<u32> {
        self.lesson
    }
    fn trick_info(&self, rider: u32, t: u32) -> [u32; 2] {
        self.table[(rider * 0x348 / 0x1c + t) as usize]
    }
    fn lesson_done(&mut self) {
        self.done += 1;
    }
    fn book_done_message(&mut self) {
        self.msgs += 1;
    }
}

fn byte(r: &mut Runner, a: u32) -> u8 {
    (r.mem.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

#[test]
fn trick_book_check_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(app + 0x730, race);
    let gate = r.mem.alloc(0x40);
    let lesson = r.mem.alloc(0x600);
    let score = r.mem.alloc(0x300);
    let mid = r.mem.alloc(0x600);
    let rider = r.mem.alloc(0x600);
    r.mem.write_u32(score + 0x24c, mid);
    r.mem.write_u32(mid + 0x464, rider);
    let msgs: Rc<RefCell<Vec<(u32, u32, u32, u32)>>> = Rc::default();
    let m2 = msgs.clone();
    r.stub(0x157330, move |c, _| m2.borrow_mut().push((c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32)));
    let sfx: Rc<RefCell<u32>> = Rc::default();
    let s2 = sfx.clone();
    r.stub(0x2190f0, move |_, _| *s2.borrow_mut() += 1);
    r.mem.write_u32(0x343050, 0);
    // the helpers it calls run as they are (allowed before the first call: a call stopped part way would leave its
    // writes behind)
    for addr in [0x154638u32, 0x157c18, 0x1640a8, 0x164060, 0x163f18, 0x163f50, 0x166550, 0x14c2a8, 0x14c2b8, 0x164178, 0x164258, 0x1641f8] {
        r.allow(addr);
    }
    for l in include_str!("../../../ghidra/symbols.txt").lines() {
        let mut w = l.split_whitespace();
        if let (Some("G"), Some(a), Some(n)) = (w.next(), w.next(), w.next()) {
            if n.starts_with("TrickBook_") || n.starts_with("Career_Find") {
                r.allow(u32::from_str_radix(a, 16).unwrap());
            }
        }
    }
    let mut g = Rng(0x7a11_c0de_5eed_0b0b);
    let mut ours = Career::default();
    let (mut ticked, mut lessons, mut skipped) = (0, 0, 0);
    for step in 0..60_000 {
        // the career block: four slots with players, records of riders, books
        for k in 0..4usize {
            let o = 0x44 * k;
            ours.bytes[0x30fc + o] = if g.next() % 6 == 0 { 0xff } else { (g.next() % 4) as u8 };
            let rec = 0x100 + 0x1c4 * (g.next() % 12);
            ours.w32(0x3114 + o, CAREER_BASE + rec);
            ours.w32(0x310c + o, g.next() & if g.next() % 2 == 0 { 0 } else { 0x3fff_ffff });
            let book = rec as usize + 0x14;
            ours.w32(book, g.next() % 12);
            // pages are mostly complete up to a point so every page's tricks get their turn
            let upto = g.next() % 31;
            let mask = if upto >= 30 { 0x3fff_ffff } else { (1u32 << upto) - 1 };
            let noise = g.next() & g.next() & 0x3fff_ffff;
            ours.w32(book + 4, if g.next() % 2 == 0 { mask | noise } else { mask });
            ours.w32(book + 8, if g.next() % 3 == 0 { g.next() & g.next() & 0x3fff_ffff } else { 0 });
        }
        let mut env = Env {
            mode: g.next() % 5 != 0,
            player: {
                // a player with a slot (one without reads the bytes before the block's first slot)
                let have: Vec<i8> = (0..4).map(|k| ours.bytes[0x30fc + 0x44 * k] as i8).filter(|&p| p >= 0).collect();
                if g.next() % 8 == 0 || have.is_empty() { -1 } else { have[g.next() as usize % have.len()] }
            },
            lesson: if g.next() % 8 == 0 { Some(g.next() % 30) } else { None },
            table: Vec::new(),
            done: 0,
            msgs: 0,
        };
        // trick table: 12 riders x 30 entries of 0x1c bytes (the rows are 0x348 apart: 30 entries and 0x30 spare)
        let rows = 12 * 0x348 / 0x1c;
        for i in 0..rows {
            let small = g.next() % 3 != 0;
            let id = [if small { g.next() % 3 } else { g.next() }, if small { g.next() & 0x0700_0f00 | (g.next() & 0xff) } else { g.next() }];
            env.table.push(id);
            r.mem.write_u32(0x31e210 + 0x1c * i as u32, id[0]);
            r.mem.write_u32(0x31e210 + 0x1c * i as u32 + 4, id[1]);
        }
        // the gates (memory)
        let ml = g.next() % 6;
        let (gate_ptr, gate_val) = match (env.mode, ml) {
            (true, 0) => (0, 0),
            (true, 1) => (gate, 0),
            (true, _) => (gate, 10 + g.next() % 4),
            (false, _) => (gate, g.next() % 10),
        };
        let _ = ml;
        r.mem.write_u32(race + 0x40, gate_ptr);
        r.mem.write_u32(gate + 0x20, gate_val);
        // the mode gate in our env is what the memory says
        env.mode = gate_ptr == 0 || gate_val == 0 || (gate_val as i32) >= 10;
        r.mem.write_u32(race + 0x34, if env.lesson.is_some() { lesson } else { 0 });
        if let Some(t) = env.lesson {
            r.mem.write_u32(lesson + 0x4e8, t);
        }
        r.mem.write_u32(lesson + 0x64, 0);
        r.mem.write_u32(rider + 0x64, 0);
        let pb = (rider + 0x65) & !3;
        let sh = 8 * ((rider + 0x65) & 3);
        let w = r.mem.read_u32(pb);
        r.mem.write_u32(pb, w & !(0xff << sh) | (env.player as u8 as u32) << sh);
        for k in 0..CAREER_SIZE as u32 / 4 {
            let v = u32::from_le_bytes(ours.bytes[4 * k as usize..4 * k as usize + 4].try_into().unwrap());
            r.mem.write_u32(CAREER_BASE + 4 * k, v);
        }
        // a landed trick: one of the book's or random
        let id = if let (Some(lt), true) = (env.lesson, g.next() % 2 == 0) {
            let rd = ours.r32(ours.ptr(0x3114 + 0x44 * ours.find_player_slot(env.player).max(0) as usize) + 0x14);
            let e = env.table[(rd * 0x348 / 0x1c + lt) as usize];
            [e[0], e[1] ^ (g.next() & 0x00ff_00ff)]
        } else if g.next() % 4 != 0 {
            let rd = ours.r32(ours.ptr(0x3114 + 0x44 * (g.next() % 4) as usize) + 0x14);
            let e = env.table[(rd * 0x348 / 0x1c + g.next() % 30) as usize];
            [e[0], e[1] ^ (g.next() & 0x00ff_00ff)]
        } else {
            [g.next() % 3, g.next()]
        };
        msgs.borrow_mut().clear();
        *sfx.borrow_mut() = 0;
        r.cpu.set_gpr(4, score as u64);
        let idp = r.mem.alloc(8);
        r.mem.write_u32(idp, id[0]);
        r.mem.write_u32(idp + 4, id[1]);
        r.cpu.set_gpr(5, idp as u64);
        loop {
            match r.call(0x1579e0) {
                Ok(()) => break,
                Err(e) => panic!("step {step}: {e}"),
            }
        }
        let before = ours.bytes.clone();
        ours.trick_book_check(&mut env, id);
        for k in 0..CAREER_SIZE as u32 {
            assert_eq!(byte(&mut r, CAREER_BASE + k), ours.bytes[k as usize], "step {step}: career byte {k:#x} (before {:#x}, id {id:x?}, player {}, lesson {:?}, mode {})", before[k as usize], env.player, env.lesson, env.mode);
        }
        assert_eq!(r.mem.read_u32(lesson + 0x64), (env.done > 0) as u32, "step {step}: lesson done");
        assert_eq!(msgs.borrow().len() as u32, env.msgs, "step {step}: messages");
        assert_eq!(*sfx.borrow(), env.msgs, "step {step}: sounds");
        if let Some(m) = msgs.borrow().first() {
            assert_eq!(*m, (0x3f99999a, 0x11, 0, 0), "step {step}");
            ticked += 1;
        }
        lessons += env.done;
        skipped += (!env.mode || env.player < 0) as u32;
    }
    assert!(ticked > 500 && lessons > 500 && skipped > 500, "ticked {ticked} lessons {lessons} skipped {skipped}");
}
