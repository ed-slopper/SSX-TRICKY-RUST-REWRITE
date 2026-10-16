//! The in-race HUD drawn by the game's own code (board row G8j): `tricky_game::hudstate` (the HUD overlay's
//! constructor and update), `hudsprites::draw_player_sprites` and `hudtext::draw_player_text` run on a copy of the
//! game's memory, and what they draw (textured quads, strings) is shown in the 640×480 screen.
//!
//! The memory is the game's own where it can be: the HUD's static tables (the meter's bands, the font colour, the
//! view layouts' initialisers, the number and time formats) come from the executable on the player's disc
//! (`TRICKY_DISC`, `/SLUS_203.26`), the strings from `DATA/LANG/AMERICAN.LOC`, the sprites' rectangles from
//! `chars/hud/sprites.json` (what `SpriteSet_InitHudGameRects` 0x1f4440 builds) and the fonts from the disc's SFN
//! files (parsed and measured by `tricky_game::font`; their glyphs drawn by `sfnfont`).
//! The rider, race and world fields the HUD reads are filled from the app's own race every frame, and the race's
//! path set, placings and progress run the game's own code on them.
//!
//! Without the disc, the sheets or the fonts, the app's own HUD (`ui::hud`, `hudsprites`, `sfnfont`) stays.

use crate::sfnfont::{SfnFonts, SfnText, Slot};
use crate::{ui, CharLib, Mode, RiderRes};
use bevy::prelude::*;
use std::collections::HashMap;
use tricky_game::hudsprites::{self, SpritesEnv};
use tricky_game::hudstate::{self, HudEnv};
use tricky_game::pf::Mem;
use tricky_game::{fontprint, hudtext};

/// The game's globals the HUD reads
const GAPP: u32 = 0x337c58;
const FONT_COLOR: u32 = 0x337fe8;
/// The fake objects, in one block above the game's memory
const ARENA: u32 = 0x0100_0000;
const ARENA_SIZE: u32 = 0x6_0000;
const HUD: u32 = ARENA;
const APP: u32 = ARENA + 0x1000;
const RACE: u32 = ARENA + 0x2000;
const WORLD: u32 = ARENA + 0x2400;
const MAN: u32 = ARENA + 0x2500;
const FONT1: u32 = ARENA + 0x2f00;
const FONT2: u32 = ARENA + 0x2f40;
const SET: u32 = ARENA + 0x3000;
/// The two fonts' glyph tables (`Font_ParseGlyphs` allocates them)
const FONT1_GLYPHS: u32 = ARENA + 0x4800;
const FONT2_GLYPHS: u32 = ARENA + 0x6400;
const RIDERS: u32 = ARENA + 0x8000;
const RIDER_SIZE: u32 = 0x6000;
const STRINGS: u32 = ARENA + 0x2_0000;
const STACK_TOP: u32 = ARENA + 0x4_0000;
/// The race's path set (`AIP_LoadPathsFromFile` 0x197da0 fills it): +8 the event paths' count, +0xc their records,
/// +0x10 the main one; its event paths' records and events are kept from here
const PATH_SET: u32 = 0x337fb8;
const EVENT_PATHS: u32 = ARENA + 0x4_0000;
/// The game's ranges copied from the executable: the HUD's globals (bands, colour, tables), the view layouts
/// (bss), the layouts' initialisers and the formats, the settings the constructor reads, the audio object the
/// update's sounds are handed (the app plays none of them), the placings' rubber band tables
const ELF_RANGES: [(u32, u32); 7] = [
    (0x337fe8, 0x200),
    (0x33ca40, 4 * 0x424),
    (0x38d000, 0x1400),
    (0x32ca10, 0x40),
    (0x32db6c, 0x340),
    (0x343050, 4),
    (0x365060, 0x60),
];

/// One thing the game drew
enum Cmd {
    /// `turned`: `Sprite_SubmitQuad` 0x19bdf8, the texture a quarter turn anticlockwise (its right edge on top).
    /// The HUD never turns one (no layout entry's corner and no corner its code passes has 0x10), so it is shown as
    /// drawn
    Quad { sheet: u32, uv: [f32; 4], x: f32, y: f32, w: f32, h: f32, rgba: [f32; 4], layer: u32, turned: bool },
    Text { font: usize, text: String, x: f32, y: f32, scale: f32, rgba: [f32; 4], layer: u32 },
}

#[derive(Component)]
pub struct GameHudRoot;
#[derive(Component)]
pub struct QuadNode(usize);
#[derive(Component)]
pub struct TextNode(usize);

#[derive(Resource, Default)]
pub struct GameHud {
    tried: bool,
    on: bool,
    mem: Option<Mem>,
    loc: Option<tricky_data::loc::LocFile>,
    sheets: Vec<Handle<Image>>,
    tick: f32,
    /// the game mode and course the HUD overlay was built for
    mode: Option<(u32, u32)>,
    /// the disc, for the courses' `.aip`
    disc: String,
    quads: usize,
    texts: usize,
}

