//! Tables read out of the original game's executable (SLUS_203.26): each rider's attributes and
//! the grab / tweak / uber trick bound to each of the fifteen shoulder-button combinations.
//! Generated; do not edit by hand.

/// Attributes are percentages as the game stores them: (start, fully trained).
pub struct RiderData {
    pub name: &'static str,
    pub edging: (u8, u8), pub speed: (u8, u8), pub stability: (u8, u8), pub tricks: (u8, u8),
    pub jump: u8, pub windup: u8, pub weight: u8,
    /// 0 = BX, 1 = freestyle, 2 = alpine
    pub board: u8,
    pub goofy: bool,
    pub rows: [Row; 15],
}
/// One shoulder-button combination. `uber` is empty when the combination has no uber trick.
pub struct Row { pub grab: &'static str, pub clip: &'static str, pub rate: f32, pub tweak: &'static str, pub uber: &'static str, pub uber_clip: &'static str, pub uber_rate: f32 }

/// Shoulder masks (L1 = 1, R1 = 2, L2 = 4, R2 = 8) in row order.
pub const COMBOS: [u8; 15] = [4, 2, 8, 1, 2 | 8, 1 | 4, 1 | 8, 4 | 8, 1 | 2, 2 | 4, 1 | 2 | 8, 2 | 4 | 8, 1 | 2 | 4, 1 | 4 | 8, 15];

pub const RIDERS: [RiderData; 12] = [
    RiderData { name: "Eddie", edging: (23, 85), speed: (40, 95), stability: (20, 85), tricks: (28, 85), jump: 70, windup: 75, weight: 80, board: 1, goofy: true, rows: [
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Near Miss", uber_clip: "bxUT_OTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Footloose", uber_clip: "bxUT_SKMUTE", uber_rate: 18.0 },
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Indian", uber_clip: "bxUT_MTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nuclear", clip: "bxT_NUCLEAR", rate: 1.5, tweak: "Nuclear REACTOR", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Lein", clip: "bxT_LEIN", rate: 1.5, tweak: "MEAN Lein", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Kaori", edging: (28, 85), speed: (28, 85), stability: (13, 80), tricks: (43, 99), jump: 90, windup: 94, weight: 60, board: 1, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Judo", uber_clip: "bxUT_SKMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "The Rake", uber_clip: "bxUT_OTINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Nothing", uber_clip: "bxUT_MTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Footloose", uber_clip: "bxUT_SKMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Seatbelt", clip: "bxT_SEATBELTAIR", rate: 1.5, tweak: "BUCKLED Seatbelt", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Luther", edging: (23, 85), speed: (25, 85), stability: (50, 99), tricks: (13, 80), jump: 50, windup: 55, weight: 127, board: 0, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "FrogHop", uber_clip: "bxUT_OTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "The Rake", uber_clip: "bxUT_OTINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Nothing", uber_clip: "bxUT_MTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "Bronco Buster", uber_clip: "bxUT_SIGLUT", uber_rate: 20.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nuclear", clip: "bxT_NUCLEAR", rate: 1.5, tweak: "Nuclear REACTOR", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Lein", clip: "bxT_LEIN", rate: 1.5, tweak: "MEAN Lein", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Slob", clip: "bxT_SLOBAIR", rate: 2.0, tweak: "TOTAL Slob", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Mac", edging: (23, 80), speed: (28, 85), stability: (15, 85), tricks: (45, 99), jump: 94, windup: 88, weight: 65, board: 1, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Judo", uber_clip: "bxUT_SKMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Blender", uber_clip: "bxUT_SKINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Benihana", uber_clip: "bxUT_SKSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Footloose", uber_clip: "bxUT_SKMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Lein", clip: "bxT_LEIN", rate: 1.5, tweak: "MEAN Lein", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nuclear", clip: "bxT_NUCLEAR", rate: 1.5, tweak: "Nuclear REACTOR", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Moby", edging: (23, 85), speed: (30, 85), stability: (23, 90), tricks: (35, 90), jump: 70, windup: 80, weight: 75, board: 0, goofy: true, rows: [
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Nothing", uber_clip: "bxUT_MTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Bar Hop", uber_clip: "bxUT_MTMUTE", uber_rate: 18.0 },
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "FrogHop", uber_clip: "bxUT_OTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "SuperMan Barspin", uber_clip: "bxUT_SIGMOB", uber_rate: 20.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Chicken Salad", clip: "bxT_CHICKENSALAD", rate: 2.0, tweak: "RUBBER Chicken Salad", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stalemaskey", clip: "bxT_STALEMASKY", rate: 2.0, tweak: "FRESH Stalemaskey", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Zoe", edging: (28, 85), speed: (33, 90), stability: (20, 85), tricks: (30, 90), jump: 75, windup: 75, weight: 65, board: 0, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Indian", uber_clip: "bxUT_MTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Near Miss", uber_clip: "bxUT_OTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Bar Hop", uber_clip: "bxUT_MTMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "Pommel Me", uber_clip: "bxUT_SIGZOE", uber_rate: 20.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Slob", clip: "bxT_SLOBAIR", rate: 2.0, tweak: "TOTAL Slob", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "JP", edging: (18, 80), speed: (30, 88), stability: (23, 83), tricks: (40, 99), jump: 80, windup: 65, weight: 85, board: 1, goofy: true, rows: [
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Benihana", uber_clip: "bxUT_SKSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Judo", uber_clip: "bxUT_SKMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "The Rake", uber_clip: "bxUT_OTINDY", uber_rate: 18.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Spaghetti", clip: "bxT_SPAGHETTI", rate: 2.0, tweak: "MEATBALLED Spaghetti", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nuclear", clip: "bxT_NUCLEAR", rate: 1.5, tweak: "Nuclear REACTOR", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Chicken Salad", clip: "bxT_CHICKENSALAD", rate: 2.0, tweak: "RUBBER Chicken Salad", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Lein", clip: "bxT_LEIN", rate: 1.5, tweak: "MEAN Lein", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Slob", clip: "bxT_SLOBAIR", rate: 2.0, tweak: "TOTAL Slob", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Elise", edging: (25, 80), speed: (30, 95), stability: (25, 85), tricks: (30, 90), jump: 70, windup: 70, weight: 70, board: 0, goofy: true, rows: [
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Near Miss", uber_clip: "bxUT_OTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Bar Hop", uber_clip: "bxUT_MTMUTE", uber_rate: 18.0 },
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Judo", uber_clip: "bxUT_SKMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "LaLaLa Lock Step", uber_clip: "bxUT_SIGELI", uber_rate: 20.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Iguana", clip: "bxT_IGUANA", rate: 2.0, tweak: "Iguana KABOB", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Stiffy", clip: "bxT_STIFFY", rate: 1.25, tweak: "IFFY Stiffy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Crail", clip: "bxT_CRAIL", rate: 2.0, tweak: "HOLY Crail", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Lein", clip: "bxT_LEIN", rate: 1.5, tweak: "MEAN Lein", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Psymon", edging: (15, 90), speed: (38, 88), stability: (30, 88), tricks: (28, 85), jump: 75, windup: 70, weight: 84, board: 0, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "FrogHop", uber_clip: "bxUT_OTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Nothing", uber_clip: "bxUT_MTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "Guillotine", uber_clip: "bxUT_SIGPSY", uber_rate: 20.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Seatbelt", clip: "bxT_SEATBELTAIR", rate: 1.5, tweak: "BUCKLED Seatbelt", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Seeiah", edging: (23, 85), speed: (33, 90), stability: (18, 75), tricks: (38, 99), jump: 75, windup: 80, weight: 63, board: 1, goofy: true, rows: [
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Benihana", uber_clip: "bxUT_SKSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Indian", uber_clip: "bxUT_MTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Blender", uber_clip: "bxUT_SKINDY", uber_rate: 18.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Seatbelt", clip: "bxT_SEATBELTAIR", rate: 1.5, tweak: "BUCKLED Seatbelt", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Brodi", edging: (20, 90), speed: (43, 99), stability: (33, 90), tricks: (15, 70), jump: 70, windup: 65, weight: 93, board: 2, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Judo", uber_clip: "bxUT_SKMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Nac Nac", uber_clip: "bxUT_MTINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Nothing", uber_clip: "bxUT_MTSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Seatbelt", clip: "bxT_SEATBELTAIR", rate: 1.5, tweak: "BUCKLED Seatbelt", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
    RiderData { name: "Marisol", edging: (28, 90), speed: (40, 95), stability: (13, 80), tricks: (30, 85), jump: 80, windup: 80, weight: 54, board: 2, goofy: false, rows: [
        Row { grab: "Method", clip: "bxT_METHOD", rate: 1.0, tweak: "Method MADNESS", uber: "Indian", uber_clip: "bxUT_MTMETHOD", uber_rate: 18.0 },
        Row { grab: "Indy", clip: "bxT_INDY", rate: 1.0, tweak: "SINFUL Indy", uber: "Blender", uber_clip: "bxUT_SKINDY", uber_rate: 18.0 },
        Row { grab: "Stalefish", clip: "bxT_STALEFISH", rate: 1.0, tweak: "FILET O'Stalefish", uber: "Benihana", uber_clip: "bxUT_SKSTALEFISH", uber_rate: 18.0 },
        Row { grab: "Mute", clip: "bxT_MUTE", rate: 1.0, tweak: "MUTATION", uber: "Torpedo", uber_clip: "bxUT_OTMUTE", uber_rate: 18.0 },
        Row { grab: "Tailgrab", clip: "bxT_TAILGRAB", rate: 1.75, tweak: "Tail WAG", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Nosegrab", clip: "bxT_NOSEGRAB", rate: 1.75, tweak: "NOSEBLEED", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Seatbelt", clip: "bxT_SEATBELTAIR", rate: 1.5, tweak: "BUCKLED Seatbelt", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Flying Squirrel", clip: "bxT_FLYINGSQUIRREL", rate: 1.25, tweak: "SKINNED Flying Squirrel", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Roast Beef", clip: "bxT_ROASTBEEF", rate: 1.5, tweak: "Roast Beef JERKY", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Canadian Bacon", clip: "bxT_CANADIAN", rate: 2.0, tweak: "Canadian BACK Bacon", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Swiss Cheese", clip: "bxT_SWISSCHEESE", rate: 2.0, tweak: "RIPE Swiss Cheese", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Melancholy", clip: "bxT_MELANCHOLY", rate: 2.0, tweak: "GLEEFUL Melancholy", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Japan", clip: "bxT_JAPAN", rate: 1.5, tweak: "MADE IN Japan", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Rocket", clip: "bxT_ROCKET", rate: 2.0, tweak: "Rocket BOOSTER", uber: "", uber_clip: "", uber_rate: 0.0 },
        Row { grab: "Experimental", clip: "bxT_EXPERIMENT", rate: 2.5, tweak: "UNETHICAL Experimental", uber: "", uber_clip: "", uber_rate: 0.0 },
    ] },
];

/// A board: its name, shape (0 = BX, 1 = freestyle, 2 = alpine) and what it adds to edging, speed,
/// stability and tricks (the game mixes 20% board with 80% rider). Twelve per rider, in the game's order.
pub struct Board { pub name: &'static str, pub kind: u8, pub bonus: [u8; 4] }

/// In the same rider order as `RIDERS`.
pub const BOARDS: [[Board; 12]; 12] = [
    // Eddie
    [Board { name: "Old School", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Anthem", kind: 2, bonus: [10, 10, 10, 0] }, Board { name: "Warholinator", kind: 1, bonus: [20, 10, 10, 30] }, Board { name: "Bell Bottom", kind: 1, bonus: [10, 20, 10, 30] }, Board { name: "Heavy Metal", kind: 1, bonus: [30, 30, 30, 30] }, Board { name: "Skunk Work", kind: 1, bonus: [30, 20, 30, 40] }, Board { name: "New Recruit", kind: 0, bonus: [50, 50, 50, 40] }, Board { name: "Grand Funk", kind: 1, bonus: [50, 40, 50, 50] }, Board { name: "Pop & Lock", kind: 1, bonus: [70, 60, 70, 60] }, Board { name: "Groove E", kind: 1, bonus: [70, 50, 70, 70] }, Board { name: "Hip Hugger", kind: 1, bonus: [80, 60, 80, 80] }, Board { name: "UBERBOARD", kind: 1, bonus: [90, 70, 90, 90] }],
    // Kaori
    [Board { name: "Panda Joy", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Lady Love Bug", kind: 2, bonus: [10, 10, 10, 0] }, Board { name: "Skyhopper", kind: 1, bonus: [20, 10, 10, 30] }, Board { name: "Moo Moo 22", kind: 1, bonus: [10, 20, 10, 30] }, Board { name: "Alpha", kind: 1, bonus: [30, 30, 30, 30] }, Board { name: "Candy", kind: 1, bonus: [30, 20, 30, 40] }, Board { name: "Twinkle Star", kind: 0, bonus: [50, 50, 50, 40] }, Board { name: "Kaori-anime", kind: 1, bonus: [50, 40, 50, 50] }, Board { name: "Banana Peeler", kind: 1, bonus: [70, 60, 70, 60] }, Board { name: "Snow Lover", kind: 1, bonus: [70, 50, 70, 70] }, Board { name: "Princess Pie", kind: 1, bonus: [80, 60, 80, 80] }, Board { name: "UBERBOARD", kind: 1, bonus: [90, 70, 90, 90] }],
    // Luther
    [Board { name: "The Mullet", kind: 0, bonus: [10, 10, 0, 10] }, Board { name: "Chitterling", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Phoenix USA", kind: 0, bonus: [20, 20, 20, 10] }, Board { name: "Sweet Potato Pie", kind: 0, bonus: [20, 20, 10, 20] }, Board { name: "The Gipper", kind: 2, bonus: [30, 50, 30, 20] }, Board { name: "Growler", kind: 0, bonus: [30, 40, 30, 30] }, Board { name: "True Grits ", kind: 0, bonus: [60, 50, 60, 40] }, Board { name: "Ham Bone", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "Jug Band Boogie", kind: 0, bonus: [80, 70, 60, 60] }, Board { name: "Swamp Buggy", kind: 0, bonus: [70, 60, 70, 70] }, Board { name: "Gears n Grease", kind: 0, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 0, bonus: [90, 90, 80, 90] }],
    // Mac
    [Board { name: "Wiener", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Hotrod", kind: 2, bonus: [10, 10, 10, 0] }, Board { name: "Radioactive", kind: 1, bonus: [20, 10, 10, 30] }, Board { name: "Spirit", kind: 1, bonus: [10, 20, 10, 30] }, Board { name: "Boo Boo", kind: 0, bonus: [30, 30, 30, 30] }, Board { name: "Flame On", kind: 1, bonus: [30, 20, 30, 40] }, Board { name: "Buzzsaw", kind: 1, bonus: [50, 50, 50, 40] }, Board { name: "AnimMac", kind: 1, bonus: [50, 40, 50, 50] }, Board { name: "VeloCity", kind: 1, bonus: [70, 60, 70, 60] }, Board { name: "Falciform", kind: 1, bonus: [70, 50, 70, 70] }, Board { name: "Privateer", kind: 1, bonus: [80, 60, 80, 80] }, Board { name: "UBERBOARD", kind: 1, bonus: [90, 70, 90, 90] }],
    // Moby
    [Board { name: "Plasmatic", kind: 0, bonus: [10, 10, 0, 10] }, Board { name: "Patriot", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Cutting", kind: 0, bonus: [20, 30, 10, 10] }, Board { name: "Dream On", kind: 0, bonus: [20, 20, 10, 20] }, Board { name: "Essence", kind: 0, bonus: [40, 30, 40, 20] }, Board { name: "Greenblade", kind: 0, bonus: [30, 40, 30, 30] }, Board { name: "Kinoptic", kind: 2, bonus: [50, 70, 60, 30] }, Board { name: "Royal Strait", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "Datum", kind: 0, bonus: [80, 70, 60, 60] }, Board { name: "Sloppy Bite", kind: 0, bonus: [70, 60, 70, 70] }, Board { name: "Hector", kind: 0, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 0, bonus: [90, 90, 80, 90] }],
    // Zoe
    [Board { name: "Seratonin", kind: 0, bonus: [10, 10, 0, 10] }, Board { name: "Anarchy", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Bad Girl", kind: 0, bonus: [20, 20, 20, 10] }, Board { name: "Lethium Surprise", kind: 0, bonus: [20, 20, 10, 20] }, Board { name: "Plastic You", kind: 2, bonus: [30, 50, 30, 20] }, Board { name: "Alien Blues", kind: 0, bonus: [30, 40, 30, 30] }, Board { name: "Chaos", kind: 0, bonus: [60, 50, 60, 40] }, Board { name: "Latency", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "Iridium", kind: 0, bonus: [80, 70, 60, 60] }, Board { name: "Bomb Dropper", kind: 0, bonus: [70, 60, 70, 70] }, Board { name: "Righteous Path", kind: 0, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 0, bonus: [90, 90, 80, 90] }],
    // JP
    [Board { name: "ShowStopper", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "JetSetter", kind: 2, bonus: [10, 10, 10, 0] }, Board { name: "Magnate Split", kind: 1, bonus: [20, 10, 20, 20] }, Board { name: "Eurotech", kind: 1, bonus: [20, 20, 10, 20] }, Board { name: "Suave", kind: 0, bonus: [30, 30, 30, 30] }, Board { name: "Resonance", kind: 1, bonus: [30, 20, 30, 40] }, Board { name: "Chaos Crippler", kind: 1, bonus: [50, 50, 50, 40] }, Board { name: "JP Ego-nomics", kind: 1, bonus: [50, 40, 50, 50] }, Board { name: "Hucker Special", kind: 1, bonus: [80, 60, 60, 60] }, Board { name: "Big Poppa", kind: 1, bonus: [70, 50, 70, 70] }, Board { name: "Crazy Ates", kind: 1, bonus: [80, 60, 80, 80] }, Board { name: "UBERBOARD", kind: 1, bonus: [90, 70, 90, 90] }],
    // Elise
    [Board { name: "Kamoniwana", kind: 0, bonus: [10, 10, 0, 10] }, Board { name: "Angel", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "SmoothMaker", kind: 0, bonus: [20, 30, 10, 10] }, Board { name: "Butterfly High", kind: 0, bonus: [20, 20, 10, 20] }, Board { name: "True Colours", kind: 0, bonus: [40, 30, 40, 20] }, Board { name: "Buttercup", kind: 0, bonus: [30, 40, 30, 30] }, Board { name: "Cutting Edge", kind: 2, bonus: [50, 70, 60, 30] }, Board { name: "Snake Eyes", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "First Date", kind: 0, bonus: [80, 70, 60, 60] }, Board { name: "Last Date", kind: 0, bonus: [70, 60, 70, 70] }, Board { name: "Black Widow", kind: 0, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 0, bonus: [90, 90, 80, 90] }],
    // Psymon
    [Board { name: "Bio Hazard", kind: 0, bonus: [10, 10, 0, 10] }, Board { name: "Evil Eye", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Dolmans Land", kind: 0, bonus: [20, 30, 10, 10] }, Board { name: "Chained Heat", kind: 0, bonus: [20, 20, 10, 20] }, Board { name: "Mayhem", kind: 0, bonus: [40, 30, 40, 20] }, Board { name: "Flesh Eater", kind: 0, bonus: [30, 40, 30, 30] }, Board { name: "Torc Rules", kind: 2, bonus: [50, 70, 60, 30] }, Board { name: "Live Wire", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "Shock Therapy", kind: 0, bonus: [80, 70, 60, 60] }, Board { name: "Banshee", kind: 0, bonus: [70, 60, 70, 70] }, Board { name: "Dogs of War", kind: 0, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 0, bonus: [90, 90, 80, 90] }],
    // Seeiah
    [Board { name: "Funk-o-matic", kind: 1, bonus: [10, 0, 10, 10] }, Board { name: "Psychadelic", kind: 2, bonus: [10, 10, 10, 0] }, Board { name: "Potpourri", kind: 1, bonus: [20, 10, 20, 20] }, Board { name: "Foxy Lady", kind: 1, bonus: [20, 20, 10, 20] }, Board { name: "Lucky Stars", kind: 0, bonus: [30, 30, 30, 30] }, Board { name: "Goddess", kind: 1, bonus: [30, 20, 30, 40] }, Board { name: "Venusian", kind: 1, bonus: [50, 50, 50, 40] }, Board { name: "Geodesic", kind: 1, bonus: [50, 40, 50, 50] }, Board { name: "Time Machine", kind: 1, bonus: [80, 60, 60, 60] }, Board { name: "Retro Beats", kind: 1, bonus: [70, 50, 70, 70] }, Board { name: "Funky Drummer", kind: 1, bonus: [80, 60, 80, 80] }, Board { name: "UBERBOARD", kind: 1, bonus: [90, 70, 90, 90] }],
    // Brodi
    [Board { name: "Now and Zen", kind: 2, bonus: [10, 10, 0, 10] }, Board { name: "Buddha Booster", kind: 1, bonus: [10, 0, 0, 20] }, Board { name: "Siddhartha", kind: 2, bonus: [20, 20, 20, 10] }, Board { name: "Balance", kind: 2, bonus: [20, 20, 10, 20] }, Board { name: "Flower Power", kind: 2, bonus: [40, 50, 20, 20] }, Board { name: "The Absolute", kind: 2, bonus: [30, 40, 30, 30] }, Board { name: "Yin-Yang", kind: 0, bonus: [60, 60, 50, 40] }, Board { name: "Tsunami", kind: 2, bonus: [50, 60, 50, 50] }, Board { name: "Master Blaster", kind: 2, bonus: [60, 70, 80, 60] }, Board { name: "Nirvana", kind: 2, bonus: [70, 60, 70, 70] }, Board { name: "Transcendence", kind: 2, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 2, bonus: [90, 90, 90, 80] }],
    // Marisol
    [Board { name: "Fiesta", kind: 2, bonus: [10, 10, 0, 10] }, Board { name: "Loco Motion", kind: 1, bonus: [10, 0, 0, 20] }, Board { name: "Sand Fish", kind: 2, bonus: [20, 20, 20, 10] }, Board { name: "Party Girl", kind: 2, bonus: [20, 20, 10, 20] }, Board { name: "Dance Explosion", kind: 2, bonus: [40, 50, 20, 20] }, Board { name: "Money Maker", kind: 2, bonus: [30, 40, 30, 30] }, Board { name: "Wind Rider", kind: 2, bonus: [60, 60, 50, 40] }, Board { name: "Booty Call", kind: 0, bonus: [50, 60, 50, 50] }, Board { name: "Heart Breaker", kind: 2, bonus: [60, 70, 80, 60] }, Board { name: "Latin Heat", kind: 2, bonus: [70, 60, 70, 70] }, Board { name: "Rave On", kind: 2, bonus: [80, 80, 70, 80] }, Board { name: "UBERBOARD", kind: 2, bonus: [90, 90, 90, 80] }],
];

/// Each rider's uber tricks by the grab they start from (the trick book's chapter 6,
/// DATA/TUTORIAL/TRICKDEF.DAT): the last is the rider's signature uber.
pub const UBER_NAMES: [(&str, &[(&str, &str)]); 12] = [
    ("Eddie", &[("Indy", "Gut Buster"), ("Stalefish", "SuperMan"), ("Mute", "Proper Propeller"), ("Method", "Hand in Hand"), ("Nosegrab", "Worm")]),
    ("Kaori", &[("Mute", "Paddle Wheel"), ("Method", "Hand in Hand"), ("Indy", "Can Can"), ("Stalefish", "Body Board"), ("Tailgrab", "Pirouette Grind")]),
    ("Luther", &[("Mute", "Torpedo"), ("Method", "FrogHop"), ("Indy", "The Rake"), ("Stalefish", "Nothing"), ("Tailgrab", "Bronco Buster")]),
    ("Mac", &[("Mute", "Paddle Wheel"), ("Method", "Hand in Hand"), ("Indy", "Sad Sack"), ("Stalefish", "Scooter"), ("Tailgrab", "Walking The Dog")]),
    ("Moby", &[("Indy", "Nac Nac"), ("Stalefish", "Nothing"), ("Mute", "Bar Hop"), ("Method", "FrogHop"), ("Nosegrab", "SuperMan Barspin")]),
    ("Zoe", &[("Mute", "Bar Hop"), ("Method", "Indian"), ("Indy", "Nac Nac"), ("Stalefish", "Near Miss"), ("Nosegrab", "Pommel Me")]),
    ("JP", &[("Indy", "Gut Buster"), ("Stalefish", "SuperMan"), ("Mute", "Paddle Wheel"), ("Method", "Hand in Hand"), ("Nosegrab", "HeadSpin 2 Poseur")]),
    ("Elise", &[("Indy", "Nac Nac"), ("Stalefish", "Near Miss"), ("Mute", "Bar Hop"), ("Method", "Judo"), ("Tailgrab", "LaLaLa Lock Step")]),
    ("Psymon", &[("Mute", "Torpedo"), ("Method", "FrogHop"), ("Indy", "Nac Nac"), ("Stalefish", "Nothing"), ("Tailgrab", "Guillotine")]),
    ("Seeiah", &[("Indy", "Can Can"), ("Stalefish", "Body Board"), ("Mute", "Proper Propeller"), ("Method", "Hand in Hand"), ("Nosegrab", "Soul Grind")]),
    ("Brodi", &[("Mute", "Iron Cross"), ("Method", "Airwalk"), ("Indy", "Cordova"), ("Stalefish", "StuntMan"), ("Nosegrab", "Hang 10 Backflip")]),
    ("Marisol", &[("Mute", "Mulisha"), ("Method", "Airwalk"), ("Indy", "Cordova"), ("Stalefish", "Dervish"), ("Tailgrab", "Aerial Spock 540")]),
];
/// The name of a rider's uber started from a grab.
pub fn uber_name(rider: &str, grab: &str) -> Option<&'static str> {
    UBER_NAMES.iter().find(|u| u.0.eq_ignore_ascii_case(rider)).and_then(|u| u.1.iter().find(|g| g.0 == grab)).map(|g| g.1)
}
/// The grab a rider's signature uber starts from.
pub fn signature_grab(rider: &str) -> Option<&'static str> {
    UBER_NAMES.iter().find(|u| u.0.eq_ignore_ascii_case(rider)).and_then(|u| u.1.last()).map(|g| g.0)
}
