//! The results screens' pages (board row G9v): what `cResultsOverlay` shows after a race, built into the results
//! table the overlay draws (0x3385c0: 8 rows of 16 cells, each a 0x80-byte wide string; the rows' names at
//! 0x33c5c0, the title at 0x33c9c0, the columns' places at 0x338550). On the bytes at their addresses ([`Mem`]);
//! every callee that is not the row's (the strings, the career, the speech, the HUD's formats) is a [`TextBoxEnv`]
//! call by address. Checked in the function runner (`tests/resultpages.rs`).
//!
//! The overlay (`this`): +0 its page (+4, +8 two numbers the draw reads, +0xc the page's menu items, 8 bytes each:
//! a kind and a string, +0x34 how many, +0x50 a flag), +4 the HUD, +8 / +0xc whether the race's and the show-off's
//! final unlock was had, +0x10 the menu's count again, +0x1c the medal's rank, +0x20 the verdict line (a wide string),
//! +0xc4 whether the verdict is shown.
//!
//! Where the game formats on its stack, the port formats in `scratch` (STANDIN: the place differs, not the bytes).

use crate::animseq::{cvt_s_w, div};
use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

/// The results table: 8 rows of 16 cells of 0x80 bytes
pub const CELLS: u32 = 0x3385c0;
/// The rows' names, 8 of 0x80 bytes
pub const ROW_NAMES: u32 = 0x33c5c0;
pub const TITLE: u32 = 0x33c9c0;
/// The columns' places (17 floats) and the layout's left edge
pub const COL_X: u32 = 0x338550;
pub const LAYOUT_X: u32 = 0x3385bc;
/// Five more columns' places (the career page's) and four columns' alignments
pub const COL_RIGHT: u32 = 0x338598;
pub const COL_ALIGN: u32 = 0x3384d0;
/// The event modes' titles (a table of strings)
pub const MODE_TITLES: u32 = 0x3384b0;
/// Four lines of 0x78 bytes (a word before each), cleared with the pages
pub const LINES: u32 = 0x3382d8;
/// Two tables of ids set to −1 with the pages (16 and 8 words): which rider each row shows
pub const ROW_RIDERS: u32 = 0x338510;
pub const ROW_IDS: u32 = 0x3384f0;
pub const CAREER: u32 = 0x32b390;
/// The race: its riders' count, then 0x84 bytes each (+8 the name, +0x3c the character, +0x69 the player or −1)
pub const RACE: u32 = 0x32db6c;
/// The race's finishing order and times (the results, G4j)
pub const ORDER: u32 = 0x32e5f4;
pub const TIMES: u32 = 0x32e60c;
/// The characters' table (0x38 each; +4 the name)
pub const CHARACTERS: u32 = 0x331ea8;
pub const APP: u32 = 0x337c58;
pub const SPEECH: u32 = 0x343050;
/// "%d", "%d%s", "%f", "%f%s"
pub const FMT_D: u32 = 0x38e200;
pub const FMT_DS: u32 = 0x38e208;
pub const FMT_F: u32 = 0x38e210;
pub const FMT_FS: u32 = 0x38e218;
/// The narrow string the career page puts in each row's third cell
pub const CAREER_CELL2: u32 = 0x38e220;

pub const LOC: u32 = 0x2c5160;
pub const MEMSET: u32 = 0x2cc808;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
pub const WSTR_FORMAT_ASCII: u32 = 0x17ce70;
pub const GET_RECT: u32 = 0x1f9e50;
pub const BACKDROP_RESET: u32 = 0x1aa628;
pub const GOT_RACE_FINAL: u32 = 0x1618c0;
pub const GOT_SHOWOFF_FINAL: u32 = 0x1618e0;
pub const EVENT_MODE: u32 = 0x1635b0;
pub const PLAYER_ID: u32 = 0x166580;
pub const ATTR_POINTS: u32 = 0x164368;
pub const MEDAL_RANK: u32 = 0x163e28;
pub const CLAMP_RANK: u32 = 0x16bb58;
pub const FORMAT_TIME: u32 = 0x1a6ba0;
pub const SPEECH_END: u32 = 0x231c50;
pub const SPEECH_CLEAR: u32 = 0x228d10;
pub const COUNT_PROFILES: u32 = 0x164288;
pub const FIND_RIDER: u32 = 0x166598;
pub const SPRINTF: u32 = 0x2fca28;
/// The speed's units: miles and kilometres an hour
pub const UNIT_MPH: u32 = 0x38e438;
pub const UNIT_KMH: u32 = 0x38e440;
pub const TRICK_NAME: u32 = 0x1551c0;
pub const CIRCUIT_MEDAL: u32 = 0x16baa8;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}
fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
fn k(bits: u32) -> F {
    F(f32::from_bits(bits))
}

/// A cell's place: `row << 11 | col << 7` into the table (0x1bfa70 and 0x1bfa90 return it)
pub fn cell(row: u32, col: u32) -> u32 {
    (row << 11).wrapping_add((col << 7).wrapping_add(CELLS))
}
/// A row's name's place (0x1bfae0 and 0x1bfaf8)
pub fn row_name(row: u32) -> u32 {
    (row << 7).wrapping_add(ROW_NAMES)
}