fn elf_bytes(elf: &[u8], addr: u32, len: u32) -> Vec<u8> {
    let rd = |o: usize, n: usize| -> u32 { elf.get(o..o + n).map(|b| b.iter().rev().fold(0u32, |a, x| (a << 8) | *x as u32)).unwrap_or(0) };
    let mut out = vec![0u8; len as usize];
    let (phoff, phnum) = (rd(0x1c, 4) as usize, rd(0x2c, 2) as usize);
    for i in 0..phnum {
        let p = phoff + 32 * i;
        let (kind, off, va, fsz) = (rd(p, 4), rd(p + 4, 4), rd(p + 8, 4), rd(p + 16, 4));
        if kind != 1 {
            continue;
        }
        for (k, b) in out.iter_mut().enumerate() {
            let a = addr + k as u32;
            if a >= va && a < va + fsz {
                if let Some(v) = elf.get((off + a - va) as usize) {
                    *b = *v;
                }
            }
        }
    }
    out
}

fn ascii(m: &Mem, mut a: u32) -> String {
    let mut s = String::new();
    while s.len() < 256 {
        let c = m.r8(a);
        if c == 0 {
            break;
        }
        s.push(c as char);
        a += 1;
    }
    s
}
fn wide(m: &Mem, mut a: u32) -> String {
    let mut s = String::new();
    while s.len() < 256 {
        let c = m.r16(a);
        if c == 0 {
            break;
        }
        s.push(char::from_u32(c as u32).unwrap_or('?'));
        a += 2;
    }
    s
}
fn put_ascii(m: &mut Mem, a: u32, s: &str) {
    for (k, b) in s.bytes().chain([0]).enumerate() {
        m.w8(a + k as u32, b);
    }
}
fn put_wide(m: &mut Mem, a: u32, s: &str) {
    for (k, c) in s.chars().map(|c| c as u32 as u16).chain([0]).enumerate() {
        m.w16(a + 2 * k as u32, c);
    }
}
/// The C library's integer formats the HUD uses (`%d`, `%2d`, `%02d`, `%s` taken as nothing), from its arguments
fn format(fmt: &str, args: &[u32]) -> String {
    let (mut out, mut it, mut chars) = (String::new(), args.iter(), fmt.chars().peekable());
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&d) = chars.peek() {
            chars.next();
            if d.is_ascii_alphabetic() || d == '%' {
                spec.push(d);
                break;
            }
            spec.push(d);
        }
        let kind = spec.chars().last().unwrap_or('d');
        let pad: usize = spec.trim_start_matches('0').trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(0);
        let zero = spec.starts_with('0');
        match kind {
            '%' => out.push('%'),
            'd' | 'i' | 'u' => {
                let v = *it.next().unwrap_or(&0) as i32;
                let n = if kind == 'u' { (v as u32).to_string() } else { v.to_string() };
                let fill = if zero { '0' } else { ' ' };
                for _ in n.len()..pad {
                    out.push(fill);
                }
                out.push_str(&n);
            }
            _ => {
                it.next();
            }
        }
    }
    out
}

/// What the ported code calls out to: answered from the app, drawing recorded
struct Env<'a> {
    loc: Option<&'a tricky_data::loc::LocFile>,
    cmds: Vec<Cmd>,
    layer: u32,
    sheet: u32,
    low: u32,
    strings: &'a mut HashMap<u32, u32>,
    next_string: &'a mut u32,
    /// the race time in hundredths (`Boarder_GetRaceTime` 0x11ea28), and whether the rider has finished
    race_time: u32,
    finished: bool,
    progress: [f32; 4],
}

impl Env<'_> {
    fn colour(m: &Mem) -> [f32; 4] {
        [0, 4, 8, 12].map(|k| f32::from_bits(m.r32(FONT_COLOR + k)))
    }
    fn font_index(font: u32) -> usize {
        // gApp+0x5c holds TITLE.SFN and +0x60 MENU.SFN (`Font_ParseSfn` 0x19c858 at 0x17e934 / 0x17e964)
        if font == FONT2 { 1 } else { 0 }
    }
    /// A frame below the current one for a ported function that builds one
    fn frame<R>(&mut self, size: u32, f: impl FnOnce(&mut Self, u32) -> R) -> R {
        let at = self.low;
        self.low -= size;
        let r = f(self, at);
        self.low = at;
        r
    }
}

