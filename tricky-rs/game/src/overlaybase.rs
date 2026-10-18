//! The race overlays' constructors, destructors and small virtuals (board row G8n6): GCC 2's virtual-base set up
//! and tear down for the results family (the backdrop at the overlay's +0, its +0x28 the vtable the overlay's
//! class sees it through) and the memory card family (the backdrop at +0x42c, the card UI's own table at +0x428),
//! the screen and backdrop overlays' destructors, the empty or one-line virtuals, `Draw_SolidRect`.
//!
//! When an overlay is part of a whole object (`whole` 0 / the destructor's flags 0), the class's table is copied
//! to the frame and its `this` shifts (half words at given offsets) moved by where the backdrop is in the object
//! (`this` − (the backdrop − its offset)); the backdrop's +0x28 then points at the copy. A destructor with flags
//! sets the backdrop's table and leaves it; flag 2 destroys the backdrop, flag 1 frees the object (0x114f80).

use crate::pf::Mem;
use crate::ps2f::F;
use crate::textbox::TextBoxEnv;

pub const APP: u32 = 0x337c58;
pub const DRAW_COLOUR: u32 = 0x337fe8;
pub const LOC: u32 = 0x2c5160;
pub const SET_COLOUR: u32 = 0x14eee8;
pub const FILLED_RECT: u32 = 0x19b108;
pub const BACKDROP_INIT: u32 = 0x1a8900;
pub const RESULTS_INIT: u32 = 0x1aacb0;
pub const SAVE_INIT: u32 = 0x1b8568;
pub const SCREEN_INIT: u32 = 0x1a7d78;
pub const BACKDROP_DTOR: u32 = 0x1bff78;
pub const SCREEN_DTOR: u32 = 0x1c01e8;
pub const MC_DTOR: u32 = 0x1ce580;
pub const DELETE: u32 = 0x114f80;
pub const ANIM_DTOR: u32 = 0x1f1128;
pub const MEMBER_DTOR: u32 = 0x155058;
pub const SAVE_TO_CARD: u32 = 0x1d2078;
pub const WSTR_SET_ASCII: u32 = 0x17ccb8;
/// The classes' tables as their backdrops see them (and the card UI's own)
pub const RESULTS_TABLE: u32 = 0x38f880;
pub const MC_TABLE: u32 = 0x38f918;
pub const MC_OWN: u32 = 0x38f9b0;
pub const BACKDROP_TABLE: u32 = 0x38fb78;
pub const SCREEN_TABLE: u32 = 0x38fc10;

fn call(m: &mut Mem, env: &mut dyn TextBoxEnv, func: u32, args: &[u32]) -> u32 {
    env.call(m, func, args, &[])
}
fn loc(m: &mut Mem, env: &mut dyn TextBoxEnv, id: u32) -> u32 {
    let t = m.r32(m.r32(APP) + 0x50);
    call(m, env, LOC, &[t, id])
}

/// `len` bytes of `table` copied to `frame`
fn copy(m: &mut Mem, table: u32, frame: u32, len: u32) {
    for i in 0..len / 4 {
        m.w32(frame + 4 * i, m.r32(table + 4 * i));
    }
}

/// The class's table copied to `frame` (0x98 bytes), its shifts at `offs` moved by `delta`, the backdrop's +0x28
/// pointed at the copy
fn table_copy(m: &mut Mem, b: u32, table: u32, offs: &[u32], delta: u32, frame: u32) {
    copy(m, table, frame, 0x98);
    for &o in offs {
        let h = (m.r16(table + o) as u32).wrapping_add(delta);
        m.w16(frame + o, h as u16);
    }
    m.w32(b + 0x28, frame);
}

/// A results-family class: its table, where its backdrop sits, its table's shifts
pub struct ResultsClass {
    pub table: u32,
    pub base: u32,
    pub offs: &'static [u32],
}

pub const UNLOCKED: ResultsClass = ResultsClass { table: 0x38f328, base: 0x268, offs: &[8, 0x68, 0x70, 0x78, 0x80, 0x88] };
pub const HELP_RACE: ResultsClass = ResultsClass { table: 0x38f3c0, base: 0x54, offs: &[8, 0x50, 0x68, 0x70, 0x78, 0x80, 0x88] };
pub const NEXT_RACE: ResultsClass = ResultsClass { table: 0x38f458, base: 0x258, offs: &[8, 0x48, 0x68, 0x70, 0x78, 0x80, 0x88] };
pub const HISCORE_RECORD: ResultsClass = ResultsClass { table: 0x38f4f0, base: 0x118, offs: &[8, 0x68, 0x70, 0x78, 0x80, 0x88] };
pub const VIEW_HISCORES: ResultsClass = ResultsClass { table: 0x38f588, base: 0x54, offs: &[8, 0x68, 0x70, 0x78, 0x80] };
const RESULTS_OFFS: [u32; 3] = [8, 0x70, 0x78];

