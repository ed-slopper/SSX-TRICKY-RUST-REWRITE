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

use crate::animseq::cvt_s_w;
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
        4 => career_page(m, env, this, scratch),
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