impl SpritesEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, ints: &[u32], floats: &[f32]) -> (u32, f32) {
        let a = |k: usize| ints.get(k).copied().unwrap_or(0);
        let f = |k: usize| floats.get(k).copied().unwrap_or(0.0);
        match func {
            // the sprites, down to the quads
            0x1c0068 => hudsprites::draw_sprite(m, self, f(0), f(1), f(2), f(3), a(0), a(1), a(2)),
            0x14eed0 => hudsprites::draw_rect_scaled(m, self, a(0), a(1), f(0), f(1), f(2), f(3)),
            0x1fc178 => hudsprites::draw_anchored(m, self, a(0), a(1), f(0), f(1), f(2), f(3)),
            0x1fc728 => hudsprites::draw_tiled(m, self, a(0), a(1), f(0), f(1), f(2), f(3), f(4), f(5)),
            0x19b730 | 0x19bdf8 => {
                let rgba = Self::colour(m);
                self.cmds.push(Cmd::Quad { sheet: self.sheet, uv: [f(4), f(5), f(6), f(7)], x: f(0), y: f(1), w: f(2), h: f(3), rgba, layer: self.layer, turned: func == 0x19bdf8 });
            }
            0x1f9e50 => return (hudsprites::sprite_rect(a(0), a(1) as i32), 0.0),
            0x1a82e8 => {
                let w = self.frame(0x150, |e, sp| hudsprites::draw_number(m, e, f(0), f(1), f(2), f(3), a(0), a(1), a(2), sp));
                return (0, w);
            }
            0x1a6ba0 => hudsprites::format_time(m, self, a(0), a(1)),
            0x1a6c90 => hudsprites::format_time_signed(m, self, a(0), a(1)),
            // the prints, down to the strings
            0x19d3f0 | 0x19d568 => {
                let wide = func == 0x19d568;
                let at = self.low;
                fontprint::print_shadowed(m, self, wide, a(0), a(1), f(0), f(1), f(2), at)
            }
            0x19d6e0 | 0x19d770 => {
                let at = self.low;
                fontprint::print_centred(m, self, func == 0x19d770, a(0), a(1), f(0), f(1), f(2), at)
            }
            0x19d868 => {
                let at = self.low;
                fontprint::print_right(m, self, a(0), a(1), f(0), f(1), f(2), at)
            }
            0x19d800 => {
                let at = self.low;
                fontprint::print_ascii_right(m, self, a(0), a(1), f(0), f(1), f(2), at)
            }
            0x19cf70 | 0x19d0c0 => {
                let text = if func == 0x19d0c0 { wide(m, a(1)) } else { ascii(m, a(1)) };
                let rgba = Self::colour(m);
                self.cmds.push(Cmd::Text { font: Self::font_index(a(0)), text, x: f(0), y: f(1), scale: f(2), rgba, layer: self.layer });
            }
            0x19da30 | 0x19dc78 => return (tricky_game::font::measure(m, func == 0x19dc78, f(0), a(0), a(1), a(2)) as u32, 0.0),
            // the strings
            0x2c5160 => {
                let id = a(1);
                if let Some(&at) = self.strings.get(&id) {
                    return (at, 0.0);
                }
                let s: String = self.loc.and_then(|l| l.get(id)).unwrap_or_default();
                let at = *self.next_string;
                if at + 2 * (s.chars().count() as u32 + 1) < STACK_TOP - 0x1_0000 {
                    put_wide(m, at, &s);
                    *self.next_string = at + 2 * (s.chars().count() as u32 + 1);
                    self.strings.insert(id, at);
                }
                return (at, 0.0);
            }
            0x2fca28 => {
                let s = format(&ascii(m, a(1)), &ints[2.min(ints.len())..]);
                put_ascii(m, a(0), &s);
            }
            // `vsprintf` / `WStr_VFormat` without the caller's variable arguments: none of the HUD's printed strings
            // holds a `%` (the two loc strings that do, 0x116 and 0xe29, are formatted by `WStr_Format` first)
            0x301ca0 => {
                let s = format(&ascii(m, a(1)), &[]);
                put_ascii(m, a(0), &s);
            }
            0x2c48e8 => {
                let s = format(&wide(m, a(1)), &[]);
                put_wide(m, a(0), &s);
            }
            0x17ce38 => {
                let s = format(&wide(m, a(1)), &ints[2.min(ints.len())..]);
                put_wide(m, a(0), &s);
            }
            0x17ce70 => {
                let s = format(&ascii(m, a(1)), &ints[2.min(ints.len())..]);
                put_wide(m, a(0), &s);
            }
            0x17ccf8 => {
                let s = wide(m, a(1));
                put_ascii(m, a(0), &s);
            }
            0x17cc80 => {
                let s = wide(m, a(1));
                put_wide(m, a(0), &s);
            }
            0x17ccb8 => {
                let s = ascii(m, a(1));
                put_wide(m, a(0), &s);
            }
            // the race: the main event path of the course's path set, the app's race time and finish
            0x197ef0 => return (m.r32(PATH_SET + 0x10), 0.0),
            // `Boarder_GetRaceProgress` 0x1170d0 for the drawing rider
            0x1170d0 => return (0, self.progress[0]),
            0x11ea28 => return (self.race_time, 0.0),
            0x11d998 => return (self.finished as u32, 0.0),
            // `Score_GetRankForPoints`: the rank (`score::rank_for_points`) and its colour, the rank's record at
            // 0x338120 (+4..+16; rank 0's below every threshold)
            0x19e7a0 => {
                let rank = tricky_game::score::rank_for_points(a(0) as i32) as u32;
                if a(1) != 0 {
                    for k in 0..4 {
                        let v = m.r32(0x338120 + 20 * rank + 4 + 4 * k);
                        m.w32(a(1) + 4 * k, v);
                    }
                }
                return (rank, 0.0);
            }
            0x156900 => return (tricky_game::grade::icon_tier(a(1) as i32), 0.0),
            _ => {}
        }
        (0, 0.0)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, ints: &[u32], _floats: &[f32]) -> u32 {
        match slot {
            0x1d4 => self.layer = ints.first().copied().unwrap_or(0),
            0x1dc => return self.layer,
            0x1f4 => self.sheet = ints.first().copied().unwrap_or(0),
            _ => {}
        }
        0
    }
}