/// A results-family overlay's destructor (`vf0`, this, flags): its class's table (copied to the frame's +0xa0 with
/// no flags), then the results overlay's (copied to the frame), then the flags'
pub fn results_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, c: &ResultsClass, this: u32, flags: u32, frame: u32) {
    let b = m.r32(this);
    m.w32(b + 0x28, c.table);
    let (free, base) = if flags == 0 {
        let b = m.r32(this);
        table_copy(m, b, c.table, c.offs, this.wrapping_sub(b.wrapping_sub(c.base)), frame + 0xa0);
        (0, 0)
    } else {
        (flags & 1, flags & 2)
    };
    let b = m.r32(this);
    m.w32(b + 0x28, RESULTS_TABLE);
    let bb = m.r32(this);
    table_copy(m, b, RESULTS_TABLE, &RESULTS_OFFS, this.wrapping_sub(bb.wrapping_sub(0x54)), frame);
    if base != 0 {
        backdrop_dtor(m, env, b, 0);
    }
    if free != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// `cResultsOverlay` vf0 0x1bfc70(this, flags): the results overlay's own destructor
pub fn results_overlay_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32, frame: u32) {
    let b = m.r32(this);
    m.w32(b + 0x28, RESULTS_TABLE);
    let (free, base) = if flags == 0 {
        table_copy(m, b, RESULTS_TABLE, &RESULTS_OFFS, this.wrapping_sub(b.wrapping_sub(0x54)), frame);
        (0, 0)
    } else {
        (flags & 1, flags & 2)
    };
    if base != 0 {
        let bd = m.r32(this);
        backdrop_dtor(m, env, bd, 0);
    }
    if free != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// A results-family constructor's set up after `ResultsOverlay_Init`: the backdrop (its own at `base` when
/// `whole`), the results' init, the class's table (copied when not `whole`)
fn results_ctor(m: &mut Mem, env: &mut dyn TextBoxEnv, c: &ResultsClass, this: u32, whole: u32, frame: u32) {
    if whole != 0 {
        m.w32(this, this + c.base);
        call(m, env, BACKDROP_INIT, &[this + c.base]);
    }
    crate::raceoverlays::results_init(m, env, this, 0, frame + 0x200);
    let b = m.r32(this);
    m.w32(b + 0x28, c.table);
    if whole == 0 {
        table_copy(m, b, c.table, c.offs, this.wrapping_sub(b.wrapping_sub(c.base)), frame);
    }
}

/// `NextRaceOverlay_Init` 0x1b3f50(this, whole): the next race's table; its three standings titles (0xe4b, 0xe4c,
/// 0xe4a at 0x338290), no targets, the targets' three texts emptied (+0x118, 0x40 apart)
pub fn next_race_init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, frame: u32) -> u32 {
    results_ctor(m, env, &NEXT_RACE, this, whole, frame);
    for (i, id) in [0xe4bu32, 0xe4c, 0xe4a].into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(0x338290 + 4 * i as u32, s);
    }
    m.w32(this + 0x1d8, 0);
    for i in 0..3 {
        call(m, env, WSTR_SET_ASCII, &[this + 0x118 + 0x40 * i, 0x38e490]);
    }
    this
}

