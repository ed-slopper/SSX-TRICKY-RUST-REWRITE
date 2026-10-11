//! The courses and modes against the game (board row C0): the course table, the World Circuit's order and medal
//! thresholds read from the player's SLUS_203.26 (TRICKY_ELF; skipped without it) as the runner loads it, and
//! `Circuit_GetMedal` 0x16baa8 and `cRaceHandler::Enter` 0x113740 run for every course and mode, against
//! `tricky_game::courses`.

use r5900::Runner;
use tricky_game::courses::{course_unlocked, medal, time_limit, Unlocks, CIRCUIT, COURSES, MEDALS, VENUES};

fn cstr(m: &mut r5900::Mem, at: u32) -> String {
    let mut v = Vec::new();
    let mut a = at;
    loop {
        let mut b = [0u8];
        m.read(a, &mut b);
        if b[0] == 0 {
            return String::from_utf8_lossy(&v).to_string();
        }
        v.push(b[0]);
        a += 1;
    }
}

#[test]
fn course_tables_are_the_games() {
    let Some(mut r) = Runner::from_env() else { return };
    let m = &mut r.mem;
    for (i, c) in COURSES.iter().enumerate() {
        let at = 0x332f28 + 0x1c * i as u32;
        let s = |m: &mut r5900::Mem, o: u32| {
            let p = m.read_u32(at + o);
            cstr(m, p)
        };
        let rec = (m.read_u32(at), s(m, 4), s(m, 8), m.read_u32(at + 12), s(m, 16), s(m, 20), m.read_u32(at + 24));
        assert_eq!(rec, (c.number, c.name.to_string(), c.code.to_string(), c.showoff_time, c.file.to_string(), c.file.to_string(), 5), "course {i}");
    }
    // the front end closes the table
    assert_eq!((m.read_u32(0x332f28 + 0x1c * 12), m.read_u32(0x332f28 + 0x1c * 12 + 24)), (12, 0));
    for (i, c) in CIRCUIT.iter().enumerate() {
        assert_eq!(m.read_u32(0x3352f0 + 16 * i as u32), *c, "circuit event {i}");
    }
    for (i, (c, t)) in MEDALS.iter().enumerate() {
        let at = 0x335370 + 16 * i as u32;
        assert_eq!((m.read_u32(at), [m.read_u32(at + 4) as i32, m.read_u32(at + 8) as i32, m.read_u32(at + 12) as i32]), (*c, *t), "medals {i}");
    }
}


#[test]
fn medals_and_time_limits_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.allow(0x16ba60);
    let mut seed = 0x51f2_0c7a_93d4_e816u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 16) as u32
    };
    for course in 0..13u32 {
        for _ in 0..3000 {
            let points = match next() % 4 {
                0 => (next() % 1_000_000) as i32,
                1 => [25000, 24999, 40000, 55000, 249999, 250000, 500000, 800000, 799999, 150000][next() as usize % 10],
                2 => -((next() % 1000) as i32),
                _ => (next() % 120_000) as i32,
            };
            r.cpu.set_gpr(4, points as i64 as u64);
            r.cpu.set_gpr(5, course as u64);
            r.call(0x16baa8).unwrap();
            assert_eq!(r.cpu.gpr(2) as u32, medal(points, course), "course {course}, {points} points");
        }
    }
    // the race clock's limit
    r.stub(0x11f0a8, |_, _| {});
    let race = r.mem.alloc(0x200);
    for course in 0..13u32 {
        for mode in 0..8u32 {
            r.mem.write_u32(0x32de88, course);
            r.mem.write_u32(0x32de8c, mode);
            r.mem.write_u32(race + 0x88, 0);
            r.mem.write_u32(race + 0x14, 0x7777);
            r.cpu.set_gpr(4, 0);
            r.cpu.set_gpr(5, race as u64);
            r.call(0x113740).unwrap();
            assert_eq!(r.mem.read_u32(race + 0x14), time_limit(course, mode).to_bits(), "course {course}, mode {mode}");
        }
    }
}

#[test]
fn venues_and_unlocks_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let mut b = [0u8; 10];
    r.mem.read(0x3a8af0, &mut b);
    assert_eq!(b.map(|v| v as u32), VENUES);
    let career = r.mem.alloc(0x3200);
    let circ = r.mem.alloc(0x60);
    r.mem.write_u32(career + 0x3114, circ);
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 16) as u32
    };
    for _ in 0..4000 {
        let mut u = Unlocks { bits: next(), career_slot: [-1i8, 0, 1, -128, 3][next() as usize % 5], circuit: [[0; 2]; 12] };
        for c in u.circuit.iter_mut() {
            *c = [(next() % 3) as u8 * (next() % 2) as u8, (next() % 2) as u8];
        }
        r.mem.write_u32(career + 8, u.bits);
        let mut slot = [0u8; 4];
        r.mem.read(career + 0x30fc, &mut slot);
        slot[0] = u.career_slot as u8;
        r.mem.write(career + 0x30fc, &slot);
        let mut t = [0u8; 0x60];
        for (i, c) in u.circuit.iter().enumerate() {
            t[i * 4 + 0x26] = c[0];
            t[i * 4 + 0x27] = c[1];
        }
        r.mem.write(circ, &t);
        let cheat = next() % 4 == 0;
        r.mem.write_u32(0x3352a0, cheat as u32);
        for mode in 0..10u32 {
            for course in 0..12u32 {
                r.cpu.set_gpr(4, career as u64);
                r.cpu.set_gpr(5, mode as u64);
                r.cpu.set_gpr(6, course as u64);
                r.call(0x163bb8).unwrap();
                assert_eq!(r.cpu.gpr(2) as u32 != 0, course_unlocked(&u, mode, course, cheat), "mode {mode} course {course} {u:?} cheat {cheat}");
            }
        }
    }
}
