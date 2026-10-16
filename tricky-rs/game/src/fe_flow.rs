//! The front end's screen flow as a model (board row G9u1): each screen's items and the next screen by the item and
//! the game state, taken from the ported screens (`fe_title` .. `fe_dvdcontent`, `fe_manager`): which screen each
//! `CreateNextState` makes, the game type the mode select sets as its items are highlighted (vf22), the game mode
//! word the event select sets, the players set by the number-of-players screen and the manager (0x263670,
//! 0x265a58), the character modify's six items and its done for each player. For the app's menus (G9u2) until the
//! game's widgets are drawn; the screens' looks, sounds and cameras stay with the ports.
//!
//! Items carry the game's text ids (the strings in `DATA/LANG/AMERICAN.LOC`); courses and riders carry their numbers.

/// A screen of the flow
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    NumPlayers,
    ModeSelect,
    EventSel,
    DifficultSel,
    VenueSel,
    CharSelect,
    UserName,
    CharAccept,
    CharModify,
    CustomBody,
    CustomBoard,
    RiderProfile,
    TrickBook,
    DvdContent,
}

/// What an item shows: a text id, or a course / rider / outfit / board by its number
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Text(u32),
    Course(u8),
    Rider(u8),
    Outfit(u8),
    Board(u8),
}

/// An item: its label and its number (the item's +0 word in the game)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: Label,
    pub id: u32,
}

/// The screens' title text ids (the base widgets' title each builder passes)
pub fn title_text(s: Screen, player: i8) -> Option<u32> {
    Some(match s {
        Screen::Title => return None,
        Screen::NumPlayers => 0xdac,
        Screen::ModeSelect => 0xdab,
        Screen::EventSel => 0xdaa,
        Screen::DifficultSel => 0xda9,
        Screen::VenueSel => 0xdad,
        Screen::CharSelect | Screen::CharAccept | Screen::CharModify => {
            if player == 1 {
                0xdaf
            } else {
                0xdae
            }
        }
        Screen::CustomBody => 0xb6,
        Screen::CustomBoard => 0xb5,
        Screen::RiderProfile => 0xda6,
        Screen::TrickBook => 0xdb0,
        Screen::UserName => 0xdb1,
        Screen::DvdContent => 0xcfb,
    })
}

/// The title's items (0x3b2108): start, the DVD content
pub const TITLE_ITEMS: [u32; 2] = [0xe70, 0xcf3];
/// The number of players' items (0x3b2a80)
pub const NUM_PLAYERS_ITEMS: [u32; 2] = [0xe0d, 0xe1b];
/// The mode select's items (practice, single event, World Circuit)
pub const MODE_ITEMS: [u32; 3] = [0xe0a, 0xe0b, 0xe0c];
/// The event select's five events (time challenge, freeride, race, showoff, trick tutorial) and the items each game
/// type lists (single event, practice, World Circuit)
pub const EVENT_TEXTS: [u32; 5] = [0xe85, 0xe04, 0xe05, 0xe06, 0xe07];
pub const EVENT_LISTS: [&[u32]; 3] = [&[2, 3, 0], &[1, 4], &[2, 3]];
/// The difficulty select's items (amateur, semi-pro, pro)
pub const DIFFICULTY_ITEMS: [u32; 3] = [0xe03, 0xe09, 0xe08];
/// The venue select's courses in the menu order (the bytes at 0x3a8af0)
pub const VENUE_ORDER: [u8; 10] = [0, 1, 2, 3, 4, 8, 5, 11, 7, 6];
/// The front end's rider order (the bytes at 0x3470b8)
pub const RIDER_ORDER: [u8; 12] = [3, 1, 8, 10, 0, 7, 11, 9, 2, 6, 5, 4];
/// The character accept's item (0x3af6b8) and the character modify's six (0x3af688: continue, outfit, board, rider
/// profile, trick book, user name)
pub const ACCEPT_ITEMS: [u32; 1] = [0xe01];
pub const MODIFY_ITEMS: [u32; 6] = [0xe01, 0xe13, 0xdfa, 0xe16, 0xe1a, 0xe1c];
/// The rider profile's five panels (0x3afac0) and the trick book's six chapters (0x3af6f0)
pub const PROFILE_ITEMS: [u32; 5] = [0xe14, 0xe7c, 0xe17, 0xe86, 0xe15];
pub const CHAPTER_ITEMS: [u32; 6] = [0xdfb, 0xdfc, 0xdfd, 0xdfe, 0xdff, 0xe00];
/// The DVD content's nine items (0x3b2420)
pub const DVD_ITEMS: [u32; 9] = [0xcfb, 0xcf7, 0xcf6, 0xcf9, 0xcf8, 0xcfa, 0xcf5, 0xcfc, 0xcfd];