/// `cUnlockedOverlay::cUnlockedOverlay` 0x1b58c0(this, whole): the unlocked overlay's table; the unlocks' single
/// (0x338268) and plural (0x338240) formats, ten each; the kinds' sprites (+0x1d0: 0x58, 0x59, −1, 0x5a, 0x5b, 0x5c,
/// 0x5d, −1, 0x5f, 0x5e)
pub fn unlocked_init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, frame: u32) -> u32 {
    results_ctor(m, env, &UNLOCKED, this, whole, frame);
    let single = [0x23bu32, 0x23d, 0x237, 0x235, 0x233, 0x239, 0xdb3, 0xe1e, 0x22a, 0x23f];
    let plural = [0x23au32, 0x23c, 0x236, 0x234, 0x232, 0x238, 0xdb4, 0xe1f, 0x22a, 0x23e];
    for (i, id) in single.into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(0x338268 + 4 * i as u32, s);
    }
    for (i, id) in plural.into_iter().enumerate() {
        let s = loc(m, env, id);
        m.w32(0x338240 + 4 * i as u32, s);
    }
    for (o, v) in [
        (0x1d0u32, 0x58u32), (0x1d4, 0x59), (0x1dc, 0x5a), (0x1e0, 0x5b), (0x1e4, 0x5c), (0x1e8, 0x5d),
        (0x1ec, 0xffff_ffff), (0x1f0, 0x5f), (0x1f4, 0x5e), (0x1d8, 0xffff_ffff),
    ] {
        m.w32(this + o, v);
    }
    this
}

/// `cHelpRaceOverlay::cHelpRaceOverlay` 0x1bd190(this, whole): the race help's table
pub fn help_race_init(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, whole: u32, frame: u32) -> u32 {
    results_ctor(m, env, &HELP_RACE, this, whole, frame);
    this
}

/// A card-family class: its own table (+0x428), its backdrop's table, where its backdrop sits, the shifts, the
/// field its constructor clears
pub struct CardClass {
    pub own: u32,
    pub table: u32,
    pub base: u32,
    pub offs: &'static [u32],
    pub field: u32,
}

pub const SELECT_EVENT: CardClass = CardClass { own: 0x38e6b0, table: 0x38e618, base: 0x478, offs: &[8, 0x38, 0x70, 0x78, 0x80], field: 0x464 };
pub const SELECT_VENUE: CardClass = CardClass { own: 0x38e910, table: 0x38e878, base: 0x570, offs: &[8, 0x40, 0x70, 0x78, 0x80], field: 0x55c };
pub const POINTS: CardClass = CardClass { own: 0x38eb70, table: 0x38ead8, base: 0x44c, offs: &[8, 0x30, 0x70, 0x78, 0x80, 0x88], field: 0x440 };
pub const PAUSE: CardClass = CardClass { own: 0x38f160, table: 0x38f0c8, base: 0x820, offs: &[8, 0x18, 0x68, 0x70, 0x78, 0x80], field: 0 };

/// The card UI's own tear down (its table at +0x428, its backdrop's copied with its one shift), its base's
/// destructor (0x1ce580), then the flags'
fn card_base_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, free: u32, base: u32, frame: u32) {
    m.w32(this + 0x428, MC_OWN);
    let b = m.r32(this + 0x42c);
    m.w32(b + 0x28, MC_TABLE);
    table_copy(m, b, MC_TABLE, &[8], this.wrapping_sub(b.wrapping_sub(0x438)), frame);
    call(m, env, MC_DTOR, &[this, 0]);
    if base != 0 {
        let bd = m.r32(this + 0x42c);
        backdrop_dtor(m, env, bd, 0);
    }
    if free != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// A card-family overlay's destructor (this, flags): its own tables (copied to the frame's +0xa0 with no flags),
/// (the pause menu's member at +0x7f8 destroyed), then the card UI's
pub fn card_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, c: &CardClass, this: u32, flags: u32, frame: u32) {
    m.w32(this + 0x428, c.own);
    let b = m.r32(this + 0x42c);
    m.w32(b + 0x28, c.table);
    let (free, base) = if flags == 0 {
        let b = m.r32(this + 0x42c);
        table_copy(m, b, c.table, c.offs, this.wrapping_sub(b.wrapping_sub(c.base)), frame + 0xa0);
        (0, 0)
    } else {
        (flags & 1, flags & 2)
    };
    if c.field == 0 {
        call(m, env, MEMBER_DTOR, &[this + 0x7f8, 2]);
    }
    card_base_dtor(m, env, this, free, base, frame);
}