/// The two halves of a float made double (`__extendsfdf2` 0x2f7aa8), as a format's argument
fn double(v: F) -> [u32; 2] {
    let d = (v.0 as f64).to_bits();
    [d as u32, (d >> 32) as u32]
}

/// `Results_SetCellFloat` 0x1bf918(_, row, col, suffix; value): "%f%s". (The double goes in one register; the
/// port passes its two halves.)
pub fn set_cell_float_suffix(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, suffix: u32, v: F) {
    let [lo, hi] = double(v);
    call(m, env, WSTR_FORMAT_ASCII, &[cell(row, col), FMT_FS, lo, hi, suffix]);
}
/// 0x1bf980(_, row, col; value): "%f"
pub fn set_cell_float(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, v: F) {
    let [lo, hi] = double(v);
    call(m, env, WSTR_FORMAT_ASCII, &[cell(row, col), FMT_F, lo, hi]);
}
/// 0x1bf9d8(_, row, col, n, suffix): "%d%s"
pub fn set_cell_int_suffix(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, n: u32, suffix: u32) {
    call(m, env, WSTR_FORMAT_ASCII, &[cell(row, col), FMT_DS, n, suffix]);
}
/// `Results_SetCellInt` 0x1bfa08(_, row, col, n): "%d"
pub fn set_cell_int(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, n: u32) {
    call(m, env, WSTR_FORMAT_ASCII, &[cell(row, col), FMT_D, n]);
}
/// `Results_SetCellText` 0x1bfa30(_, row, col, wide)
pub fn set_cell_text(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, s: u32) {
    call(m, env, WSTR_COPY, &[cell(row, col), s]);
}
/// `Results_SetCellAscii` 0x1bfa50(_, row, col, narrow)
pub fn set_cell_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, col: u32, s: u32) {
    call(m, env, WSTR_SET_ASCII, &[cell(row, col), s]);
}
/// `Results_SetRowName` 0x1bfab0(_, row, wide)
pub fn set_row_name(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, s: u32) {
    call(m, env, WSTR_COPY, &[row_name(row), s]);
}
/// `Results_SetRowNameAscii` 0x1bfac8(_, row, narrow)
pub fn set_row_name_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, row: u32, s: u32) {
    call(m, env, WSTR_SET_ASCII, &[row_name(row), s]);
}
/// `Results_SetTitle` 0x1bfb10(_, wide)
pub fn set_title(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32) {
    call(m, env, WSTR_COPY, &[TITLE, s]);
}
/// `Results_SetTitleAscii` 0x1bfb20(_, narrow)
pub fn set_title_ascii(m: &mut Mem, env: &mut dyn TextBoxEnv, s: u32) {
    call(m, env, WSTR_SET_ASCII, &[TITLE, s]);
}

/// `Results_SetRowLayout` 0x1aae10(page; x, w): the left edge kept; the first column at x + 0.85 · the page's
/// width (its +0 → +4 → +0xc, a whole number) + 12, each next one that width · 0.85 + w further on
pub fn set_row_layout(m: &mut Mem, page: u32, x: F, w: F) {
    let base = m.r32(m.r32(page) + 4);
    wf(m, LAYOUT_X, x);
    let f0 = cvt_s_w(m.r32(base + 0xc) as i32) * k(0x3f59_999a);
    let step = f0 + w;
    let first = (x + f0) + F(12.0);
    wf(m, COL_X, first);
    for c in 1..17 {
        let v = rf(m, COL_X + 4 * (c - 1)) + step;
        wf(m, COL_X + 4 * c, v);
    }
}

/// `Results_ClearText` 0x1aae88: the rows' names and the cells cleared
pub fn clear_text(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    call(m, env, MEMSET, &[ROW_NAMES, 0, 0x400]);
    call(m, env, MEMSET, &[CELLS, 0, 0x4000]);
}

/// A menu item onto the page: its kind and string at +0xc + 8·count, then the count read again and one more (a
/// sixth item would be the count itself)
fn add_item(m: &mut Mem, this: u32, kind: u32, s: u32) {
    let page = m.r32(this);
    let n = m.r32(page + 0x34);
    let e = (n << 3).wrapping_add(page) + 0xc;
    m.w32(e, kind);
    m.w32(e + 4, s);
    let n = m.r32(page + 0x34);
    m.w32(page + 0x34, n.wrapping_add(1));
}