/// The choices the flow hands to the game when it leaves for a race
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    /// the front end's game type (+0xf0): 0 single event, 1 practice, 2 World Circuit, 3 trick tutorial, 4 freeride
    pub game_type: i32,
    /// the game mode word (0x32de8c): 1 freeride, 2 single race, 3 single showoff, 4 World Circuit race, 5 World
    /// Circuit showoff, 6 trick tutorial, 7 time challenge
    pub game_mode: u32,
    /// the course (0x32de88), its menu place's course number
    pub course: u8,
    /// the difficulty (0x32dea4)
    pub difficulty: u32,
    /// the players (the manager's +4)
    pub players: i8,
    /// each player's rider, outfit and board
    pub rider: [u8; 2],
    pub outfit: [u8; 2],
    pub board: [u8; 2],
}

/// What a pick did
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// a new screen on top
    Opened(Screen),
    /// the screen went back
    Closed,
    /// the flow left for the game with these choices
    Race(Choices),
    /// a DVD film or nothing to do
    Stay,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    screen: Screen,
    cursor: usize,
    /// the player this screen is for (the screens' +0x2e)
    player: i8,
}

/// The flow: a stack of screens with their cursors, and the choices made so far
#[derive(Clone, Debug)]
pub struct Flow {
    stack: Vec<Frame>,
    pub choices: Choices,
    /// the riders that can't be picked yet (by rider number) and the courses (by course number): the career's
    /// locks, set by the app
    pub locked_riders: [bool; 12],
    pub locked_courses: [bool; 12],
    /// the boards that can't be picked yet (the custom board's 0x267c70)
    pub locked_boards: [bool; 12],
    /// the pads in (the manager's 0x265870)
    pub pads: u32,
}

impl Default for Flow {
    fn default() -> Self {
        Flow::new()
    }
}

impl Flow {
    /// The flow on the title
    pub fn new() -> Flow {
        Flow {
            stack: vec![Frame { screen: Screen::Title, cursor: 0, player: 0 }],
            choices: Choices { players: 1, ..Choices::default() },
            locked_riders: [false; 12],
            locked_courses: [false; 12],
            locked_boards: [false; 12],
            pads: 1,
        }
    }

    /// The screen on top
    pub fn screen(&self) -> Screen {
        self.top().screen
    }

    /// The player the screen on top is for
    pub fn player(&self) -> i8 {
        self.top().player
    }

    /// The highlighted item's place
    pub fn cursor(&self) -> usize {
        self.top().cursor
    }

    fn top(&self) -> &Frame {
        self.stack.last().expect("the flow always has a screen")
    }

    fn top_mut(&mut self) -> &mut Frame {
        self.stack.last_mut().expect("the flow always has a screen")
    }

