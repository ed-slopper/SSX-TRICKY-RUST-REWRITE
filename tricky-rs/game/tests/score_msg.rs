//! The score's messages (board row G2) against the original in the function runner (TRICKY_ELF; skipped without
//! it): `Scoring_PushMessage` 0x157330 on random timed entries (every kind, lives and ages around their end, 7 / 8
//! / 12 already present or not, all twelve taken), `Scoring_MsgMultiplier` 0x155ca0, `SpinState_SetTrickClipRate`
//! 0x156050 on every clip, and the empty 0x155cc8; the entries compared word for word and the calls out.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::score::{announce_speaks_trick, trick_clip_rate, Score, ScoreCall, Timed};

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

const F: [f32; 9] = [0.0, 2.5, 1.5, 3.0, 2.1, -1.0, 2.4999, 0.0166, 1.0];

fn read(r: &mut Runner, at: u32) -> [Timed; 12] {
    let mut t = [Timed::default(); 12];
    for (k, e) in t.iter_mut().enumerate() {
        let b = at + 0x15c + 20 * k as u32;
        *e = Timed {
            kind: r.mem.read_u32(b),
            value: r.mem.read_u32(b + 4) as i32,
            life: f32::from_bits(r.mem.read_u32(b + 8)),
            age: f32::from_bits(r.mem.read_u32(b + 12)),
            extra: r.mem.read_u32(b + 16) as i32,
        };
    }
    t
}