/// `Results_BuildPages` 0x1af720(this): the backdrop reset, the table and the overlay's lines cleared, the menu
/// emptied, whether the final unlock was had (the race's in mode 4, else the show-off's), the rank 3, the rows'
/// riders all −1; then the page of the career's mode (+0x2afc): 1 the stats, 2 and 9 the standings, 3 and 5 the
/// medals, 4 the career, 6 the lesson, 7 the time challenge, 8 or any other none. Last the menu's count kept and
/// the race's end lines queued.
pub fn build_pages(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let hud = m.r32(this + 4);
    call(m, env, BACKDROP_RESET, &[m.r32(hud + 0x42c)]);
    clear_text(m, env);
    call(m, env, MEMSET, &[this + 0x20, 0, 0xa0]);
    m.w32(this + 0xe8, 0);
    for k in 0..4 {
        let at = LINES + 0x78 * k;
        call(m, env, MEMSET, &[at, 0, 0x6e]);
        m.w32(this + 0xd0 + 4 * k, 0xffff_ffff);
        m.w32(at - 4, 0);
    }
    let page = m.r32(this);
    let mode = m.r32(CAREER + 0x2afc);
    m.w32(page + 0x34, 0);
    m.w32(page + 0x38, 0);
    m.w32(this + 0x10, 0);
    m.w32(this + 8, 0);
    m.w32(this + 0xc, 0);
    if mode == 4 {
        let v = call(m, env, GOT_RACE_FINAL, &[CAREER]);
        m.w32(this + 8, v);
    } else {
        let v = call(m, env, GOT_SHOWOFF_FINAL, &[CAREER]);
        m.w32(this + 0xc, v);
    }
    m.w32(this + 0x1c, 3);
    m.w32(this + 0xc8, 0);
    m.w32(this + 0xc4, 0);
    for k in (0..16).rev() {
        m.w32(ROW_RIDERS + 4 * k, 0xffff_ffff);
    }
    for k in (0..8).rev() {
        m.w32(ROW_IDS + 4 * k, 0xffff_ffff);
    }
    match m.r32(CAREER + 0x2afc) {
        1 => stats_page(m, env, this, scratch),
        2 | 9 => standings_page(m, env, this, scratch),
        3 | 5 => medal_page(m, env, this),
        4 => career_page(m, env, this, scratch),
        6 => lesson_page(m, env, this),
        7 => time_challenge_page(m, env, this),
        _ => {}
    }
    let page = m.r32(this);
    let speech = m.r32(SPEECH);
    let n = m.r32(page + 0x34);
    let hud = m.r32(this + 4);
    m.w32(this + 0x10, n);
    m.w32(hud + 0x430, 0);
    call(m, env, SPEECH_END, &[speech, n, hud]);
}

/// 0x1af940: the speech's event cleared (`SpeechSys_ClearEvent`(speech, 0))
pub fn clear_speech(m: &mut Mem, env: &mut dyn TextBoxEnv) {
    let s = m.r32(SPEECH);
    call(m, env, SPEECH_CLEAR, &[s, 0]);
}

/// The career page's menu entry for continuing: "continue" (0x18d) when the event is mode 2 and the player has
/// attribute points to spend, 0xdb5 when mode 2 without, 0xdb7 otherwise
fn has_points(m: &mut Mem, env: &mut dyn TextBoxEnv) -> u32 {
    let id = call(m, env, PLAYER_ID, &[CAREER, 0]);
    call(m, env, ATTR_POINTS, &[CAREER, id])
}