/// `cSaveOverlay::~cSaveOverlay` 0x1bfe00(this, flags): the card UI's own tear down alone
pub fn save_overlay_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32, frame: u32) {
    m.w32(this + 0x428, MC_OWN);
    let b = m.r32(this + 0x42c);
    m.w32(b + 0x28, MC_TABLE);
    let (free, base) = if flags == 0 {
        table_copy(m, b, MC_TABLE, &[8], this.wrapping_sub(b.wrapping_sub(0x438)), frame);
        (0, 0)
    } else {
        (flags & 1, flags & 2)
    };
    call(m, env, MC_DTOR, &[this, 0]);
    if base != 0 {
        let bd = m.r32(this + 0x42c);
        backdrop_dtor(m, env, bd, 0);
    }
    if free != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// A card-family constructor (this, whole): its backdrop (its own at `base` when `whole`), `SaveOverlay_Init`, its
/// backdrop's table (copied when not `whole`), its own table, its field cleared
pub fn card_ctor(m: &mut Mem, env: &mut dyn TextBoxEnv, c: &CardClass, this: u32, whole: u32, frame: u32) -> u32 {
    if whole != 0 {
        m.w32(this + 0x42c, this + c.base);
        call(m, env, BACKDROP_INIT, &[this + c.base]);
    }
    call(m, env, SAVE_INIT, &[this, 0]);
    let b = m.r32(this + 0x42c);
    m.w32(b + 0x28, c.table);
    if whole == 0 {
        table_copy(m, b, c.table, c.offs, this.wrapping_sub(b.wrapping_sub(c.base)), frame);
    }
    m.w32(this + 0x428, c.own);
    m.w32(this + c.field, 0);
    this
}

/// `cViewHiScoresOverlay` vf19 0x1bf580(this, flags): the race results overlay's destructor, an object of two
/// parts (`this`+0 the results part, +4 the card part): with no flags the high scores' table (shifts at 8, 0x20,
/// 0x68..0x88, its backdrop at 0x10c) copied to the frame's +0xa0 and the card part's own table (0x1c8 bytes,
/// shifts 8 and 0x20..0x38, at 0x454) to its +0x140; flag 2 the results part's and the card part's tear down
/// (the card UI's base destructor, the backdrop's); flag 1 frees it
pub fn race_results_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32, frame: u32) {
    let (a, own) = (0x38f7e8u32, 0x38f620u32);
    let card = m.r32(this + 4);
    let b = m.r32(card + 0x42c);
    m.w32(b + 0x28, a);
    m.w32(card + 0x428, own);
    let (free, base) = if flags == 0 {
        let card = m.r32(this + 4);
        let b = m.r32(card + 0x42c);
        table_copy(m, b, a, &[8, 0x20, 0x68, 0x70, 0x78, 0x80, 0x88], this.wrapping_sub(b.wrapping_sub(0x10c)), frame + 0xa0);
        copy(m, own, frame + 0x140, 0x1c8);
        let card = m.r32(this + 4);
        let d = this.wrapping_sub(card.wrapping_sub(0x454));
        for o in [8u32, 0x20, 0x28, 0x30, 0x38] {
            let h = (m.r16(own + o) as u32).wrapping_add(d);
            m.w16(frame + 0x140 + o, h as u16);
        }
        m.w32(card + 0x428, frame + 0x140);
        (0, 0)
    } else {
        (flags & 1, flags & 2)
    };
    if base != 0 {
        let res = m.r32(this);
        let rb = m.r32(res);
        m.w32(rb + 0x28, RESULTS_TABLE);
        copy(m, RESULTS_TABLE, frame, 0x98);
        let rb = m.r32(res);
        let d = res.wrapping_sub(rb.wrapping_sub(0x54));
        let h: Vec<u16> = RESULTS_OFFS.iter().map(|&o| (m.r16(RESULTS_TABLE + o) as u32).wrapping_add(d) as u16).collect();
        m.w32(rb + 0x28, frame);
        let cb = m.r32(card + 0x42c);
        m.w32(card + 0x428, MC_OWN);
        for (&o, &v) in RESULTS_OFFS.iter().zip(&h) {
            m.w16(frame + o, v);
        }
        m.w32(cb + 0x28, MC_TABLE);
        copy(m, MC_TABLE, frame, 0x98);
        let cb = m.r32(card + 0x42c);
        let h8 = (m.r16(MC_TABLE + 8) as u32).wrapping_add(card.wrapping_sub(cb.wrapping_sub(0x438)));
        m.w32(cb + 0x28, frame);
        m.w16(frame + 8, h8 as u16);
        call(m, env, MC_DTOR, &[card, 0]);
        let c2 = m.r32(this + 4);
        let bd = m.r32(c2 + 0x42c);
        backdrop_dtor(m, env, bd, 0);
    }
    if free != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// `cBackdropOverlay::~cBackdropOverlay` 0x1bff78(this, flags): its table, its animation's destructor (+0x94),
/// then the screen overlay's
pub fn backdrop_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this + 0x28, BACKDROP_TABLE);
    call(m, env, ANIM_DTOR, &[this + 0x94, 2]);
    screen_dtor(m, env, this, flags);
}