#[test]
fn score_messages_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(0x400);
    let calls: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let c2 = calls.clone();
    r.stub(0x233220, move |c, _| c2.borrow_mut().push((5..8).map(|k| c.gpr(k) as u32).collect()));
    let mut g = Rng(0x1573_30ee_8e8e_0001);
    for case in 0..60_000u32 {
        // the entries: kinds 0..14, now and then all twelve taken, lives and ages around their end
        let full = g.below(4) == 0;
        for k in 0..12u32 {
            let b = base + 0x15c + 20 * k;
            let kind = if full { 1 + g.below(14) } else { g.below(15) };
            r.mem.write_u32(b, kind);
            r.mem.write_u32(b + 4, g.next());
            r.mem.write_u32(b + 8, F[g.below(F.len() as u32) as usize].to_bits());
            r.mem.write_u32(b + 12, F[g.below(F.len() as u32) as usize].to_bits());
            r.mem.write_u32(b + 16, g.next());
        }
        r.mem.write_u32(base + 0x24c, g.next());
        let mut s = Score { timed: read(&mut r, base), ..Score::default() };
        let (life, kind, value, extra) = (F[g.below(F.len() as u32) as usize].to_bits(), [7, 8, 12, 9, 6, 1, 2, 4, 13, 15, 0][g.below(11) as usize], g.next() as i32, g.next() as i32);
        calls.borrow_mut().clear();
        r.cpu.set_f(12, f32::from_bits(life));
        r.cpu.set_gpr(4, base as u64);
        r.cpu.set_gpr(5, kind as u64);
        r.cpu.set_gpr(6, value as i64 as u64);
        r.cpu.set_gpr(7, extra as i64 as u64);
        r.call(0x157330).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = read(&mut r, base);
        s.push_message(life, kind, value, extra);
        assert_eq!(theirs, s.timed, "case {case}: kind {kind}");
        // the speech call after, always: the score's rider (+0x24c), the kind, the value
        assert_eq!(*calls.borrow(), vec![vec![r.mem.read_u32(base + 0x24c), kind, value as u32]], "case {case}: speech");
    }
    // the multiplier's message (the push logged)
    let pushed: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let p2 = pushed.clone();
    r.stub(0x157330, move |c, _| p2.borrow_mut().push(vec![c.f(12).to_bits(), c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32]));
    for n in [0i32, 1, 2, 3, 5, 9, -1, -7] {
        pushed.borrow_mut().clear();
        r.cpu.set_gpr(4, base as u64);
        r.cpu.set_gpr(5, n as i64 as u64);
        r.cpu.set_gpr(7, 0x5555 as u64);
        r.call(0x155ca0).unwrap();
        let mine: Vec<Vec<u32>> = Score::default().msg_multiplier(n).into_iter().map(|c| match c {
            ScoreCall::Message(l, k, v, x) => vec![l, k, v as u32, x as u32],
            _ => vec![],
        }).collect();
        assert_eq!(*pushed.borrow(), mine, "multiplier {n}");
    }
    // `Score_AnnounceTrick`: the calls out in order (the name, the announcer, the message) and the id kept; then
    // `Scoring_MsgKnockdown`
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let l = log.clone();
    r.stub(0x1551c0, move |c, m| {
        let at = c.gpr(5) as u32;
        l.borrow_mut().push(vec![0x1551c0, c.gpr(4) as u32, m.read_u32(at), m.read_u32(at + 4)]);
    });
    let l = log.clone();
    r.stub(0x232498, move |c, m| {
        let at = c.gpr(6) as u32;
        l.borrow_mut().push(vec![0x232498, c.gpr(4) as u32, c.gpr(5) as u32, m.read_u32(at), m.read_u32(at + 4), c.gpr(7) as u32]);
    });
    let l = log.clone();
    r.stub(0x2333e0, move |c, _| l.borrow_mut().push(vec![0x2333e0, c.gpr(4) as u32, c.gpr(5) as u32]));
    let focus = Rc::new(RefCell::new(0u64));
    let f2 = focus.clone();
    r.stub(0x157c50, move |c, _| c.set_gpr(2, *f2.borrow()));
    r.stub(0x155440, |c, _| c.set_gpr(2, 4321));
    let idp = base + 0x300;
    let mut g = Rng(0x1570_88ee_8e8e_0001);
    for case in 0..2000u32 {
        let id = [g.next(), g.next()];
        let (repeat, mode, f) = ([0u32, 1, 7][g.below(3) as usize], [6u32, 2, 4, 0][g.below(4) as usize], g.below(2) as u64);
        r.mem.write_u32(idp, id[0]);
        r.mem.write_u32(idp + 4, id[1]);
        r.mem.write_u32(0x32de8c, mode);
        r.mem.write_u32(0x343050, 0x00ab_cd00);
        r.mem.write_u32(base + 0x24c, 0x0077_0000);
        *focus.borrow_mut() = f;
        log.borrow_mut().clear();
        pushed.borrow_mut().clear();
        r.cpu.set_gpr(4, base as u64);
        r.cpu.set_gpr(5, idp as u64);
        r.cpu.set_gpr(6, repeat as u64);
        r.call(0x157088).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut s = Score::default();
        let msg = s.announce_trick(id, repeat);
        assert_eq!([r.mem.read_u32(base + 0x6c), r.mem.read_u32(base + 0x70)], s.trick_id, "case {case}: id");
        let mut want = vec![vec![0x1551c0, base + 0x250, id[0], id[1]]];
        if f != 0 {
            want.push(if announce_speaks_trick(repeat, mode) {
                vec![0x232498, 0x00ab_cd00, 0x0077_0000, id[0], id[1], 4321]
            } else {
                vec![0x2333e0, 0x00ab_cd00, 0x0077_0000]
            });
        }
        assert_eq!(*log.borrow(), want, "case {case}: calls (repeat {repeat}, mode {mode}, focus {f})");
        let ScoreCall::Message(l, k, v, x) = msg else { panic!() };
        assert_eq!(*pushed.borrow(), vec![vec![l, k, v as u32, x as u32]], "case {case}: message");
    }
    for n in [0u32, 1, 0x7fff_ffff, u32::MAX] {
        pushed.borrow_mut().clear();
        r.mem.write_u32(base + 0xbc, n);
        r.cpu.set_gpr(4, base as u64);
        r.call(0x155d98).unwrap();
        let mut s = Score { xbc: n as _, ..Score::default() };
        let ScoreCall::Message(l, k, v, x) = s.msg_knockdown() else { panic!() };
        assert_eq!(r.mem.read_u32(base + 0xbc), s.xbc as u32, "knockdown {n}");
        assert_eq!(*pushed.borrow(), vec![vec![l, k, v as u32, x as u32]], "knockdown {n}");
    }
    // every clip's rate
    for clip in 0x200u32..0x300 {
        r.mem.write_u32(base + 0x2c, 0x1234_5678);
        r.cpu.set_gpr(4, base as u64);
        r.cpu.set_gpr(5, clip as u64);
        r.call(0x156050).unwrap();
        let theirs = r.mem.read_u32(base + 0x2c);
        let mine = trick_clip_rate(clip).map_or(0x1234_5678, f32::to_bits);
        assert_eq!(theirs, mine, "clip {clip:#x}");
    }
    // the empty function changes nothing
    r.call(0x155cc8).unwrap();
}