/// `Results_BuildCareerPage` 0x1af950(this): the layout from the HUD's sprite 0x37 (its x + 79, rows 18 apart),
/// five columns' places and four alignments, the rows' names (0xe75, 0xe5d, 0xe5c, 0xe61), the page's numbers (4
/// and the career's +0x27dc), the title of the event's mode. In mode 2 the menu is "continue", "spend points" (if
/// there are none to spend… the game's test is for none), "retry", "quit" and "save" (again if none), and the
/// verdict is shown with the columns spaced 37 apart. In the other modes the verdict line is "advance" (0x19c) if
/// no player finished in the first half of the race, else the mode's line (0x19f for mode 0, 0x19e otherwise),
/// and the menu has the continue entry and 0x191, 0x190, 0x193, 0x18e. Then each finisher's row: its place
/// ("%d%s" with "st", "nd", "rd", "th"), its name (the character's for a player, its own for the rest), the cell
/// string, its time; and with the verdict shown, a player's rank set and the verdict made.
pub fn career_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let hud = m.r32(this + 4);
    let sprites = m.r32(m.r32(hud + 0x42c));
    let r = call(m, env, GET_RECT, &[sprites, 0x37]);
    let x = (((rf(m, r + 8) + F(10.0)) + F(7.0)) + F(23.0)) + F(39.0);
    set_row_layout(m, m.r32(this), x, F(18.0));
    for (i, v) in [0x42a2_0000u32, 0x4324_0000, 0x4392_0000, 0x43f6_0000, 0x4411_8000].into_iter().enumerate() {
        m.w32(COL_RIGHT + 4 * i as u32, v);
    }
    m.w32(COL_ALIGN, 0);
    let page = m.r32(this);
    m.w32(page + 0x50, 1);
    m.w32(COL_ALIGN + 0xc, 2);
    m.w32(COL_ALIGN + 4, 0);
    m.w32(COL_ALIGN + 8, 0);
    for (row, id) in [0xe75u32, 0xe5d, 0xe5c, 0xe61].into_iter().enumerate() {
        let s = loc(m, env, id);
        set_row_name(m, env, row as u32, s);
    }
    let page = m.r32(this);
    m.w32(page + 8, 4);
    m.w32(page + 4, m.r32(CAREER + 0x27dc));
    let mode = call(m, env, EVENT_MODE, &[CAREER]);
    let t = m.r32((mode << 2).wrapping_add(MODE_TITLES));
    set_title(m, env, t);
    if mode == 2 {
        let s = loc(m, env, 0x18d);
        add_item(m, this, 0, s);
        if has_points(m, env) == 0 {
            let s = loc(m, env, 0x191);
            add_item(m, this, mode, s);
        }
        let s = loc(m, env, 0x190);
        add_item(m, this, 1, s);
        let s = loc(m, env, 0x193);
        add_item(m, this, 4, s);
        if has_points(m, env) == 0 {
            let s = loc(m, env, 0x18e);
            add_item(m, this, 3, s);
        }
        m.w32(this + 0xc4, 1);
        let step = F(37.0);
        let mut v = rf(m, COL_X);
        for c in 1..7 {
            v = v + step;
            wf(m, COL_X + 4 * c, v);
        }
    } else {
        let n = m.r32(CAREER + 0x27dc) as i32;
        let mut advance = true;
        if n / 2 > 0 {
            let first = m.r32(CAREER + 0x3264);
            if m.r8(first.wrapping_mul(0x84).wrapping_add(RACE) + 0x69) as i8 >= 0 {
                advance = false;
            } else {
                let mut i = 1;
                while i < n / 2 {
                    let r = m.r32(ORDER + 4 * i as u32);
                    if m.r8(r.wrapping_mul(0x84).wrapping_add(RACE) + 0x69) as i8 >= 0 {
                        advance = false;
                        break;
                    }
                    i += 1;
                }
            }
        }
        let line = loc(m, env, if mode == 0 { 0x19f } else { 0x19e });
        if advance {
            let s = loc(m, env, 0x19c);
            call(m, env, WSTR_COPY, &[this + 0x20, s]);
        } else {
            let id = if call(m, env, EVENT_MODE, &[CAREER]) == 2 {
                if has_points(m, env) as i32 > 0 { 0x18d } else { 0xdb5 }
            } else {
                0xdb7
            };
            let s = loc(m, env, id);
            add_item(m, this, 0, s);
            call(m, env, WSTR_COPY, &[this + 0x20, line]);
        }
        for (kind, id) in [(2u32, 0x191u32), (1, 0x190), (4, 0x193), (3, 0x18e)] {
            let s = loc(m, env, id);
            add_item(m, this, kind, s);
        }
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(RACE) as i32 {
        let place = i + 1;
        let r = m.r32(ORDER + 4 * i);
        let suffix = match place {
            1 => 0xe44,
            2 => 0xe54,
            3 => 0xe58,
            _ => 0xe45,
        };
        let s = loc(m, env, suffix);
        call(m, env, WSTR_FORMAT_ASCII, &[scratch, FMT_DS, place, s]);
        set_cell_text(m, env, 0, i, scratch);
        let e = r.wrapping_mul(0x84).wrapping_add(RACE);
        let player = m.r8(e + 0x69) as i8;
        if player == -1 {
            m.w32(ROW_RIDERS + 4 * i, player as i32 as u32);
            let ch = m.r32(e + 0x3c).wrapping_mul(0x38).wrapping_add(CHARACTERS);
            set_cell_ascii(m, env, 1, i, m.r32(ch + 4));
        } else {
            m.w32(ROW_RIDERS + 4 * i, 0);
            set_cell_ascii(m, env, 1, i, e + 8);
        }
        set_cell_ascii(m, env, 2, i, CAREER_CELL2);
        let t = m.r32((r << 2).wrapping_add(TIMES));
        call(m, env, FORMAT_TIME, &[t, cell(3, i)]);
        if m.r32(this + 0xc4) != 0 && m.r8(r.wrapping_mul(0x84).wrapping_add(RACE) + 0x69) as i8 >= 0 {
            let rank = call(m, env, CLAMP_RANK, &[i]);
            m.w32(this + 0x1c, rank);
            medal_verdict(m, env, this);
        }
        i += 1;
    }
}

/// `Results_SetMedalVerdict` 0x1b1ba8(this): the verdict line from the career's rank for the event (0x163e28 on
/// its +0x27dc+0x320 and +0x31c): 3 → 0x249; else under the medal's rank (+0x1c) → 0x248, at or past it with rank
/// 0 → 0x247, otherwise 0x246
pub fn medal_verdict(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let worse = loc(m, env, 0x248);
    let other = loc(m, env, 0x246);
    let first = loc(m, env, 0x247);
    let none = loc(m, env, 0x249);
    let ev = CAREER + 0x27dc;
    let v = call(m, env, MEDAL_RANK, &[CAREER, m.r32(ev + 0x320), m.r32(ev + 0x31c)]) as i32;
    let rank = m.r32(this + 0x1c) as i32;
    let s = if v == 3 {
        none
    } else if v < rank {
        worse
    } else if rank == 0 {
        first
    } else {
        other
    };
    call(m, env, WSTR_COPY, &[this + 0x20, s]);
}

/// The finishers' rows of the career and standings pages: each place ("%d%s" with "st", "nd", "rd", "th" into
/// `scratch`), the rider's row-rider mark (−1 for one that is not a player's, else 0)
fn place_cell(m: &mut Mem, env: &mut dyn TextBoxEnv, i: u32, scratch: u32) {
    let place = i + 1;
    let suffix = match place {
        1 => 0xe44,
        2 => 0xe54,
        3 => 0xe58,
        _ => 0xe45,
    };
    let s = loc(m, env, suffix);
    call(m, env, WSTR_FORMAT_ASCII, &[scratch, FMT_DS, place, s]);
    set_cell_text(m, env, 0, i, scratch);
}

/// A rider's row's verdict: with the verdict shown and the rider a player's, the rank of the place set and the
/// verdict made
fn row_verdict(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, r: u32, i: u32) {
    if m.r32(this + 0xc4) != 0 && m.r8(r.wrapping_mul(0x84).wrapping_add(RACE) + 0x69) as i8 >= 0 {
        let rank = call(m, env, CLAMP_RANK, &[i]);
        m.w32(this + 0x1c, rank);
        medal_verdict(m, env, this);
    }
}

