//! `tricky_game::nextevent` (board rows G8n5a, G8n5b, G8n5c) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000) with the overlay (its flags, slots,
//! progress, state, pick) and its backdrop in the arena made random, the app, its font and the renderer's table,
//! the card system's and the overlay's own tables pointing at stand-ins. Then one of the row's functions runs on
//! both sides. Every callee is a stand-in on both sides answering from the case's seed; the draws log their float
//! arguments and the drawer's colour, the text box's draw its alignment words and colour. The runner's stack is
//! cleared before each case and a buffer on it logs as one token. The calls and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::nextevent as ne;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::TextBoxEnv;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x328000, 0x18000);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2800;
const THIS: u32 = ARENA;
const BACKDROP: u32 = ARENA + 0x800;
const FONT: u32 = ARENA + 0xa00;
const APPOBJ: u32 = ARENA + 0x1000;
const RENDER: u32 = ARENA + 0x1800;
const RVT: u32 = ARENA + 0x1900;
const RECT: u32 = ARENA + 0x1c00;
const CARD: u32 = ARENA + 0x1c80;
const CARD_VT: u32 = ARENA + 0x1d00;
const OWN_VT: u32 = ARENA + 0x1f00;
const RACE: u32 = ARENA + 0x2000;
const HUDOBJ: u32 = ARENA + 0x2100;
const R1C: u32 = ARENA + 0x2200;
const TIMES: (u32, u32) = (0x3ce300, 0x600);
const SYS_AT: (u32, u32) = (0x343000, 0x100);
const VTABLES: (u32, u32) = (0x3b6c00, 0x300);
const COUNTS: (u32, u32) = (0x37ed00, 0x100);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
/// the card system's virtuals' stand-ins and the overlay's own
fn cv(off: u32) -> u32 {
    0x0180_0000 + off
}
fn ov(off: u32) -> u32 {
    0x0181_0000 + off
}
const ROFFS: [u32; 4] = [0x1d0, 0x1e0, 0x1f0, 0x1f8];
const COFFS: [u32; 6] = [0x70, 0x1a0, 0x1b0, 0x1d0, 0x1d8, 0x1e0];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w32(&mut self, a: u32, v: u32);
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn w32(&mut self, a: u32, v: u32) {
        self.write(a, &v.to_le_bytes()).unwrap()
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn stack(a: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&a)
}
fn token(a: u32) -> u32 {
    if stack(a) { STACK_TOKEN } else { a }
}