    /// The screens under the top, oldest first
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// The items the screen on top lists
    pub fn items(&self) -> Vec<Item> {
        let text = |ids: &[u32]| ids.iter().enumerate().map(|(k, t)| Item { label: Label::Text(*t), id: k as u32 }).collect();
        match self.screen() {
            Screen::Title => text(&TITLE_ITEMS),
            Screen::NumPlayers => text(&NUM_PLAYERS_ITEMS),
            Screen::ModeSelect => text(&MODE_ITEMS),
            Screen::EventSel => {
                let list: &[u32] = match self.choices.game_type {
                    0 => EVENT_LISTS[0],
                    1 => EVENT_LISTS[1],
                    2 => EVENT_LISTS[2],
                    _ => &[],
                };
                list.iter().map(|&k| Item { label: Label::Text(EVENT_TEXTS[k as usize]), id: k }).collect()
            }
            Screen::DifficultSel => text(&DIFFICULTY_ITEMS),
            Screen::VenueSel => VENUE_ORDER
                .iter()
                .enumerate()
                .map(|(k, c)| Item { label: Label::Course(*c), id: k as u32 })
                .collect(),
            Screen::CharSelect => RIDER_ORDER.iter().map(|&r| Item { label: Label::Rider(r), id: r as u32 }).collect(),
            Screen::CharAccept => text(&ACCEPT_ITEMS),
            Screen::CharModify => text(&MODIFY_ITEMS),
            Screen::CustomBody => (0..7).map(|k| Item { label: Label::Outfit(k), id: k as u32 }).collect(),
            Screen::CustomBoard => (0..12).map(|k| Item { label: Label::Board(k), id: k as u32 }).collect(),
            Screen::RiderProfile => text(&PROFILE_ITEMS),
            Screen::TrickBook => text(&CHAPTER_ITEMS),
            Screen::UserName => vec![Item { label: Label::Text(0xe1c), id: 0 }],
            Screen::DvdContent => text(&DVD_ITEMS),
        }
    }

    /// Whether an item can be picked: a locked rider, course or board can't (the screens' +0x100 enabled flag)
    pub fn enabled(&self, item: &Item) -> bool {
        match item.label {
            Label::Rider(r) => !self.locked_riders.get(r as usize).copied().unwrap_or(true),
            Label::Course(c) => !self.locked_courses.get(c as usize).copied().unwrap_or(true),
            Label::Board(b) => !self.locked_boards.get(b as usize).copied().unwrap_or(true),
            _ => true,
        }
    }

    /// The highlight moved by `step` (wrapping, as the menu lists do); the mode select sets the game type for the
    /// item highlighted (its vf22: practice 1, single event 0, World Circuit 2)
    pub fn step(&mut self, step: i32) {
        let n = self.items().len();
        if n == 0 {
            return;
        }
        let c = self.cursor() as i32;
        self.top_mut().cursor = (c + step).rem_euclid(n as i32) as usize;
        self.highlighted();
    }

    fn highlighted(&mut self) {
        if self.screen() == Screen::ModeSelect {
            self.choices.game_type = match self.cursor() {
                0 => 1,
                1 => 0,
                _ => 2,
            };
        }
    }

    fn open(&mut self, screen: Screen, player: i8) -> Outcome {
        self.stack.push(Frame { screen, cursor: 0, player });
        self.highlighted();
        Outcome::Opened(screen)
    }

    /// The players' next screen (the manager's 0x265a58 with `NumPlayers` for two pads, else one player and the
    /// character select)
    fn after_players(&mut self) -> Outcome {
        if self.pads >= 2 {
            self.open(Screen::NumPlayers, 0)
        } else {
            self.choices.players = 1;
            self.open(Screen::CharSelect, 0)
        }
    }