/// `Results_BuildStandingsPage` 0x1b0188(this): laid out for one player (from the sprite's x + 79, rows 18 apart;
/// five columns, the rows' names 0xe75, 0xe5d, 0xe5c, 0xe61) or for more (x + 87, 16 apart; six columns, names on
/// rows 0, 1 and 4); the page's numbers (4 or 5 and the race's riders); the menu "continue" (0xdb5), 0x191,
/// 0x190, 0x193, 0x18e; the title 0xe57. Then each finisher's row: its place, its own name, its time (in the fourth
/// cell for one player, the fifth for more), the verdict.
pub fn standings_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let hud = m.r32(this + 4);
    let sprites = m.r32(m.r32(hud + 0x42c));
    let r = call(m, env, GET_RECT, &[sprites, 0x37]);
    let x0 = rf(m, r + 8) + F(10.0);
    let one = call(m, env, COUNT_PROFILES, &[CAREER]) == 1;
    let page = m.r32(this);
    if one {
        set_row_layout(m, page, ((x0 + F(7.0)) + F(23.0)) + F(39.0), F(18.0));
    } else {
        set_row_layout(m, page, (((x0 + F(7.0)) + F(23.0)) + F(39.0)) + F(8.0), F(16.0));
    }
    let kind = if call(m, env, COUNT_PROFILES, &[CAREER]) == 1 {
        m.w32(COL_ALIGN + 0xc, 2);
        for (i, v) in [0x42a2_0000u32, 0x4324_0000, 0x4392_0000, 0x43f6_0000, 0x4411_8000].into_iter().enumerate() {
            m.w32(COL_RIGHT + 4 * i as u32, v);
        }
        m.w32(COL_ALIGN, 0);
        let page = m.r32(this);
        m.w32(page + 0x50, 1);
        m.w32(COL_ALIGN + 4, 0);
        m.w32(COL_ALIGN + 8, 0);
        for (row, id) in [(0u32, 0xe75u32), (1, 0xe5d), (2, 0xe5c), (3, 0xe61)] {
            let s = loc(m, env, id);
            set_row_name(m, env, row, s);
        }
        4
    } else {
        m.w32(COL_ALIGN + 0x10, 2);
        for (i, v) in [0x4248_0000u32, 0x4305_0000, 0x4382_8000, 0x43be_8000, 0x43f5_8000, 0x4411_4000].into_iter().enumerate() {
            m.w32(COL_RIGHT + 4 * i as u32, v);
        }
        m.w32(COL_ALIGN, 0);
        let page = m.r32(this);
        m.w32(page + 0x50, 1);
        for o in [4, 8, 0xc] {
            m.w32(COL_ALIGN + o, 0);
        }
        for (row, id) in [(0u32, 0xe75u32), (1, 0xe5d), (4, 0xe61)] {
            let s = loc(m, env, id);
            set_row_name(m, env, row, s);
        }
        5
    };
    let page = m.r32(this);
    m.w32(page + 8, kind);
    m.w32(page + 4, m.r32(RACE));
    for (kind, id) in [(0u32, 0xdb5u32), (2, 0x191), (1, 0x190), (4, 0x193), (3, 0x18e)] {
        let s = loc(m, env, id);
        add_item(m, this, kind, s);
    }
    let t = loc(m, env, 0xe57);
    set_title(m, env, t);
    let mut i = 0u32;
    while (i as i32) < m.r32(RACE) as i32 {
        let r = m.r32(ORDER + 4 * i);
        place_cell(m, env, i, scratch);
        let e = r.wrapping_mul(0x84).wrapping_add(RACE);
        let player = m.r8(e + 0x69) as i8;
        m.w32(ROW_RIDERS + 4 * i, if player == -1 { player as i32 as u32 } else { 0 });
        set_cell_ascii(m, env, 1, i, e + 8);
        let t = m.r32((r << 2).wrapping_add(TIMES));
        let row = if call(m, env, COUNT_PROFILES, &[CAREER]) == 1 { 3 } else { 4 };
        call(m, env, FORMAT_TIME, &[t, cell(row, i)]);
        row_verdict(m, env, this, r, i);
        i += 1;
    }
}

/// `Results_BuildTimeChallengePage` 0x1b07f8(this): laid out from the sprite's x + 119 (rows 3 apart), no numbers,
/// the menu "continue" (0xdb5), 0x191, 0x190, 0x18e, the title 0xe9c
pub fn time_challenge_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let hud = m.r32(this + 4);
    let sprites = m.r32(m.r32(hud + 0x42c));
    let r = call(m, env, GET_RECT, &[sprites, 0x37]);
    let x = (((rf(m, r + 8) + F(50.0)) + F(7.0)) + F(23.0)) + F(39.0);
    set_row_layout(m, m.r32(this), x, F(3.0));
    let page = m.r32(this);
    m.w32(page + 8, 0);
    m.w32(page + 4, 0);
    for (kind, id) in [(0u32, 0xdb5u32), (2, 0x191), (1, 0x190), (3, 0x18e)] {
        let s = loc(m, env, id);
        add_item(m, this, kind, s);
    }
    let t = loc(m, env, 0xe9c);
    set_title(m, env, t);
}