thread_local! {
    static BAR_RECT: Cell<u32> = const { Cell::new(0) };
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

/// How many int and float arguments a callee has (`this` counted for a virtual)
fn arity(func: u32) -> (usize, usize) {
    match func {
        f if f == rv(0x1f0) || f == rv(0x1f8) => (3, 0),
        f if f & 0xffff_0000 == 0x01ff_0000 => (2, 0),
        f if f == cv(0x70) => (3, 0),
        f if f == cv(0x1b0) => (2, 0),
        f if f == cv(0x1e0) => (4, 0),
        f if f & 0xffff_0000 == 0x0180_0000 || f & 0xffff_0000 == 0x0181_0000 => (1, 0),
        ne::LOC | ne::SET_COLOUR | ne::GET_RECT | ne::TB_SET_FONT | ne::TB_APPEND | ne::TB_DELETE => (2, 0),
        ne::FILLED_RECT | ne::DRAW_LINE | ne::TB_SET_RECT => (1, 4),
        ne::MEASURE_WIDE | ne::LABEL_BOX => (3, 1),
        ne::PRINTF_SHADOWED_A | ne::DRAW_STRING_A => (2, 3),
        ne::DRAW_SPRITE => (3, 4),
        ne::BACKDROP_FRAME => (2, 1),
        ne::DRAW_BAR => (2, 4),
        ne::WSTR_FORMAT | ne::WSTR_FORMAT_B | ne::TB_DRAW => (3, 0),
        ne::WSTR_COUNT | ne::TB_INIT | ne::TB_LINE_COUNT | ne::SLOT_COUNT => (1, 0),
        ne::MC_NEXT | ne::MC_PREV | ne::MC_SLOT_NEXT | ne::MC_SLOT_PREV => (1, 0),
        ne::MOVE_SOUND | ne::BACK_SOUND | ne::SELECT_SOUND => (1, 0),
        ne::TB_SET_LINE_HEIGHT => (1, 1),
        ne::SLOT_LABEL => (4, 0),
        ne::BACKDROP_RESET | ne::BACKDROP_UPDATE | ne::DIALOG_DRAW | ne::MC_UPDATE | ne::MC_IS_DONE | ne::MC_RESULT => (1, 0),
        ne::MC_DESTROY | ne::MC_CREATE | ne::MC_RESTART | ne::MC_DRAW | ne::CAREER_SAVE_A | ne::SAVE_TO_CARD => (1, 0),
        ne::TICK_SOUND | ne::ERROR_SOUND | ne::END_RACE | ne::UPDATE_COLOR | ne::BAR_DRAW | ne::TB_RESET => (1, 0),
        ne::DIALOG_POLL | ne::MC_BEGIN | ne::MC_START_SAVE | ne::NEW_BUTTONS | ne::PLAYER_ID | ne::PLAYER_PROFILE => (2, 0),
        ne::PLAYER_LEVEL | ne::LEVEL_NAME | ne::POINTS | ne::SET_PENDING_VIDEO | ne::SPEECH_LEVEL | ne::SND_STOP_ONE => (2, 0),
        ne::BAR_SET_POS | ne::BAR_DELETE | ne::STRCPY | ne::TB_WRAP => (2, 0),
        ne::REFUND_POINT | ne::SPEND_POINT | ne::ATTRIBUTES | ne::OVERLAY_HIDE | ne::HUD_SHOW => (3, 0),
        ne::SPRINTF => (4, 0),
        ne::DIALOG_SET => (6, 0),
        ne::DRAW_MENU_LIST => (10, 3),
        ne::PRINT_SHADOWED_CENTRED | ne::PRINTF_SHADOWED_CENTERED | ne::PRINT_SHADOWED_RIGHT | ne::PRINTF_SHADOWED => (2, 3),
        ne::BAR_SET_VALUE => (1, 1),
        ne::TB_SET_SCALE => (1, 2),
        ne::VENUE_RACE | ne::VENUE_FREE | ne::VENUE_OTHER | ne::STRLEN | ne::RESULTS_UPDATE => (1, 0),
        ne::COURSE_UNLOCKED | ne::COURSE_MEDAL | ne::MEMSET => (3, 0),
        ne::WSTR_SET_ASCII | ne::WSTR_COPY => (2, 0),
        ne::VEC4_SET => (1, 4),
        f if f == BAR_RECT.with(|b| b.get()) => (2, 0),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    // (the venue text's format takes three)
    let ni = if func == ne::WSTR_FORMAT_B && w[1] == ne::FMT_VENUE { 5 } else { ni };
    // (the video's "%s" takes one)
    let ni = if func == ne::SPRINTF && w[1] == ne::FMT_VIDEO { 3 } else { ni };
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    if func == ne::SET_COLOUR {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    if matches!(
        func,
        ne::FILLED_RECT | ne::DRAW_LINE | ne::PRINTF_SHADOWED_A | ne::DRAW_STRING_A | ne::DRAW_SPRITE | ne::BACKDROP_FRAME
            | ne::DRAW_BAR | ne::LABEL_BOX | ne::PRINT_SHADOWED_CENTRED | ne::PRINTF_SHADOWED_CENTERED
            | ne::PRINT_SHADOWED_RIGHT | ne::PRINTF_SHADOWED | ne::DRAW_MENU_LIST
    ) {
        log.extend((0..4).map(|k| io.r32(ne::DRAW_COLOUR + 4 * k)));
    }
    // the progress bar's words (stack addresses as the token) at its calls, the rectangle its virtual and its
    // placing take, the menu list's label and "on"
    if matches!(func, ne::UPDATE_COLOR | ne::BAR_SET_VALUE | ne::BAR_SET_POS | ne::BAR_DRAW | ne::BAR_DELETE) || func == BAR_RECT.with(|b| b.get()) {
        let o = w[0];
        log.extend((0..0x1b0 / 4).map(|k| token(io.r32(o.wrapping_add(4 * k)))));
    }
    if func == ne::BAR_SET_POS || func == BAR_RECT.with(|b| b.get()) {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    if func == ne::DRAW_MENU_LIST {
        log.push(io.r32(w[3]));
        log.push(io.r32(w[9]));
    }
    // the text box's alignment words and colour as it draws
    if func == ne::TB_DRAW {
        log.extend([0x20u32, 0x24, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x6c].map(|o| io.r32(w[0] + o)));
    }
    // the box's alignment words as it goes, the backdrop's +0x50 as its frame draws
    if func == ne::TB_DELETE {
        log.extend([0x20u32, 0x24].map(|o| io.r32(w[0] + o)));
    }
    if func == ne::BACKDROP_FRAME {
        log.push(io.r32(w[0] + 0x50));
    }
    let r = ans.next();
    let answer = match func {
        ne::MEASURE_WIDE => [r % 150, 150 + r % 300, 390 + r % 200, 200, 384, 512][(r >> 9) as usize % 6],
        ne::GET_RECT => RECT,
        ne::WSTR_COUNT => [0u32, r][(r >> 5) as usize % 2],
        ne::SLOT_COUNT => r % 6,
        f if f == cv(0x1d0) || f == cv(0x1d8) => r % 7,
        f if f == cv(0x1a0) => r % 2,
        ne::NEW_BUTTONS => [0u32, 0x800, 0x800, 0x800, 0x20001, 0x40002, 0x100008, 0x80004, r][(r % 9) as usize],
        ne::DIALOG_POLL => [0u32, 1, 1, 1, 2, 3, 4][(r % 7) as usize],
        ne::MC_IS_DONE | ne::MC_RESULT | ne::REFUND_POINT | ne::SPEND_POINT => r % 2,
        ne::POINTS => [0u32, 0, r % 5][(r >> 4) as usize % 3],
        ne::PLAYER_PROFILE => r % 16,
        ne::PLAYER_ID => r % 4,
        ne::VENUE_RACE | ne::VENUE_FREE | ne::VENUE_OTHER => r % 14,
        ne::COURSE_UNLOCKED => r % 2,
        ne::COURSE_MEDAL => r % 4,
        ne::STRLEN => [r % 20, 0x8000_0003, 0xffff_fff0][(r >> 3) as usize % 3],
        ne::MEMSET => {
            for k in 0..w[2] / 4 {
                io.w32(w[0] + 4 * k, (w[1] & 0xff) * 0x0101_0101);
            }
            0
        }
        // (the attributes: four rows of four words written for real)
        ne::ATTRIBUTES => {
            for k in 0..16 {
                io.w32(w[2] + 4 * k, (r >> (k % 8)) % 12);
            }
            0
        }
        _ => r,
    };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(10, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(8, 0);
        let (a, c) = standin(m, func, &w, &f, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

const FUNCS: [u32; 44] = [
    0x1ba330, 0x1ba330, 0x1ba330, 0x1ba330,
    0x1bb678, 0x1bb678, 0x1bbf20, 0x1bbf20, 0x1bc6a0, 0x1bc6a8, 0x1bcd28, 0x1bd188, 0x1bd2b0, 0x1bd308, 0x1bd310,
    0x1bd358, 0x1bd3b8, 0x1bd898, 0x1bda18, 0x1bda30, 0x1bda38,
    0x1ba208, 0x1ba2f0, 0x1ba300, 0x1ba330, 0x1ba330, 0x1ba788, 0x1ba788, 0x1bb670,
    0x1b8718, 0x1b8bb8, 0x1b8d08, 0x1b8d08, 0x1b9530, 0x1b9530, 0x1b9968, 0x1b9b80, 0x1ba0f0, 0x1ba138, 0x1ba170,
    0x1ba178, 0x1ba1c0, 0x1ba1f8, 0x1ba200,
];

#[test]
fn next_event_screens_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in FUNCS {
        r.allow(f);
    }
    let mut stubs = vec![
        ne::LOC, ne::SET_COLOUR, ne::FILLED_RECT, ne::DRAW_LINE, ne::MEASURE_WIDE, ne::PRINTF_SHADOWED_A, ne::DRAW_STRING_A,
        ne::DRAW_SPRITE, ne::BACKDROP_FRAME, ne::LABEL_BOX, ne::DRAW_BAR, ne::GET_RECT, ne::WSTR_FORMAT, ne::WSTR_FORMAT_B,
        ne::WSTR_COUNT, ne::TB_INIT, ne::TB_SET_FONT, ne::TB_SET_RECT, ne::TB_APPEND, ne::TB_SET_LINE_HEIGHT,
        ne::TB_LINE_COUNT, ne::TB_DRAW, ne::TB_DELETE, ne::SLOT_LABEL, ne::SLOT_COUNT, ne::MC_NEXT, ne::MC_PREV,
        ne::MC_SLOT_NEXT, ne::MC_SLOT_PREV, ne::MOVE_SOUND, ne::BACK_SOUND, ne::SELECT_SOUND, ov(0xd8), ne::BACKDROP_RESET,
        ne::BACKDROP_UPDATE, ne::DIALOG_DRAW, ne::MC_UPDATE, ne::MC_IS_DONE, ne::MC_RESULT, ne::MC_DESTROY, ne::MC_CREATE,
        ne::MC_RESTART, ne::MC_DRAW, ne::CAREER_SAVE_A, ne::SAVE_TO_CARD, ne::TICK_SOUND, ne::ERROR_SOUND, ne::END_RACE,
        ne::UPDATE_COLOR, ne::BAR_DRAW, ne::TB_RESET, ne::DIALOG_POLL, ne::MC_BEGIN, ne::MC_START_SAVE, ne::NEW_BUTTONS,
        ne::PLAYER_ID, ne::PLAYER_PROFILE, ne::PLAYER_LEVEL, ne::LEVEL_NAME, ne::POINTS, ne::SET_PENDING_VIDEO,
        ne::SPEECH_LEVEL, ne::SND_STOP_ONE, ne::BAR_SET_POS, ne::BAR_DELETE, ne::STRCPY, ne::TB_WRAP, ne::REFUND_POINT,
        ne::SPEND_POINT, ne::ATTRIBUTES, ne::OVERLAY_HIDE, ne::HUD_SHOW, ne::SPRINTF, ne::DIALOG_SET, ne::DRAW_MENU_LIST,
        ne::PRINT_SHADOWED_CENTRED, ne::PRINTF_SHADOWED_CENTERED, ne::PRINT_SHADOWED_RIGHT, ne::PRINTF_SHADOWED,
        ne::BAR_SET_VALUE, ne::TB_SET_SCALE, ne::VENUE_RACE, ne::VENUE_FREE, ne::VENUE_OTHER, ne::STRLEN,
        ne::RESULTS_UPDATE, ne::COURSE_UNLOCKED, ne::COURSE_MEDAL, ne::MEMSET, ne::WSTR_SET_ASCII, ne::WSTR_COPY,
        ne::VEC4_SET,
    ];
    // (the results' buffers' clearing, real on both sides)
    r.allow(0x1aae88);
    // (the progress bar's virtual +0x138, from its table)
    let bar_rect = r.mem.read_u32(ne::PROGRESS_VTABLE + 0x13c);
    BAR_RECT.with(|b| b.set(bar_rect));
    stubs.push(bar_rect);
    stubs.extend(ROFFS.map(rv));
    stubs.extend(COFFS.map(cv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let mut w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            // (the menu list's ninth and tenth arguments, on the stack)
            let sp = c.gpr(29) as u32;
            w.push(mem.read_u32(sp));
            w.push(mem.read_u32(sp + 8));
            let f: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, func, &w, &f, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let data = read(&mut r, DATA);
    let vtables = read(&mut r, VTABLES);
    let mut g = Rng(0x5e1e_c700_0000_0001);
    let (mut rows, mut questions, mut bars) = (0usize, 0usize, 0usize);
    let mut more = [0usize; 8];
    let per = 4000u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (ARENA, vec![0; SIZE as usize]),
                (SCRATCH, vec![0; 0x3000]),
                (TIMES.0, vec![0; TIMES.1 as usize]),
                (SYS_AT.0, vec![0x77; SYS_AT.1 as usize]),
                (VTABLES.0, vtables.clone()),
                (COUNTS.0, vec![0; COUNTS.1 as usize]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (strings +0x50, the font +0x60, the renderer +0x724), the renderer's table, the font's height
        m.w32(ne::APP, APPOBJ);
        m.w32(APPOBJ + 0x60, FONT);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        m.w32(FONT + 0xc, g.pick(&[0u32, 12, 22, 30, 13, 25]));
        wf(&mut m, RECT + 8, g.f(0.0, 200.0));
        // the overlay: its word 0, the options' flags, the slot picked, the progress, the message's alpha, the
        // state, the slots' flags, the entries and pick, the card system and its table, its own table, the
        // backdrop, the title
        m.w32(THIS, g.pick(&[0u32, 1, 2]));
        for o in [0x10u32, 0x18, 0x1c] {
            m.w32(THIS + o, g.below(2));
        }
        m.w32(THIS + 0x38, g.pick(&[0u32, 1, 2, 5, 0xffff_ffff]));
        let p = g.f(0.0, 1.0);
        wf(&mut m, THIS + 0x48, g.pick(&[0.0f32, -0.5, p, 1.0]));
        wf(&mut m, THIS + 0x4c, g.f(0.0, 1.0));
        m.w32(THIS + 0x54, g.pick(&[7u32, 7, 3]));
        for i in 0..8 {
            m.w32(THIS + 0x168 + 0x58 * i, g.pick(&[0u32, 1, 1]));
            m.w32(THIS + 0x174 + 0x58 * i, g.pick(&[0u32, 1, 1]));
        }
        m.w32(THIS + 0x218, g.below(6));
        m.w32(THIS + 0x21c, g.pick(&[0u32, 1, 2, 5, 0xffff_ffff]));
        m.w32(THIS + 0x224, CARD);
        m.w32(CARD, CARD_VT);
        for off in COFFS {
            m.w16(CARD_VT + off, g.pick(&[0u16, 8]));
            m.w32(CARD_VT + off + 4, cv(off));
        }
        m.w32(THIS + 0x428, OWN_VT);
        m.w16(OWN_VT + 0xd8, g.pick(&[0u16, 4]));
        m.w32(OWN_VT + 0xdc, ov(0xd8));
        m.w32(THIS + 0x42c, BACKDROP);
        for k in 0..4 {
            wf(&mut m, ne::DRAW_COLOUR + 4 * k, g.f(0.0, 1.0));
        }
        // the points overlay's: the race (+0x38 its HUD, +0x1c), the backdrop's dialog, fonts and alpha, the card
        // operation, the points at open, the attribute picked, the exit timer, the career's mode, the exit's wait,
        // the points noted
        m.w32(APPOBJ + 0x730, RACE);
        m.w32(RACE + 0x38, HUDOBJ);
        m.w32(RACE + 0x1c, R1C);
        m.w32(BACKDROP + 0x328, g.pick(&[0u32, 0, 1]));
        m.w32(BACKDROP + 4, FONT);
        m.w32(BACKDROP + 8, FONT + 0x40);
        wf(&mut m, BACKDROP + 0x70, g.f(0.0, 1.0));
        m.w32(THIS + 0x430, g.pick(&[0u32, 0, 1, 1, 1, 2]));
        m.w32(THIS + 0x438, g.below(30));
        m.w32(THIS + 0x440, g.pick(&[0u32, 1, 2, 3, 0xffff_ffff]));
        m.w32(THIS + 0x444, g.pick(&[0u32, 0, 1]));
        let t = g.f(-0.5, 2.0);
        wf(&mut m, THIS + 0x448, g.pick(&[0.0f32, 1.0 / 60.0, 0.01, t]));
        m.w32(ne::CAREER + 0x370c, g.pick(&[1u32, 0, 2]));
        m.w32(ne::EXIT_WAITS, g.below(2));
        m.w32(ne::POINTS_SEEN, g.below(2));
        m.w32(ne::CAREER + 0x2afc, g.below(10));
        m.w32(APPOBJ + 0x58, g.pick(&[1u32, 2, 3]));
        let any = g.next();
        m.w32(ne::CAREER + 4, any);
        // the venue / event overlays': the lists' counts, the career's venue mode, the menu's items and pick,
        // the venues, their texts, flags and sprites, the event's flags, pick and help line; the help overlay's
        // backdrop at its word 0
        for k in 0..3 {
            m.w32(ne::VENUE_COUNTS + 4 * k, g.below(8));
        }
        m.w32(ne::CAREER + 0x3818, g.pick(&[1u32, 2, 3, 4, 5, 7, 0]));
        m.w32(THIS + 0x438, g.below(9));
        m.w32(THIS + 0x43c, g.below(9));
        for k in 0..9 {
            m.w32(THIS + 0x440 + 4 * k, g.below(14));
            m.w32(THIS + 0x4b0 + 4 * k, g.below(2));
        }
        for k in 0..5 {
            m.w32(THIS + 0x44c + 4 * k, g.below(2));
        }
        m.w32(THIS + 0x558, g.below(9));
        m.w32(THIS + 0x460, g.below(5));
        m.w32(THIS + 0x464, g.below(2));
        if matches!(func, 0x1bd2b0..=0x1bd3b8) {
            m.w32(THIS, BACKDROP);
            wf(&mut m, BACKDROP + 0x2c, g.f(0.0, 480.0));
        }
        let any4 = g.next();
        let (any1, any2, any3) = (g.next(), g.next(), g.next());
        let text = g.pick(&[0u32, any1, any1]);
        let first = g.pick(&[0u32, any2, any2]);
        let second = g.pick(&[0u32, any3, any3]);
        let swapped = g.below(2);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let e = &mut env;
        let mm = &mut m;
        let mut ours = u32::MAX;
        match func {
            0x1b8718 => ne::progress_draw(mm, e, THIS, SCRATCH),
            0x1b8bb8 => ne::message_draw(mm, e, THIS, SCRATCH),
            0x1b8d08 => ne::card_screen_draw(mm, e, THIS, SCRATCH),
            0x1b9530 => ne::question_draw(mm, e, THIS, text, first, second, swapped, SCRATCH),
            0x1b9968 => ne::notice_draw(mm, e, THIS, text, SCRATCH),
            0x1b9b80 => ne::slots_draw(mm, e, THIS, SCRATCH),
            0x1ba0f0 => ne::vf47(mm, e, THIS),
            0x1ba138 => ne::vf48(mm, e, THIS),
            0x1ba170 | 0x1ba1f8 => ne::select_sound(mm, e, THIS),
            0x1ba178 => ne::vf50(mm, e, THIS),
            0x1ba1c0 => ne::vf51(mm, e, THIS),
            0x1ba200 => ne::back_sound(mm, e, THIS),
            0x1ba208 => ne::points_reset(mm, e, THIS),
            0x1ba2f0 => ne::points_vf16(mm, e),
            0x1ba300 => {
                ours = ne::start_exit_timer(mm, THIS);
            }
            0x1ba330 => ne::points_update(mm, e, THIS, SCRATCH),
            0x1ba788 => ne::points_draw(mm, e, THIS, SCRATCH),
            0x1bb670 => ne::points_vf3(mm, e, THIS),
            0x1bb678 => ne::venue_reset(mm, e, THIS),
            0x1bbf20 => ne::draw_venue(mm, e, THIS, SCRATCH),
            0x1bc6a0 | 0x1bd188 => ne::venue_vf3(mm, e, THIS),
            0x1bc6a8 => ne::event_reset(mm, e, THIS),
            0x1bcd28 => ne::draw_event(mm, e, THIS, SCRATCH),
            0x1bd2b0 => ne::help_reset(mm, e, THIS),
            0x1bd308 => ne::help_empty(),
            0x1bd310 => ne::help_update(mm, e, THIS),
            0x1bd358 => ne::help_draw(mm, e, THIS),
            0x1bd3b8 => ne::help_draw_text(mm, e, THIS, SCRATCH),
            0x1bd898 => ne::help_static_init(mm),
            0x1bda18 => ne::vec4_from(mm, e, any4, THIS + 0x100),
            0x1bda30 => ne::set_help(mm, THIS, any4),
            0x1bda38 => {
                ours = ne::identity(THIS);
            }
            _ => unreachable!(),
        }
        rows += env.log.iter().filter(|c| c.0 == cv(0x70)).count();
        questions += env.log.iter().filter(|c| c.0 == ne::DRAW_SPRITE && func == 0x1b9530).count();
        bars += env.log.iter().filter(|c| c.0 == ne::DRAW_BAR).count();
        let br = BAR_RECT.with(|b| b.get());
        for (k, f) in [br, ne::BAR_SET_POS, ne::DIALOG_SET, ne::MC_START_SAVE, ne::COURSE_MEDAL, ne::WSTR_COPY, ne::VEC4_SET, ne::END_RACE].into_iter().enumerate() {
            more[k] += env.log.iter().filter(|c| c.0 == f).count();
        }
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x1b9530 => vec![THIS, text, first, second, swapped],
            0x1b9968 => vec![THIS, text],
            0x1bda18 => vec![any4, THIS + 0x100],
            0x1bda30 => vec![THIS, any4],
            _ => vec![THIS],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != u32::MAX {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{rows} entries, {questions} answer sprites, {bars} bars");
    assert!(rows > 1000 && questions > 1000 && bars > 1000);
    println!("bar rects, bar places, dialogs, saves, medals, copies, vectors, race ends: {more:?}");
    assert!(more.iter().all(|&n| n > 30));
}