    /// The highlighted item picked
    pub fn accept(&mut self) -> Outcome {
        let items = self.items();
        let Some(item) = items.get(self.cursor()).copied() else { return Outcome::Stay };
        if !self.enabled(&item) {
            return Outcome::Stay;
        }
        let player = self.player();
        match self.screen() {
            // `cFE3DStateTitle::CreateNextState`: start the mode select, extras the DVD content
            Screen::Title => match item.id {
                0 => self.open(Screen::ModeSelect, 0),
                _ => self.open(Screen::DvdContent, 0),
            },
            // `cFE3DStateNumPlayers::CreateNextState`: the players set, then the character select for the first
            Screen::NumPlayers => {
                self.choices.players = item.id as i8 + 1;
                self.open(Screen::CharSelect, 0)
            }
            // `cFE3DStateModeSelect::CreateNextState`: practice the event select, single event the players, World
            // Circuit one player and the character select
            Screen::ModeSelect => match item.id {
                0 => self.open(Screen::EventSel, 0),
                1 => self.after_players(),
                _ => {
                    self.choices.players = 1;
                    self.open(Screen::CharSelect, 0)
                }
            },
            // `cFE3DStateEventSel::CreateNextState`: the game mode by the event and the game type
            Screen::EventSel => {
                let kind = self.choices.game_type;
                match item.id {
                    0 => {
                        self.choices.game_mode = 7;
                        self.choices.game_type = 0;
                    }
                    1 => {
                        self.choices.game_mode = 1;
                        self.choices.game_type = 4;
                        return self.after_players();
                    }
                    2 => match kind {
                        0 => {
                            self.choices.game_mode = 2;
                            return self.open(Screen::DifficultSel, player);
                        }
                        2 => self.choices.game_mode = 4,
                        _ => {}
                    },
                    3 => match kind {
                        0 => self.choices.game_mode = 3,
                        2 => self.choices.game_mode = 5,
                        _ => {}
                    },
                    _ => {
                        self.choices.game_mode = 6;
                        self.choices.game_type = 3;
                        self.choices.players = 1;
                        return self.open(Screen::CharSelect, 0);
                    }
                }
                self.open(Screen::VenueSel, player)
            }
            // `cFE3DStateDifficultSel::CreateNextState`: the difficulty word, then the venue select
            Screen::DifficultSel => {
                self.choices.difficulty = item.id;
                self.open(Screen::VenueSel, player)
            }
            // `cFE3DStateVenueSel::CreateNextState`: the course, and the race set up
            Screen::VenueSel => {
                self.choices.course = VENUE_ORDER[item.id as usize];
                Outcome::Race(self.choices)
            }
            // `cFE3DStateCharacterSelect::CreateNextState`: the World Circuit's user name, else the accept
            Screen::CharSelect => {
                if let Some(r) = self.choices.rider.get_mut(player.max(0) as usize) {
                    *r = item.id as u8;
                }
                if self.choices.game_type == 2 {
                    self.open(Screen::UserName, player)
                } else {
                    self.open(Screen::CharAccept, player)
                }
            }
            // `cFE3DStateUserName::CreateNextState`: for the World Circuit the accept
            Screen::UserName => {
                if self.choices.game_type == 2 {
                    self.open(Screen::CharAccept, player)
                } else {
                    self.back()
                }
            }
            // `cFE3DStateCharacterAccept::CreateNextState`: the character modify
            Screen::CharAccept => self.open(Screen::CharModify, player),
            // `cFE3DStateCharacterModify::CreateNextState`
            Screen::CharModify => match item.id {
                0 => {
                    if (player as i32) + 1 != self.choices.players as i32 {
                        return self.open(Screen::CharSelect, player + 1);
                    }
                    match self.choices.game_type {
                        4 => self.open(Screen::VenueSel, player),
                        3 => Outcome::Race(self.choices),
                        _ => self.open(Screen::EventSel, player),
                    }
                }
                1 => self.open(Screen::CustomBody, player),
                2 => self.open(Screen::CustomBoard, player),
                3 => self.open(Screen::RiderProfile, player),
                4 => self.open(Screen::TrickBook, player),
                _ => self.open(Screen::UserName, player),
            },
            // the outfit and board saved as the screens' vf26 do, then back
            Screen::CustomBody => {
                if let Some(o) = self.choices.outfit.get_mut(player.max(0) as usize) {
                    *o = item.id as u8;
                }
                self.back()
            }
            Screen::CustomBoard => {
                if let Some(b) = self.choices.board.get_mut(player.max(0) as usize) {
                    *b = item.id as u8;
                }
                self.back()
            }
            // the profile's panels, the trick book's chapters and the DVD's films stay on their screen
            Screen::RiderProfile | Screen::TrickBook | Screen::DvdContent => Outcome::Stay,
        }
    }