/// A player's stats block (0xbc bytes): the career's +0x3294, by the rider's index at the career's +0x27e0 (0x84
/// apart)
fn stats_block(m: &Mem, rider: u32) -> u32 {
    let idx = m.r32(rider.wrapping_mul(0x84).wrapping_add(CAREER) + 0x27e0);
    idx.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3294
}

/// A float × 100 made whole (`cvt.w.s`), as a time's argument
fn hundredths(v: F) -> u32 {
    crate::sndlib::cvt_w((v * F(100.0)).0) as u32
}

/// `Results_BuildRiderStatsRows` 0x1b0998(this, who): `who` is a player (+0) and a row (+4). The row gets the
/// labels 0xdc4, 0x1f6, 0x1fb, 0x1fc, 0x1fe, 0x1fa; the next row the player's stats (the block's +0x7c, +0 / 180,
/// +4 / 360, +8, +0xc, and the time +0x5c). 6
pub fn rider_stats_rows(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, who: u32) -> u32 {
    let row = m.r32(who + 4);
    let rider = call(m, env, FIND_RIDER, &[CAREER, m.r32(who)]);
    let blk = stats_block(m, rider);
    let _ = this;
    for (col, id) in [0xdc4u32, 0x1f6, 0x1fb, 0x1fc, 0x1fe, 0x1fa].into_iter().enumerate() {
        let s = loc(m, env, id);
        set_cell_text(m, env, row, col as u32, s);
    }
    let row = row.wrapping_add(1);
    set_cell_int(m, env, row, 0, m.r32(blk + 0x7c));
    set_cell_int(m, env, row, 1, (m.r32(blk) as i32).wrapping_div(180) as u32);
    set_cell_int(m, env, row, 2, (m.r32(blk + 4) as i32).wrapping_div(360) as u32);
    set_cell_int(m, env, row, 3, m.r32(blk + 8));
    set_cell_int(m, env, row, 4, m.r32(blk + 0xc));
    call(m, env, FORMAT_TIME, &[hundredths(rf(m, blk + 0x5c)), cell(row, 5)]);
    6
}

/// `Results_AddRiderStats` 0x1b1278(this, who, col): from column `col` on, the row gets the labels 0xde6, 0x1f8,
/// 0x202, 0x203, 0x1fa, 0x1f7, 0x1fd, 0x1ff, the cell string, 0x201; the next row the player's stats: the block's
/// +0x7c, its speed (+0x68 / +0x6c, × 0.6214 in miles when the career's +0x1680 is set; "%d%s" with the unit), +0x10,
/// +0x1c, four times (+0x5c, +0x50, +0x54, +0x58), the cell string, and the race time of the rider's character (the
/// career's +0x327c by the race rider's +4). The column after the last.
pub fn add_rider_stats(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, who: u32, col: u32, scratch: u32) -> u32 {
    let _ = this;
    let rider = call(m, env, FIND_RIDER, &[CAREER, m.r32(who)]);
    let row = m.r32(who + 4);
    let blk = stats_block(m, rider);
    let labels = [0xde6u32, 0x1f8, 0x202, 0x203, 0x1fa, 0x1f7, 0x1fd, 0x1ff];
    for (k, id) in labels.into_iter().enumerate() {
        let s = loc(m, env, id);
        set_cell_text(m, env, row, col.wrapping_add(k as u32), s);
    }
    set_cell_ascii(m, env, row, col.wrapping_add(8), CAREER_CELL2);
    let s = loc(m, env, 0x201);
    set_cell_text(m, env, row, col.wrapping_add(9), s);
    let row = row.wrapping_add(1);
    set_cell_int(m, env, row, col, m.r32(blk + 0x7c));
    let first = m.r8(CAREER_CELL2);
    m.w8(scratch, first);
    let speed = div(rf(m, blk + 0x68), rf(m, blk + 0x6c));
    if m.r32(CAREER + 0x1680) != 0 {
        let v = crate::sndlib::cvt_w((speed * k(0x3f1e_f9db)).0) as u32;
        call(m, env, SPRINTF, &[scratch, FMT_DS, v, UNIT_MPH]);
    } else {
        let v = crate::sndlib::cvt_w(speed.0) as u32;
        call(m, env, SPRINTF, &[scratch, FMT_DS, v, UNIT_KMH]);
    }
    set_cell_ascii(m, env, row, col.wrapping_add(1), scratch);
    set_cell_int(m, env, row, col.wrapping_add(2), m.r32(blk + 0x10));
    set_cell_int(m, env, row, col.wrapping_add(3), m.r32(blk + 0x1c));
    for (k, o) in [0x5cu32, 0x50, 0x54, 0x58].into_iter().enumerate() {
        call(m, env, FORMAT_TIME, &[hundredths(rf(m, blk + o)), cell(row, col.wrapping_add(4 + k as u32))]);
    }
    set_cell_ascii(m, env, row, col.wrapping_add(8), CAREER_CELL2);
    let ch = m.r32(rider.wrapping_mul(0x84).wrapping_add(RACE) + 4);
    let t = m.r32((ch << 2).wrapping_add(CAREER) + 0x327c);
    call(m, env, FORMAT_TIME, &[t, cell(row, col.wrapping_add(9))]);
    col.wrapping_add(10)
}

