//! `Boarder_GetRaceProgress`, `GetTimeGap`, `EstimateFinishTime` and `IsOnRecordPace` (`tricky_game::progress`, board
//! row G1f) against the original in the function runner (TRICKY_ELF; skipped without it).

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::progress::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

#[test]
fn progress_functions_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x500)).collect();
    let level = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x1c, race);
    let rs = riders.clone();
    r.stub(0x1814a8, move |c, _| c.set_gpr(2, rs[c.gpr(5) as usize] as u64));
    r.stub(0x197ef0, move |c, _| c.set_gpr(2, level as u64));
    let total_cell = r.mem.alloc(0x10);
    r.mem.write_f32(total_cell, 0.0);
    let (rs, tc) = (riders.clone(), total_cell);
    r.stub(0x11ea20, move |c, m| {
        let _ = &rs;
        c.set_f(0, m.read_f32(tc));
    });
    let rs = riders.clone();
    r.stub(0x11ea28, move |c, m| c.set_gpr(2, m.read_u32(c.gpr(4) as u32 + 0x4f0) as i32 as i64 as u64));
    let _ = rs;
    let rec = r.mem.alloc(0x200);
    let rc = rec;
    r.stub(0x163468, move |c, _| c.set_gpr(2, rc as u64));
    let mut g = Rng(0x3141_5926_5358_9793);
    for case in 0..30_000 {
        let n = 2 + g.next() as usize % 5;
        r.mem.write_u32(race + 0x88, n as u32);
        let lap_len = [g.f(-5.0, 9000.0), 0.0, -1.0, 5000.0][g.next() as usize % 4];
        r.mem.write_f32(level + 0x38, lap_len);
        let total = g.f(1000.0, 90000.0);
        r.mem.write_f32(total_cell, total);
        let ps: Vec<Progress> = (0..n)
            .map(|_| {
                let line = g.f(0.0, 9000.0);
                Progress {
                    rank: [-1, 0, 1, 2, 3, g.next() as i32 % 6][g.next() as usize % 6],
                    laps: [0, 1, 2, 3, g.next() as i32 % 5][g.next() as usize % 5],
                    along: [g.f(0.0, 9000.0), line, line - 0.5][g.next() as usize % 3],
                    line,
                    finish: if g.next() % 3 == 0 { g.next() as i32 % 1000 } else { -1 },
                    time: (g.next() % 40000) as i32,
                }
            })
            .collect();
        for (k, p) in ps.iter().enumerate() {
            let b = riders[k];
            r.mem.write_u32(b + 0x110, p.rank as u32);
            r.mem.write_u32(b + 0x114, p.laps as u32);
            r.mem.write_f32(b + 0x374, p.along);
            r.mem.write_f32(b + 0x37c, p.line);
            r.mem.write_u32(b + 0x418, p.finish as u32);
            r.mem.write_u32(b + 0x4f0, p.time as u32);
        }
        let me = g.next() as usize % n;
        let lap = if lap_len <= 0.0 { lap_len } else { lap_len };
        r.cpu.set_gpr(4, riders[me] as u64);
        r.call(0x1170d0).unwrap();
        assert_eq!(r.cpu.f(0).to_bits(), race_progress(&ps[me], lap).to_bits(), "progress {case}");
        r.cpu.set_gpr(4, riders[me] as u64);
        r.call(0x11f118).unwrap_or_else(|e| panic!("gap {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as i32, time_gap(&ps[me], &ps, total, lap), "gap {case}: me {me} {ps:?}");
        r.cpu.set_gpr(4, riders[me] as u64);
        r.call(0x11f2a0).unwrap();
        let est = estimate_finish_time(&ps[me], total, lap);
        assert_eq!(r.cpu.gpr(2) as i32, est, "estimate {case}");
        // record pace: the record row at (count-1)*0x14+0x10
        let count = 1 + g.next() % 5;
        r.mem.write_u32(rec + 0x64, count);
        let t = (g.next() % 60000) as i32;
        r.mem.write_u32(rec + (count - 1) * 0x14 + 0x10, [t as u32, est as u32, est as u32 + 1][g.next() as usize % 3]);
        let want = r.mem.read_u32(rec + (count - 1) * 0x14 + 0x10) as i32;
        r.cpu.set_gpr(4, riders[me] as u64);
        r.call(0x11f350).unwrap_or_else(|e| panic!("pace {case}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32 & 1 != 0, is_on_record_pace(est, want), "pace {case}");
    }
}