    /// Back (the screens' vf21 and the stack's pop): the screen under comes back; the title stays
    pub fn back(&mut self) -> Outcome {
        if self.stack.len() <= 1 {
            return Outcome::Stay;
        }
        self.stack.pop();
        self.highlighted();
        Outcome::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(f: &mut Flow, k: usize) -> Outcome {
        f.top_mut().cursor = k;
        f.highlighted();
        f.accept()
    }

    #[test]
    fn world_circuit_race() {
        let mut f = Flow::new();
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::ModeSelect));
        assert_eq!(pick(&mut f, 2), Outcome::Opened(Screen::CharSelect));
        assert_eq!(f.choices.game_type, 2);
        assert_eq!(pick(&mut f, 4), Outcome::Opened(Screen::UserName));
        assert_eq!(f.choices.rider[0], RIDER_ORDER[4]);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::CharAccept));
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::CharModify));
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::EventSel));
        assert_eq!(f.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::VenueSel));
        assert_eq!(f.choices.game_mode, 4);
        match pick(&mut f, 5) {
            Outcome::Race(c) => {
                assert_eq!(c.course, 8);
                assert_eq!(c.game_mode, 4);
                assert_eq!(c.players, 1);
            }
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn single_event_race_two_players() {
        let mut f = Flow::new();
        f.pads = 2;
        pick(&mut f, 0);
        assert_eq!(pick(&mut f, 1), Outcome::Opened(Screen::NumPlayers));
        assert_eq!(f.choices.game_type, 0);
        assert_eq!(pick(&mut f, 1), Outcome::Opened(Screen::CharSelect));
        assert_eq!(f.choices.players, 2);
        pick(&mut f, 0);
        pick(&mut f, 0);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::CharSelect));
        assert_eq!(f.player(), 1);
        pick(&mut f, 1);
        pick(&mut f, 0);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::EventSel));
        assert_eq!(f.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![2, 3, 0]);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::DifficultSel));
        assert_eq!(pick(&mut f, 2), Outcome::Opened(Screen::VenueSel));
        match pick(&mut f, 0) {
            Outcome::Race(c) => {
                assert_eq!((c.game_mode, c.difficulty, c.players), (2, 2, 2));
                assert_eq!(c.rider, [RIDER_ORDER[0], RIDER_ORDER[1]]);
            }
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn practice_paths() {
        let mut f = Flow::new();
        pick(&mut f, 0);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::EventSel));
        assert_eq!(f.choices.game_type, 1);
        assert_eq!(f.items().iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 4]);
        // the trick tutorial: one player, the rider, then straight into the game
        let mut t = f.clone();
        assert_eq!(pick(&mut t, 1), Outcome::Opened(Screen::CharSelect));
        assert_eq!((t.choices.game_mode, t.choices.game_type), (6, 3));
        pick(&mut t, 0);
        pick(&mut t, 0);
        assert!(matches!(pick(&mut t, 0), Outcome::Race(c) if c.game_mode == 6));
        // freeride: the players, the rider, then the venue
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::CharSelect));
        assert_eq!((f.choices.game_mode, f.choices.game_type), (1, 4));
        pick(&mut f, 0);
        pick(&mut f, 0);
        assert_eq!(pick(&mut f, 0), Outcome::Opened(Screen::VenueSel));
    }

    #[test]
    fn modify_screens_and_back() {
        let mut f = Flow::new();
        pick(&mut f, 0);
        pick(&mut f, 1);
        pick(&mut f, 3);
        pick(&mut f, 0);
        assert_eq!(f.screen(), Screen::CharModify);
        assert_eq!(pick(&mut f, 1), Outcome::Opened(Screen::CustomBody));
        assert_eq!(pick(&mut f, 5), Outcome::Closed);
        assert_eq!(f.choices.outfit[0], 5);
        assert_eq!(pick(&mut f, 2), Outcome::Opened(Screen::CustomBoard));
        f.locked_boards[11] = true;
        assert_eq!(pick(&mut f, 11), Outcome::Stay);
        f.locked_boards[11] = false;
        assert_eq!(pick(&mut f, 11), Outcome::Closed);
        assert_eq!(f.choices.board[0], 11);
        assert_eq!(f.back(), Outcome::Closed);
        assert_eq!(f.screen(), Screen::CharAccept);
        while f.back() == Outcome::Closed {}
        assert_eq!(f.screen(), Screen::Title);
    }

    #[test]
    fn locks_and_wrapping() {
        let mut f = Flow::new();
        pick(&mut f, 0);
        pick(&mut f, 1);
        f.locked_riders[RIDER_ORDER[0] as usize] = true;
        assert_eq!(pick(&mut f, 0), Outcome::Stay);
        f.top_mut().cursor = 0;
        f.step(-1);
        assert_eq!(f.cursor(), 11);
        assert_eq!(f.accept(), Outcome::Opened(Screen::CharAccept));
    }
}
