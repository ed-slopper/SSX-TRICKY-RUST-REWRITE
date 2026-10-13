//! `tricky_game::effects::plan` against `Boarder_UpdateEffects` 0x1350c0 in the function runner (TRICKY_ELF;
//! skipped without it), the six effects stubbed to record what they were called with.

use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::effects::*;

#[test]
fn update_effects_runs_the_same_effects_in_the_same_order() {
    let Some(mut r) = Runner::from_env() else { return };
    let log: Rc<RefCell<Vec<(u32, u32, u32)>>> = Rc::new(RefCell::new(vec![]));
    for (addr, _) in [(0x12eca8u32, Effect::Puff), (0x12fef0, Effect::Trail), (0x1311b0, Effect::Spray), (0x12e6e8, Effect::Brake), (0x12f4c0, Effect::Dust), (0x132648, Effect::Track)] {
        let l = log.clone();
        r.stub(addr, move |c, _| l.borrow_mut().push((addr, c.gpr(4) as u32, c.gpr(5) as u32)));
    }
    let app = r.mem.alloc(0x800);
    let mid = r.mem.alloc(0x400);
    let scr = r.mem.alloc(0x100);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, mid);
    r.mem.write_u32(mid + 0x1c, scr);
    let base = r.mem.alloc(0xc000);
    let rider = r.mem.alloc(0x10);
    for players in [-1i32, 0, 1, 2, 3, 4] {
        for window in [-1i32, 0, 1, 2, 3, 4, 100] {
            r.mem.write_u32(mid + 0x298, players as u32);
            r.mem.write_u32(scr + 0x88, window as u32);
            log.borrow_mut().clear();
            r.cpu.set_gpr(4, base as u64);
            r.cpu.set_gpr(5, rider as u64);
            r.call(0x1350c0).unwrap();
            let want: Vec<(u32, u32, u32)> = plan(players, window)
                .into_iter()
                .map(|(e, off)| {
                    let addr = match e {
                        Effect::Puff => 0x12eca8,
                        Effect::Trail => 0x12fef0,
                        Effect::Spray => 0x1311b0,
                        Effect::Brake => 0x12e6e8,
                        Effect::Dust => 0x12f4c0,
                        Effect::Track => 0x132648,
                    };
                    (addr, base + off, rider)
                })
                .collect();
            assert_eq!(*log.borrow(), want, "players {players} window {window}");
        }
    }
}