/// `cScreenOverlay::~cScreenOverlay` 0x1c01e8(this, flags): its table; flag 1 frees it
pub fn screen_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32, flags: u32) {
    m.w32(this + 0x28, SCREEN_TABLE);
    if flags & 1 != 0 {
        call(m, env, DELETE, &[this]);
    }
}

/// The small screen-family overlays' tables (the lesson controller's and the lesson's are backdrops, the others
/// screen overlays)
pub const LESSON_CONTROLLER_TABLE: u32 = 0x38ed38;
pub const LESSON_TABLE: u32 = 0x38edd0;
pub const BLACK_FADE_TABLE: u32 = 0x38ee68;
pub const POP_ASYNC_TABLE: u32 = 0x38ef00;
pub const POP_FINISH_TABLE: u32 = 0x38ef98;
pub const COUNTDOWN_TABLE: u32 = 0x38f030;

/// A backdrop-based overlay's destructor (the lesson controller's 0x1be450, the lesson's 0x1be4f0): its table,
/// then the backdrop's
pub fn backdrop_kind_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, table: u32, this: u32, flags: u32) {
    m.w32(this + 0x28, table);
    backdrop_dtor(m, env, this, flags);
}

/// A backdrop-based overlay's constructor (0x1be468, 0x1be508; not in the index): the backdrop's, its table
pub fn backdrop_kind_ctor(m: &mut Mem, env: &mut dyn TextBoxEnv, table: u32, this: u32) -> u32 {
    call(m, env, BACKDROP_INIT, &[this]);
    m.w32(this + 0x28, table);
    this
}

/// A screen overlay's destructor (the black fade's 0x1be590, the async load's 0x1be638, the pop finish's 0x1be6e8,
/// the countdown's 0x1be788): its table, then the screen overlay's
pub fn screen_kind_dtor(m: &mut Mem, env: &mut dyn TextBoxEnv, table: u32, this: u32, flags: u32) {
    m.w32(this + 0x28, table);
    screen_dtor(m, env, this, flags);
}

/// A screen overlay's constructor (0x1be5a8, 0x1be650, 0x1be700, 0x1be7a0; not in the index): `ScreenOverlay_Init`,
/// its table
pub fn screen_kind_ctor(m: &mut Mem, env: &mut dyn TextBoxEnv, table: u32, this: u32) -> u32 {
    call(m, env, SCREEN_INIT, &[this]);
    m.w32(this + 0x28, table);
    this
}

/// `cPopAsyncLoadOverlay::Reset` 0x1be630(this): +0x2c cleared
pub fn pop_async_reset(m: &mut Mem, this: u32) {
    m.w32(this + 0x2c, 0);
}

/// The virtuals that answer a constant or `this`, or do nothing (`cPopFinishOverlay` vf17 0x1be6d8 1, vf4
/// 0x1be6e0 / 0x1bdd90 / 0x1be0e0 / 0x1be850 / 0x1becd0 / 0x1bef00 / 0x1bf578 `this`, `cPauseMenuOverlay` vf4
/// 0x1be838 / 0x1bf568 / `cScreenOverlay` vf17 0x1c0040 / vf1..vf9 0x1c01a0..0x1c01e0 0, vf5 / vf6 0x1be828 /
/// 0x1be830 / 0x1bf558 / 0x1bf560 / 0x1c0048..0x1c0060 nothing)
pub fn answer_one() -> u32 {
    1
}
pub fn answer_this(this: u32) -> u32 {
    this
}
pub fn answer_zero() -> u32 {
    0
}
pub fn nothing() {}

/// `cPauseMenuOverlay` vf3 0x1be840 / the venue's and event's 0x1bc6a0-alikes: the career saved to the card; the
/// results part's 0x1bf570 saves through its card part (+4)
pub fn save_to_card(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, SAVE_TO_CARD, &[this]);
}
pub fn save_to_card_part(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    call(m, env, SAVE_TO_CARD, &[m.r32(this + 4)]);
}