/// The HUD state's calls: the overlay manager idle (the HUD only draws while the app's race plays), the score from
/// the rider's trick in progress (+0x24 units, +0x28 multiplier) through the ported score functions, no sounds
struct StateEnv {
    units: f32,
    mult: f32,
    /// each rider's race progress (`Boarder_GetRaceProgress`): its distance to the finish (+0x374), cm
    progress: [f32; 4],
}
impl HudEnv for StateEnv {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        match func {
            // `ScreenOverlay_Init`, the HUD overlay's base: the sprite set (gApp+0x728) and the two fonts
            0x1a7d78 => {
                tricky_game::overlays::screen_overlay_init(m, args.first().copied().unwrap_or(0));
                0
            }
            0x155440 => tricky_game::score::calc_points(self.units, self.mult) as u32,
            0x1554c0 => tricky_game::score::calc_base_points(self.units) as u32,
            0x197ef0 => m.r32(PATH_SET + 0x10),
            // `HUD_BuildSplitList` (the constructor's) on the race's path set
            0x1a1968 => {
                hudstate::reset(m, self, args.first().copied().unwrap_or(HUD));
                0
            }
            0x1814a8 => {
                let k = args.get(1).copied().unwrap_or(0);
                RIDERS + RIDER_SIZE * k
            }
            _ => 0,
        }
    }
    fn multiplier(&mut self, _m: &mut Mem, _score: u32) -> f32 {
        tricky_game::score::get_multiplier(self.units)
    }
    fn progress(&mut self, _m: &mut Mem, rider: u32) -> f32 {
        self.progress.get((rider.wrapping_sub(RIDERS) / RIDER_SIZE) as usize).copied().unwrap_or(0.0)
    }
    fn renderer(&mut self, _m: &mut Mem, slot: u32, _args: &[u32]) -> u32 {
        match slot {
            0x5c => 640,
            0x64 => 480,
            _ => 0,
        }
    }
    fn draw_player(&mut self, _m: &mut Mem, _func: u32, _x: f32, _y: f32, _w: f32, _h: f32, _hud: u32, _i: u32) {}
}

/// The race's event paths as `AIP_LoadEventPaths` 0x1983d0 / `AIPath_Load` 0x196f28 lay them out, from the course's
/// `.aip`: the count and records (0x3c bytes: +0 the events' count, +4 the events, +8 the points' count, +0xc the
/// start, +0x1c / +0x28 the box, +0x38 parameter 0, the distance to the finish at the start), each event 16 bytes
/// (the type's place in the table, the value, the start, the end), and the main path. The points themselves are not
/// kept (nothing the HUD calls reads them)
fn build_paths(m: &mut Mem, aip: &tricky_data::aip::Aip) {
    let n = aip.event_paths.len() as u32;
    m.w32(PATH_SET + 8, n);
    m.w32(PATH_SET + 0xc, EVENT_PATHS);
    m.w32(PATH_SET + 0x10, EVENT_PATHS + 0x3c * aip.main_event_path);
    let mut events = EVENT_PATHS + 0x3c * n;
    for (k, p) in aip.event_paths.iter().enumerate() {
        let r = EVENT_PATHS + 0x3c * k as u32;
        if events + 16 * p.events.len() as u32 > EVENT_PATHS + 0x2_0000 {
            break;
        }
        m.w32(r, p.events.len() as u32);
        m.w32(r + 4, events);
        m.w32(r + 8, p.points.len() as u32);
        for (i, v) in p.start.iter().chain(p.v1.iter()).chain(p.v2.iter()).enumerate() {
            let at = [0xc, 0x10, 0x14, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30][i];
            m.w32(r + at, v.to_bits());
        }
        m.w32(r + 0x38, p.param(0).unwrap_or(0));
        for e in &p.events {
            m.w32(events, e.kind);
            m.w32(events + 4, e.value);
            m.w32(events + 8, e.start.to_bits());
            m.w32(events + 12, e.end.to_bits());
            events += 16;
        }
    }
}

impl tricky_game::placing::PlacingEnv for StateEnv {
    fn progress(&mut self, _m: &mut Mem, rider: u32) -> f32 {
        self.progress.get((rider.wrapping_sub(RIDERS) / RIDER_SIZE) as usize).copied().unwrap_or(0.0)
    }
    /// `Sfx_OnPlacingChange`: the app plays no placing sounds
    fn placing_sound(&mut self, _m: &mut Mem, _args: [u32; 6]) {}
}