/// The two players' stats records the medal and stats pages fill (0x78 apart: +0 the player, +4 the row; the lines
/// the pages clear follow each)
pub const WHO: u32 = 0x3382d0;

/// Columns' places from `COL_RIGHT`'s first on, each the given step past the one before
fn space_columns(m: &mut Mem, steps: &[F]) {
    let mut v = rf(m, COL_RIGHT);
    for (i, &s) in steps.iter().enumerate() {
        v = v + s;
        wf(m, COL_RIGHT + 4 * (i as u32 + 1), v);
    }
}

/// `Results_BuildMedalPage` 0x1b0bb8(this): the overlay's box (363 by 320), the medal flag, the rows' riders −1.
/// The menu: with the show-off's final unlock "continue" (0x18d), "retry", "save"; else in the event's state 3
/// "continue" (0xdb5), 0x191, "retry", "save", 0x18e; else "continue" (0x18d) when the player has points to spend,
/// or 0xdb5 and 0x191, then "retry", "save". Each player's trick line formatted (`Score_FormatTrickName` into the
/// lines), the page's number 3 · players, the title 0xe56. With one player the medal's rank from the circuit (and the
/// verdict when the state is 5), laid out at 140 with the first column at 191; with more, the best score's players
/// listed in the overlay's +0xd0 (+0xe8 how many; none when all tie) and their rows' ids cleared, the first column at
/// 20. Then the stats rows of the first player (and the second, with two profiles) and the columns spaced.
pub fn medal_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let ev = CAREER + 0x27dc;
    let page = m.r32(this);
    wf(m, this + 0x14, k(0x43b5_8000));
    m.w32(page + 0x44, 0);
    wf(m, this + 0x18, k(0x43a0_0000));
    let showoff = m.r32(this + 0xc);
    m.w32(this + 0xc8, 1);
    for k in (0..16).rev() {
        m.w32(ROW_RIDERS + 4 * k, 0xffff_ffff);
    }
    for k in (0..8).rev() {
        m.w32(ROW_IDS + 4 * k, 0xffff_ffff);
    }
    let items: &[(u32, u32)] = if showoff != 0 {
        &[(0, 0x18d), (1, 0x190), (4, 0x193)]
    } else if m.r32(ev + 0x320) == 3 {
        &[(0, 0xdb5), (2, 0x191), (1, 0x190), (4, 0x193), (3, 0x18e)]
    } else if has_points(m, env) as i32 > 0 {
        &[(0, 0x18d), (1, 0x190), (4, 0x193)]
    } else {
        &[(0, 0xdb5), (2, 0x191), (1, 0x190), (4, 0x193)]
    };
    for &(kind, id) in items {
        let s = loc(m, env, id);
        add_item(m, this, kind, s);
    }
    let mut i = 0u32;
    while (i as i32) < m.r32(ev) as i32 {
        let rider = call(m, env, FIND_RIDER, &[CAREER, i]);
        let idx = m.r32(rider.wrapping_mul(0x84).wrapping_add(ev + 4));
        let blk = idx.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3308;
        call(m, env, TRICK_NAME, &[i.wrapping_mul(0x78).wrapping_add(LINES), blk]);
        i += 1;
    }
    let n = m.r32(ev);
    let page = m.r32(this);
    m.w32(page + 8, n.wrapping_mul(3));
    let t = loc(m, env, 0xe56);
    set_title(m, env, t);
    if m.r32(ev) == 1 {
        let rank = call(m, env, CIRCUIT_MEDAL, &[m.r32(ORDER + 0xac), m.r32(ev + 0x31c)]);
        m.w32(this + 0x1c, rank);
        if m.r32(ev + 0x320) == 5 {
            medal_verdict(m, env, this);
        }
        set_row_layout(m, m.r32(this), F(140.0), F(3.0));
        wf(m, COL_RIGHT, F(191.0));
    } else {
        let mut best = -1i32;
        set_row_layout(m, m.r32(this), F(140.0), F(3.0));
        wf(m, COL_RIGHT, F(20.0));
        let mut i = 0u32;
        while (i as i32) < m.r32(ev) as i32 {
            let rider = call(m, env, FIND_RIDER, &[CAREER, i]);
            let idx = m.r32(rider.wrapping_mul(0x84).wrapping_add(ev + 4));
            let blk = idx.wrapping_mul(0xbc).wrapping_add(CAREER) + 0x3294;
            let score = m.r32(blk + 0x7c) as i32;
            if best < score {
                m.w32(this + 0xd0, i);
                m.w32(this + 0xe8, 1);
                best = m.r32(blk + 0x7c) as i32;
            } else if score == best {
                let n = m.r32(this + 0xe8);
                m.w32((n << 2).wrapping_add(this) + 0xd0, i);
                let n = m.r32(this + 0xe8);
                m.w32(this + 0xe8, n.wrapping_add(1));
            }
            i += 1;
        }
        let n = m.r32(this + 0xe8);
        if n == m.r32(ev) {
            m.w32(this + 0xe8, 0);
        } else if n as i32 > 0 {
            let mut i = 0u32;
            while (i as i32) < n as i32 {
                match m.r32(this + 0xd0 + 4 * i) {
                    0 => m.w32(ROW_IDS + 4, 0),
                    1 => m.w32(ROW_IDS + 0x10, 0),
                    _ => {}
                }
                i += 1;
            }
        }
    }
    m.w32(WHO + 0x7c, 3);
    m.w32(ROW_RIDERS, 0);
    m.w32(WHO + 4, 0);
    let v = rider_stats_rows(m, env, this, WHO);
    let page = m.r32(this);
    m.w32(page + 4, v);
    if call(m, env, COUNT_PROFILES, &[CAREER]) == 2 {
        rider_stats_rows(m, env, this, WHO + 0x78);
    }
    let one = m.r32(ev) == 1;
    let (a, b, c) = if one { (F(160.0), F(90.0), F(32.0)) } else { (F(140.0), F(113.0), F(32.0)) };
    m.w32(COL_ALIGN + 0x14, 2);
    m.w32(COL_ALIGN, 0);
    space_columns(m, &[a, b, c, a, b, c]);
    m.w32(COL_ALIGN + 4, 2);
    m.w32(COL_ALIGN + 8, 2);
    m.w32(COL_ALIGN + 0xc, 0);
    m.w32(COL_ALIGN + 0x10, 2);
}