/// 0x1be848(this) (not in the index): the pause menu's member at +0x7f8
pub fn pause_member(this: u32) -> u32 {
    this + 0x7f8
}

/// 0x1bdd88(this, v) (not in the index): the venue overlay's +0x55c set
pub fn set_venue_word(m: &mut Mem, this: u32, v: u32) {
    m.w32(this + 0x55c, v);
}

/// The pad state's pointer (0x3381e0) and `cSelectNextEventOverlay` vf2 0x1bfdc8: the buttons held now that weren't
/// (+0x10 and not +0xc), its +0x2c cleared
pub const PAD_STATE: u32 = 0x3381e0;
pub fn new_buttons(m: &mut Mem) -> u32 {
    let p = m.r32(PAD_STATE);
    let held = m.r32(p + 0xc);
    let now = m.r32(p + 0x10);
    m.w32(p + 0x2c, 0);
    now & !held
}

/// 0x1bfde8(this) (not in the index): whether a card operation saves (+0x430 is 2); 0x1bfdf8(this, v): +0x434 set
pub fn card_saving(m: &Mem, this: u32) -> u32 {
    (m.r32(this + 0x430) == 2) as u32
}
pub fn set_card_word(m: &mut Mem, this: u32, v: u32) {
    m.w32(this + 0x434, v);
}

/// 0x1bff68(this) (not in the index): whether a dialog is up (+0x328)
pub fn dialog_up(m: &Mem, this: u32) -> u32 {
    (m.r32(this + 0x328) != 0) as u32
}

/// `cScreenOverlay`'s small virtuals: 0x1c0018 its +0x18; vf16 0x1c00d8 shown (+0xc); 0x1c00e8 shown and +0x14;
/// 0x1c0108 shown and +0x10; 0x1c0128 +0x10 / +0x14 set; 0x1c0138 the pad state set; 0x1c0180 / 0x1c0190 +0xc from
/// +0x24 / +0x24 from +0xc; 0x1c0148 (shown) its virtual +0x78 (its table at +0x28)
pub fn screen_word18(m: &Mem, this: u32) -> u32 {
    m.r32(this + 0x18)
}
pub fn screen_shown(m: &Mem, this: u32) -> u32 {
    (m.r32(this + 0xc) != 0) as u32
}
pub fn screen_shown_and(m: &Mem, this: u32, o: u32) -> u32 {
    if m.r32(this + 0xc) == 0 {
        return 0;
    }
    (m.r32(this + o) != 0) as u32
}
pub fn screen_set_pair(m: &mut Mem, this: u32, a: u32, b: u32) {
    m.w32(this + 0x14, b);
    m.w32(this + 0x10, a);
}
pub fn set_pad_state(m: &mut Mem, v: u32) {
    m.w32(PAD_STATE, v);
}
pub fn screen_restore(m: &mut Mem, this: u32) {
    m.w32(this + 0xc, m.r32(this + 0x24));
}
pub fn screen_keep(m: &mut Mem, this: u32) {
    m.w32(this + 0x24, m.r32(this + 0xc));
}
pub fn screen_vf_78(m: &mut Mem, env: &mut dyn TextBoxEnv, this: u32) {
    if m.r32(this + 0xc) == 0 {
        return;
    }
    let vt = m.r32(this + 0x28);
    let me = this.wrapping_add(m.r16(vt + 0x78) as i16 as u32);
    let func = m.r32(vt + 0x7c);
    env.call(m, func, &[me], &[]);
}

/// `Draw_SolidRect` 0x1b8658(_, colour; x, y, w, h): untextured (the renderer's +0x1e0 0, +0x1f0 −1, 0), the colour
/// set, the rectangle filled
pub fn draw_solid_rect(m: &mut Mem, env: &mut dyn TextBoxEnv, colour: u32, r: [F; 4]) {
    let rend = m.r32(m.r32(APP) + 0x724);
    let vt = m.r32(rend + 0x10);
    for (off, args) in [(0x1e0u32, vec![0u32]), (0x1f0, vec![0xffff_ffff, 0])] {
        let me = rend.wrapping_add(m.r16(vt + off) as i16 as u32);
        let func = m.r32(vt + off + 4);
        let mut a = vec![me];
        a.extend(args);
        env.call(m, func, &a, &[]);
    }
    call(m, env, SET_COLOUR, &[DRAW_COLOUR, colour]);
    env.call(m, FILLED_RECT, &[DRAW_COLOUR], &r);
}