/// The game's memory for the HUD, from the executable's bytes and the sprite rectangles
fn build(elf: &[u8], rects: &[(u32, [f32; 7])], title: &[u8], menu: &[u8]) -> Mem {
    let mut m = Mem::default();
    for (at, n) in ELF_RANGES {
        m.regions.push((at, elf_bytes(elf, at, n)));
    }
    m.regions.push((GAPP, vec![0; 4]));
    m.regions.push((PATH_SET, vec![0; 0x30]));
    m.regions.push((ARENA, vec![0; ARENA_SIZE as usize]));
    m.w32(GAPP, APP);
    m.w32(APP + 0x5c, FONT1);
    m.w32(APP + 0x60, FONT2);
    // the HUD's sprite set (`SpriteSet_InitHudGameRects` 0x1f4440 fills it), which `ScreenOverlay_Init` hands on
    m.w32(APP + 0x728, SET);
    m.w32(APP + 0x724, 0);
    m.w32(APP + 0x730, RACE);
    m.w32(RACE + 0x1c, WORLD);
    m.w32(RACE + 0x38, MAN);
    m.w32(RACE + 0x298, 1);
    m.w32(RACE + 0x98, 640);
    m.w32(RACE + 0x9c, 480);
    m.w32(RACE + 0xa4, RIDERS);
    for (id, r) in rects {
        let at = SET + 8 + 28 * id;
        m.w32(at, r[0] as u32);
        for k in 1..7 {
            m.w32(at + 4 * k as u32, r[k].to_bits());
        }
    }
    // the fonts as `cGame::Init` 0x17e148 loads them (`Font_ParseSfn` 0x19c858: no added advance or origin, the
    // scales 1.0; then the title font's 1.4 x 1.3 and the menu font's 1.8 x 1.4)
    for (font, glyphs, file, (sx, sy)) in [(FONT1, FONT1_GLYPHS, title, tricky_data::sfn::TITLE_SCALE), (FONT2, FONT2_GLYPHS, menu, tricky_data::sfn::MENU_SCALE)] {
        m.w32(font, 0);
        tricky_game::font::parse_glyphs(&mut m, font, file, 0, 0, glyphs);
        m.w32(font + 0x18, sx.to_bits());
        m.w32(font + 0x1c, sy.to_bits());
    }
    let rd = RIDERS;
    m.w32(rd + 0x464, rd + 0x500);
    m.w32(rd + 0x290, 1);
    // the game's settings: one player, view 0, the HUD option 0 (the mode and the course are set with the
    // constructor)
    m.w32(hudstate::PLAYERS, 1);
    m.w32(hudstate::HUD_OPTION, 0);
    m.w32(hudstate::VIEW_MODE, 0);
    m
}

/// Load the disc's executable and strings, the sheets and the rectangles, and run the HUD overlay's constructor
fn load(hud: &mut GameHud, lib: &CharLib, images: &mut Assets<Image>) -> Option<()> {
    let p = std::env::var("TRICKY_DISC").ok()?;
    hud.disc = p.clone();
    let mut disc = tricky_data::disc::Disc::open(&p).map_err(|e| eprintln!("game hud: disc {p}: {e}")).ok()?;
    let elf = disc.read_path("/SLUS_203.26").ok()??;
    hud.loc = disc.read_path("/DATA/LANG/AMERICAN.LOC").ok().flatten().map(tricky_data::loc::LocFile::new);
    let mut files = tricky_data::big::Files::new(disc);
    let title = files.load("data/fonts/title.sfn").ok()??;
    let menu = files.load("data/fonts/menu.sfn").ok()??;
    let dir = lib.dir.join("hud");
    let mut sheets = Vec::new();
    for n in ["map1", "map4", "hud1"] {
        let img = image::open(dir.join(format!("{n}.png"))).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        sheets.push(images.add(Image::new(
            bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            bevy::render::render_resource::TextureDimension::D2,
            img.into_raw(),
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        )));
    }
    let text = std::fs::read_to_string(dir.join("sprites.json")).ok()?;
    let list: Vec<serde_json::Value> = serde_json::from_str(&text).ok()?;
    let rects: Vec<(u32, [f32; 7])> = list
        .iter()
        .filter_map(|e| {
            let g = |k: &str| e.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let sheet = match e.get("sheet").and_then(|s| s.as_str()).unwrap_or("") {
                s if s.ends_with("map1") => 0.0,
                s if s.ends_with("map4") => 1.0,
                s if s.ends_with("hud1") => 2.0,
                _ => return None,
            };
            let (x, y, w, h) = (g("x"), g("y"), g("w"), g("h"));
            let (dw, dh) = (if g("draw_w") > 0.0 { g("draw_w") } else { w }, if g("draw_h") > 0.0 { g("draw_h") } else { h });
            // a rectangle as `SpriteSet_InitHudGameRects` writes it: the texture, the draw size, v0, u0, u1, v1
            Some((g("id") as u32, [sheet, dw, dh, y, x, x + w, y + h]))
        })
        .collect();
    hud.mem = Some(build(&elf, &rects, &title, &menu));
    hud.sheets = sheets;
    Some(())
}

