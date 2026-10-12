//! The career block (board row E12) against the game in the function runner (TRICKY_ELF; skipped without it):
//! `Career_InitNewGame` 0x1617a0 (with `Profile_ResetNewGame`, `Career_DefaultPlayerName`,
//! `Career_InitCharacterRecord`, `HighScore_Reset` on the effects stream, `RelTable_Init` and the audio settings) on
//! blocks of random bytes in every language, with no sound manager and with one, against `tricky_game::career`;
//! every byte of the block, the random stream and the sound calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::career::{AudioSettings, Career, CAREER_SIZE};
use tricky_game::rand::{Rand, Streams};

const FX_STATE: u32 = 0x345e58;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Vol(u32, u32),
    Speech(u32),
    Mode(u32),
}

struct Snd {
    vols: [f32; 5],
    speech: f32,
    mode: u32,
    log: Vec<Ev>,
}

impl AudioSettings for Snd {
    fn volume(&self, ch: u32) -> f32 {
        self.vols[ch as usize]
    }
    fn speech(&self) -> f32 {
        self.speech
    }
    fn mode(&self) -> u32 {
        self.mode
    }
    fn set_volume(&mut self, ch: u32, v: f32) {
        self.log.push(Ev::Vol(ch, v.to_bits()));
    }
    fn set_speech(&mut self, v: f32) {
        self.log.push(Ev::Speech(v.to_bits()));
    }
    fn set_mode(&mut self, m: u32) {
        self.log.push(Ev::Mode(m));
    }
}

#[test]
fn new_game_is_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x1617a0u32, 0x161b40, 0x2cc808, 0x163358, 0x166600, 0x1668f0, 0x23d4a8, 0x2401e0, 0x165660, 0x16b298, 0x167da0, 0x167208, 0x166c70, 0x161928, 0x161a98, 0x166a58, 0x2fca28] {
        r.allow(a);
    }
    let reltab = r.mem.alloc(0x900);
    r.stub(0x2ccf70, move |c, _| c.set_gpr(2, reltab as u64));
    // sprintf's newlib internals run as they are
    let snd_state: Rc<RefCell<([f32; 5], u32)>> = Rc::new(RefCell::new(([0.0; 5], 0)));
    let log: Rc<RefCell<Vec<Ev>>> = Rc::new(RefCell::new(vec![]));
    let s = snd_state.clone();
    r.stub(0x211f20, move |c, _| c.set_f(0, s.borrow().0[c.gpr(5) as usize]));
    let s = snd_state.clone();
    r.stub(0x21e688, move |c, _| c.set_gpr(2, s.borrow().1 as u64));
    let l = log.clone();
    r.stub(0x211db0, move |c, _| l.borrow_mut().push(Ev::Vol(c.gpr(5) as u32, c.f(12).to_bits())));
    let l = log.clone();
    r.stub(0x212210, move |c, _| l.borrow_mut().push(Ev::Speech(c.f(12).to_bits())));
    let l = log.clone();
    r.stub(0x212270, move |c, _| l.borrow_mut().push(Ev::Mode(c.gpr(5) as u32)));
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let snd = r.mem.alloc(0x3900);
    let career = 0x0080_0000u32;
    let mut g = Rng(0xca2e_e200_0000_0001);
    for case in 0..24 {
        let language = case % 6;
        let with_sound = case % 2 == 1;
        // a block of garbage
        let mut ours = Career::default();
        for b in ours.bytes.iter_mut() {
            *b = g.next() as u8;
        }
        let mut streams = Streams::default();
        streams.seed_fx(g.next());
        let vols = [0.25f32, 0.5, (g.next() % 100) as f32 / 100.0, 1.0, 0.75];
        let speech = (g.next() % 100) as f32 / 100.0;
        let mode = g.next() % 3;
        let fresh_table = case % 3 == 0;
        // newlib's internals behind sprintf are allowed as met; each try starts from the same state
        loop {
            for (k, b) in ours.bytes.iter().enumerate() {
                r.mem.write_u8(career + k as u32, *b);
            }
            r.mem.write_u32(app + 0x58, language);
            r.mem.write_u32(0x3352b4, if fresh_table { 0 } else { reltab });
            for (k, w) in streams.fx.s.iter().enumerate() {
                r.mem.write_u32(FX_STATE + 4 * k as u32, *w);
            }
            *snd_state.borrow_mut() = (vols, mode);
            r.mem.write_f32(snd + 0x383c, speech);
            r.mem.write_u32(0x343050, if with_sound { snd } else { 0 });
            if !fresh_table {
                r.cpu.set_gpr(4, reltab as u64);
                r.call(0x167208).unwrap();
            }
            log.borrow_mut().clear();
            r.cpu.set_gpr(4, career as u64);
            match r.call(0x1617a0) {
                Ok(()) => break,
                Err(r5900::Error::UnstubbedCall { target, .. }) if target >= 0x2f0000 => r.allow(target),
                Err(e) => panic!("case {case}: {e}"),
            }
        }
        let mut s = Snd { vols, speech, mode, log: vec![] };
        if with_sound {
            ours.init_new_game(language, &mut streams, Some(&mut s));
        } else {
            ours.init_new_game(language, &mut streams, None);
        }
        let mut diffs = vec![];
        for k in 0..CAREER_SIZE {
            let b = (r.mem.read_u32((career + k as u32) & !3) >> (8 * ((career + k as u32) & 3))) as u8;
            if b != ours.bytes[k] && diffs.len() < 20 {
                diffs.push(format!("+{k:#x}: game {b:#04x} ours {:#04x}", ours.bytes[k]));
            }
        }
        assert!(diffs.is_empty(), "case {case} (language {language}, sound {with_sound}):\n{}", diffs.join("\n"));
        let theirs_rand = Rand { s: [0, 1, 2, 3, 4, 5].map(|k| r.mem.read_u32(FX_STATE + 4 * k)) };
        assert_eq!(theirs_rand, streams.fx, "case {case}: the effects stream");
        assert_eq!(*log.borrow(), s.log, "case {case}: sound calls");
    }
}