/// `Results_BuildStatsPage` 0x1b16f0(this): laid out from the sprite's x + 79 (rows 3 apart), the page's riders'
/// rows −1, the menu 0xdb6, 0x191, "retry", 0x18e, the title 0xe46; with one rider the first column at 192 and the
/// number 2, else 70 and 5; each player's stats from column 1 (the second's with more than one rider) and the
/// columns spaced (160, 90, 40 for one; 151, 80, 30 for more)
pub fn stats_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, scratch: u32) {
    let hud = m.r32(this + 4);
    let page = m.r32(this);
    let sprites = m.r32(m.r32(hud + 0x42c));
    m.w32(page + 0x44, 0);
    let r = call(m, env, GET_RECT, &[sprites, 0x37]);
    let x = (((rf(m, r + 8) + F(10.0)) + F(7.0)) + F(23.0)) + F(39.0);
    set_row_layout(m, m.r32(this), x, F(3.0));
    let page = m.r32(this);
    let n = m.r32(page + 4) as i32;
    let mut i = 0;
    while i < n {
        m.w32(ROW_RIDERS + 4 * i as u32, 0xffff_ffff);
        i += 1;
    }
    for k in (0..8).rev() {
        m.w32(ROW_IDS + 4 * k, 0xffff_ffff);
    }
    for (kind, id) in [(0u32, 0xdb6u32), (2, 0x191), (1, 0x190), (3, 0x18e)] {
        let s = loc(m, env, id);
        add_item(m, this, kind, s);
    }
    let t = loc(m, env, 0xe46);
    set_title(m, env, t);
    let page = m.r32(this);
    if m.r32(RACE) == 1 {
        wf(m, COL_RIGHT, F(192.0));
        m.w32(page + 8, 2);
    } else {
        wf(m, COL_RIGHT, F(70.0));
        m.w32(page + 8, 5);
    }
    m.w32(WHO + 4, 0);
    m.w32(WHO + 0x7c, 3);
    if m.r32(RACE) == 1 {
        let v = add_rider_stats(m, env, this, WHO, 1, scratch);
        let page = m.r32(this);
        m.w32(page + 4, v);
        space_columns(m, &[F(160.0), F(90.0), F(40.0), F(160.0), F(90.0)]);
    } else {
        let v = add_rider_stats(m, env, this, WHO, 1, scratch);
        let page = m.r32(this);
        m.w32(page + 4, v);
        add_rider_stats(m, env, this, WHO + 0x78, 1, scratch);
        space_columns(m, &[F(151.0), F(80.0), F(30.0), F(151.0), F(80.0)]);
    }
    m.w32(COL_ALIGN + 0x10, 2);
    m.w32(COL_ALIGN, 0);
    m.w32(COL_ALIGN + 8, 1);
    m.w32(COL_ALIGN + 4, 2);
    m.w32(COL_ALIGN + 0xc, 0);
}

/// `Results_BuildLessonPage` 0x1b1aa8(this): laid out at 142 (rows 3 apart), the first two columns at 131 and 400,
/// the title 0xe47, the number 1, the menu 0x191 (written as the first item in place) and 0x18e
pub fn lesson_page(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    let page = m.r32(this);
    m.w32(page + 0x44, 0);
    set_row_layout(m, page, F(142.0), F(3.0));
    wf(m, COL_RIGHT, F(131.0));
    wf(m, COL_RIGHT + 4, F(400.0));
    let t = loc(m, env, 0xe47);
    set_title(m, env, t);
    let page = m.r32(this);
    m.w32(page + 8, 1);
    m.w32(page + 0x34, 0);
    m.w32(page + 4, 0);
    let s = loc(m, env, 0x191);
    let page = m.r32(this);
    let n = m.r32(page + 0x34).wrapping_add(1);
    m.w32(page + 0xc, 2);
    m.w32(page + 0x34, n);
    m.w32(page + 0x10, s);
    let s = loc(m, env, 0x18e);
    let page = m.r32(this);
    let e = (n << 3).wrapping_add(page) + 0xc;
    m.w32(e, 3);
    m.w32(e + 4, s);
    let c = m.r32(page + 0x34);
    m.w32(page + 0x34, c.wrapping_add(1));
}