/// The game's HUD: fill the game's fields from the app's race, update it at the game's 60 ticks a second, draw it,
/// and show what it drew
#[allow(clippy::too_many_arguments)]
pub fn game_hud(
    mut commands: Commands, lib: Res<CharLib>, (rider, race, opponents, list): (Res<RiderRes>, Res<crate::RaceRes>, Res<crate::Opponents>, Res<crate::LevelList>), game: Res<ui::Game>, mode: Res<Mode>, time: Res<Time>,
    fonts: Res<SfnFonts>, mut hud: ResMut<GameHud>, mut images: ResMut<Assets<Image>>, root: Query<Entity, With<ui::HudRoot>>,
    mine: Query<Entity, With<GameHudRoot>>,
    mut quads: Query<(&QuadNode, &mut Node, &mut ImageNode, &mut Visibility, &mut GlobalZIndex), Without<TextNode>>,
    mut texts: Query<(&TextNode, &mut SfnText, &mut Visibility, &mut GlobalZIndex), Without<QuadNode>>,
    mut old: Query<&mut Visibility, (Or<(With<Slot>, With<crate::hudsprites::HudSprite>, With<ui::BoostSeg>)>, Without<QuadNode>, Without<TextNode>)>,
    mut pool: Local<(HashMap<u32, u32>, u32)>,
) {
    let (strings, next_string) = &mut *pool;
    if !hud.tried {
        if !fonts.ready() {
            return;
        }
        hud.tried = true;
        let h = &mut *hud;
        if load(h, &lib, &mut images).is_none() {
            println!("game hud: no disc, sheets or fonts; the app's own HUD stays");
            return;
        }
        if let Ok(root) = root.single() {
            let full = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            commands.entity(root).with_children(|p| {
                p.spawn((full, GameHudRoot));
            });
            hud.on = true;
            println!("game hud: drawn by the game's HUD code");
        }
        return;
    }
    if !hud.on {
        return;
    }
    let Ok(groot) = mine.single() else { return };
    for mut v in old.iter_mut() {
        *v = Visibility::Hidden;
    }
    let r = &rider.0;
    let race = &race.0;
    // each rider's race progress as `Boarder_GetRaceProgress` gives it with no lap done: its distance to the finish
    // (+0x374: the race line's distance less how far along it), from the app's race lines (metres back to cm; 0
    // before the start, when the app has none)
    let cm = |d: f32| if d.is_finite() && d < 1.0e7 { d * 100.0 } else { 0.0 };
    // the finishing times (+0x418: the race time in hundredths at the line, −1 until then)
    let mut finish = [u32::MAX; 4];
    if race.state == tricky_game::race::RaceState::Finished {
        finish[0] = (race.time * 100.0) as u32;
    }
    for (k, o) in opponents.0.iter().take(3).enumerate() {
        if let Some(t) = o.driver.finished {
            finish[k + 1] = (t * 100.0) as u32;
        }
    }
    let mut progress = [0.0f32; 4];
    let mut along = [0.0f32; 4];
    let mut laps = [0i32; 4];
    along[0] = cm(race.prog.dtf);
    laps[0] = race.prog.laps as i32;
    for (k, o) in opponents.0.iter().take(3).enumerate() {
        along[k + 1] = cm(o.prog.dtf);
        laps[k + 1] = o.prog.laps as i32;
    }
    let riding = *mode == Mode::Ride && game.screen == ui::Screen::Playing;
    let GameHud { mem, loc, sheets, tick, mode: built, disc, quads: nq, texts: nt, .. } = &mut *hud;
    let Some(m) = mem.as_mut() else { return };
    // the game mode for the app's event (its menu's "World Circuit - Race" / "- Show-off" / "Free Ride":
    // `courses::Mode` CircuitRace 4, CircuitShowoff 5, Freeride 7) and the course's number from the game's course
    // table by the level folder (the table's file names are the folders' starts: "snow", "mesa", "megaple"; Merqury
    // City's is "merquer"). The HUD overlay is built (its constructor sets the HUD's flags by the mode, the medals by
    // the course) whenever either changes, as entering an event does
    let game_mode = match game.event {
        ui::Event::Race => tricky_game::courses::Mode::CircuitRace,
        ui::Event::ShowOff => tricky_game::courses::Mode::CircuitShowoff,
        ui::Event::FreeRide => tricky_game::courses::Mode::Freeride,
    } as u32;
    let folder = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let course = tricky_game::courses::COURSES
        .iter()
        .find(|c| folder.starts_with(c.file) || (folder.len() >= 4 && c.file.starts_with(&folder[..4])))
        .map(|c| c.number)
        .unwrap_or(0);
    if *built != Some((game_mode, course)) {
        m.w32(hudstate::GAME_MODE, game_mode);
        m.w32(hudstate::LAP_MODE, course);
        // the course's path set from its `.aip` on the disc (none: an empty set)
        m.w32(PATH_SET + 8, 0);
        m.w32(PATH_SET + 0x10, EVENT_PATHS);
        let aip = tricky_data::disc::Disc::open(&*disc).ok().and_then(|d| tricky_data::aip::from_disc(&mut tricky_data::big::Files::new(d), &folder, "aip"));
        if let Some(aip) = aip {
            build_paths(m, &aip);
        }
        hudstate::hud_overlay_new(m, &mut StateEnv { units: 0.0, mult: 0.0, progress }, HUD);
        hudstate::set_view_layout(m, 0);
        *built = Some((game_mode, course));
    }
    if *next_string == 0 {
        *next_string = STRINGS;
    }
    // the app's race into the game's fields
    let rd = RIDERS;
    let racing = race.state != tricky_game::race::RaceState::Finished;
    m.w32(rd + 0x1c, r.boost.clamp(0.0, 1.0).to_bits());
    m.w32(rd + 0x24, r.uber_timer.to_bits());
    for (k, v) in [r.vel.x, r.vel.y, r.vel.z, 0.0].iter().enumerate() {
        // the game's velocity is in centimetres a second
        m.w32(rd + 0x150 + 4 * k as u32, (v * 100.0).to_bits());
    }
    m.w32(rd + 0x418, finish[0]);
    m.w32(rd + 0x41c, 1);
    m.w32(rd + 0x5820 + 0x88, r.letters as u32);
    m.w32(rd + 0x5910, r.multiplier);
    for (k, t) in r.sc.timed.iter().enumerate() {
        let e = rd + 0x597c + 20 * k as u32;
        m.w32(e, t.kind);
        m.w32(e + 4, t.value as u32);
        m.w32(e + 8, t.life.to_bits());
        m.w32(e + 12, t.age.to_bits());
        m.w32(e + 16, t.extra as u32);
    }
    // `Boarder_GetRaceProgress` (`progress::race_progress`): with laps to go (+0x114, the app's count) their laps of
    // the main path's length on top, the lap line (+0x37c) its length less its lap event's (0xd) distance
    let main = m.r32(PATH_SET + 0x10);
    let lap_len = f32::from_bits(m.r32(main + 0x38));
    let mut lap_line = 0.0f32;
    for k in 0..m.r32(main) {
        let e = m.r32(main + 4) + 16 * k;
        if m.r32(e) == 0xd {
            lap_line = lap_len - f32::from_bits(m.r32(e + 8));
        }
    }
    for k in 0..4 {
        let p = tricky_game::progress::Progress { laps: laps[k], along: along[k], line: lap_line, ..Default::default() };
        progress[k] = tricky_game::progress::race_progress(&p, lap_len);
    }
    // the riders: the player, then up to three opponents: what the HUD reads of them (their progress, +0x374 /
    // +0x380, finishing time, player number, human or not; the rubber band's +0x378 is not kept, and the HUD reads
    // none of the band's results)
    let others = opponents.0.len().min(3) as u32;
    m.w32(WORLD + 0x88, 1 + others);
    // the race object's players and computer riders, for `Race_UpdatePlacingsAndRubberBand`
    m.w32(WORLD + 0x7c, 1);
    m.w32(WORLD + 0x80, others);
    for k in 0..=others {
        let o = RIDERS + RIDER_SIZE * k;
        m.w32(WORLD + 0xc4 + 4 * k, o);
        m.w32(RACE + 0xa4 + 0x80 * k, if k == 0 { o } else { 0 });
        m.w32(o + 0x374, progress[k as usize].to_bits());
        m.w32(o + 0x380, progress[k as usize].to_bits());
        // the profile's player number (+0x65): the player's 0, a computer rider's −1 (as `Career_GetPlayerLevel` and
        // `Race_FindRiderByPlayer` take it, and the progress dots read its sign)
        m.w8(o + 0x500 + 0x65, if k == 0 { 0 } else { 0xff });
        if k > 0 {
            m.w32(o + 0x464, o + 0x500);
            m.w32(o + 0x290, 1);
            m.w32(o + 0x418, finish[k as usize]);
            m.w32(o + 0x41c, 0);
        }
    }
    let mut state = StateEnv { units: r.sc.units, mult: r.sc.mult, progress };
    *tick += time.delta_secs();
    let mut n = 0;
    while *tick >= 1.0 / 60.0 && n < 4 {
        *tick -= 1.0 / 60.0;
        // the race's tick, and its placings (every 6th: the riders' places, +0x110) as the game keeps them
        let tick = m.r32(WORLD + 0x18).wrapping_add(1);
        m.w32(WORLD + 0x18, tick);
        tricky_game::placing::update(m, &mut state, WORLD, STACK_TOP - 0x800);
        hudstate::update(m, &mut state, HUD);
        n += 1;
    }
    if *tick >= 1.0 / 60.0 {
        *tick = 0.0;
    }
    let mut env = Env {
        loc: loc.as_ref(),
        cmds: Vec::new(),
        layer: 0,
        sheet: 0,
        low: STACK_TOP - 0x1000,
        strings,
        next_string,
        race_time: (race.time * 100.0) as u32,
        finished: !racing,
        progress,
    };
    if riding {
        hudsprites::draw_player_sprites(m, &mut env, 0.0, 0.0, 640.0, HUD, 0, STACK_TOP - 0x200);
        hudtext::draw_player_text(m, &mut env, 0.0, 0.0, 640.0, 480.0, HUD, 0, STACK_TOP - 0x400);
    }
    let cmds = std::mem::take(&mut env.cmds);
    // TRICKY_HUDDBG: what the game drew, once a second
    if std::env::var("TRICKY_HUDDBG").is_ok() && (time.elapsed_secs() * 1.0).fract() < time.delta_secs() {
        for c in &cmds {
            match c {
                Cmd::Quad { sheet, uv, x, y, w, h, rgba, layer, turned } => println!("hud quad sheet {sheet} uv {uv:?} at ({x}, {y}) {w}x{h} rgba {rgba:?} layer {layer} turned {turned}"),
                Cmd::Text { font, text, x, y, scale, rgba, layer } => println!("hud text {font} {text:?} at ({x}, {y}) x{scale} rgba {rgba:?} layer {layer}"),
            }
        }
        println!("hud flags {:#x}", m.r32(HUD + 0x104));
        let main = m.r32(PATH_SET + 0x10);
        let b = HUD + 0x114;
        println!(
            "hud places {:?} paths {} main {:#x} length {} markers {} progress {:?} ahead {:?} bars {:?}",
            (0..4).map(|k| m.r32(RIDERS + RIDER_SIZE * k + 0x110)).collect::<Vec<_>>(),
            m.r32(PATH_SET + 8),
            main,
            f32::from_bits(m.r32(main + 0x38)),
            m.r32(HUD + 0x2c),
            progress,
            (0..m.r32(HUD + 0x2c).min(6)).map(|k| f32::from_bits(m.r32(b + 0xdc + 4 * k))).collect::<Vec<_>>(),
            (0..4).map(|k| f32::from_bits(m.r32(b + 0xc4 + 4 * k))).collect::<Vec<_>>()
        );
    }
    // show what it drew: a pool of quads and of strings under our root
    let (mut qi, mut ti) = (0usize, 0usize);
    let px = |v: f32| Val::Vw(v / 640.0 * 100.0);
    let py = |v: f32| Val::Vh(v / 480.0 * 100.0);
    let mut want_q = Vec::new();
    let mut want_t = Vec::new();
    for (order, c) in cmds.iter().enumerate() {
        match c {
            Cmd::Quad { .. } => {
                want_q.push((qi, order, c));
                qi += 1;
            }
            Cmd::Text { .. } => {
                want_t.push((ti, order, c));
                ti += 1;
            }
        }
    }
    let z = |layer: u32, order: usize| GlobalZIndex(1000 + layer as i32 * 1000 + order as i32);
    for (i, order, c) in want_q.iter() {
        let Cmd::Quad { sheet, uv, x, y, w, h, rgba, layer, .. } = c else { continue };
        let (u0, u1, v0, v1) = (uv[1], uv[2], uv[0], uv[3]);
        let img = ImageNode {
            image: sheets.get(*sheet as usize).cloned().unwrap_or_default(),
            rect: Some(Rect::new(u0.min(u1), v0.min(v1), u0.max(u1), v0.max(v1))),
            flip_x: u0 > u1,
            flip_y: v0 > v1,
            color: Color::srgba(rgba[1], rgba[2], rgba[3], rgba[0]),
            ..default()
        };
        let node = Node { position_type: PositionType::Absolute, left: px(*x), top: py(*y), width: py(*w), height: py(*h), ..default() };
        if *i >= *nq {
            commands.entity(groot).with_children(|p| {
                p.spawn((img, node, QuadNode(*i), Visibility::Inherited, z(*layer, *order)));
            });
            *nq += 1;
        } else if let Some((_, mut n, mut im, mut v, mut zi)) = quads.iter_mut().find(|q| q.0 .0 == *i) {
            *n = node;
            *im = img;
            *v = Visibility::Inherited;
            *zi = z(*layer, *order);
        }
    }
    for (q, _, _, mut v, _) in quads.iter_mut() {
        if q.0 >= qi {
            *v = Visibility::Hidden;
        }
    }
    for (i, order, c) in want_t.iter() {
        let Cmd::Text { font, text, x, y, scale, rgba, layer } = c else { continue };
        let t = SfnText { font: *font, text: text.clone(), x: *x, y: *y, scale: *scale, color: Color::srgba(rgba[1], rgba[2], rgba[3], rgba[0]), ..default() };
        let full = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        if *i >= *nt {
            commands.entity(groot).with_children(|p| {
                p.spawn((full, t, TextNode(*i), Visibility::Inherited, z(*layer, *order)));
            });
            *nt += 1;
        } else if let Some((_, mut st, mut v, mut zi)) = texts.iter_mut().find(|q| q.0 .0 == *i) {
            if *st != t {
                *st = t;
            }
            *v = Visibility::Inherited;
            let nz = z(*layer, *order);
            if zi.0 != nz.0 {
                *zi = nz;
            }
        }
    }
    for (q, _, mut v, _) in texts.iter_mut() {
        if q.0 >= ti {
            *v = Visibility::Hidden;
        }
    }
}
