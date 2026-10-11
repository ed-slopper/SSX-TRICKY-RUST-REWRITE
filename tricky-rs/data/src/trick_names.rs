//! The trick-name tables `Score_FormatTrickName` 0x1551c0 prints from (row E5d): the pointer words from
//! 0x320970 to 0x320f58 of SLUS_203.26, as strings (`None`: a null pointer or not a string), so that an index
//! past one table reads the next one as the game does. Generated from the executable; do not edit by hand.

/// The first word's address.
pub const BASE: u32 = 0x320970;

pub const RAIL_TO: u32 = 0x320b08;
pub const RAIL_SPIN: u32 = 0x320b20;
pub const STANCE: u32 = 0x320970;
pub const SPIN_DIR: u32 = 0x320988;
pub const SPIN: u32 = 0x320998;
pub const FLIP_COUNT: u32 = 0x3209c8;
pub const FLIP_KIND: u32 = 0x3209d8;
pub const COMBO: u32 = 0x3209f0;
pub const SPIN_AFTER: u32 = 0x320aa8;
pub const LATE: u32 = 0x320b50;
pub const GRAB: u32 = 0x320b58;
pub const ENDING: u32 = 0x320ad8;
pub const CRASH: u32 = 0x320ae8;

pub const WORDS: [Option<&str>; 378] = [
    Some(""), Some("Switch "), Some("Rail To "), Some("Rail To Switch "), Some("Late "), None, Some(""), Some("FS "),
    Some("BS "), None, Some(""), Some("180 "), Some("360 "), Some("540 "), Some("720 "), Some("900 "), Some("1080 "),
    Some("1260 "), Some("1440 "), Some("1620 "), Some("1800 "), None, Some(""), Some(""), Some("Double "),
    Some("Triple "), Some(""), Some("Front Flip "), Some("Back Flip "), Some("Rodeo "), Some("Misty "), None,
    Some(""), Some("Crippler "), Some("Egg Flip "), Some("Mc Twist "), Some("Haakon Flip "), Some("Wetcat "),
    Some("Full On "), Some("Breakdancer "), Some("Scratch Artist "), Some("Famous "), Some("DJ "), Some("Mindless "),
    Some("Banzai "), Some("Shiny "), Some("Torpedo "), Some("Twister "), Some("Nose Picker "), Some("Horrifying "),
    Some("Gross "), Some("Backwash "), Some("Ambulance Trip "), Some("Ligament "), Some("Cartilage "),
    Some("Hospitalized "), Some("Life Insurance "), Some("Tripped Out "), Some("Plain Brown Wrapper "),
    Some("Brown Bag "), Some("Lunch Box "), Some("Grab Bag "), Some("Crippled Squirrel "), Some("Montezuma "),
    Some("Homesick "), Some("Double Jointed "), Some("Spider "), Some("Chunks "), Some("Heimlich "), Some("Gargle "),
    Some("Sick "), Some("Shell Cracker "), Some("Deathwish "), Some("Iron Lung "), Some("Multiple Fracture "),
    Some("Roadkill "), Some("???"), None, Some(""), Some("180 "), Some("360 "), Some("540 "), Some("720 "),
    Some("900 "), Some("1080 "), Some("1260 "), Some("1440 "), Some("1620 "), Some("1800 "), None, Some(""),
    Some("Air"), Some("To Fakie"), Some("To Rail"), Some(""), Some("Take a Seat"), Some("Trouble at the Ranch"),
    Some("Have a Nice Trip"), Some("Look Mom, No Control"), Some("That was Weak"), Some("Simply Brutal"),
    Some("Tumbleina"), Some(""), Some("50/50 Rail "), Some("Switch 50/50 Rail "), Some("BS Rail "), Some("FS Rail "),
    None, Some(""), Some("180 "), Some("360 "), Some("540 "), Some("720 "), Some("900 "), Some("1080 "),
    Some("1260 "), Some("1440 "), Some("1620 "), Some("1800 "), None, Some(""), Some("To Late "), Some(""),
    Some("Canadian Bacon "), Some("Chicken Salad "), Some("Crail "), Some("Cross "), Some("Experimental "),
    Some("Flying Squirrel "), Some("Iguana "), Some("Indy "), Some("Japan "), Some("Lein "), Some("Melancholy "),
    Some("Method "), Some("Mute "), Some("Nosegrab "), Some("Nuclear "), Some("Roast Beef "), Some("Rocket "),
    Some("Seatbelt "), Some("Skunk "), Some("Slob "), Some("Spaghetti "), Some("Stiffy "), Some("Stalefish "),
    Some("Stalemaskey "), Some("Swiss Cheese "), Some("Tailgrab "), Some("Canadian BACK Bacon "),
    Some("RUBBER Chicken Salad "), Some("HOLY Crail "), Some("Cross THE ROAD "), Some("UNETHICAL Experimental "),
    Some("SKINNED Flying Squirrel "), Some("Iguana KABOB "), Some("SINFUL Indy "), Some("MADE IN Japan "),
    Some("MEAN Lein "), Some("GLEEFUL Melancholy "), Some("Method MADNESS "), Some("MUTATION "), Some("NOSEBLEED "),
    Some("Nuclear REACTOR "), Some("Roast Beef JERKY "), Some("Rocket BOOSTER "), Some("BUCKLED Seatbelt "),
    Some("FUNKY Skunk "), Some("TOTAL Slob "), Some("MEATBALLED Spaghetti "), Some("IFFY Stiffy "),
    Some("FILET O'Stalefish "), Some("FRESH Stalemaskey "), Some("RIPE Swiss Cheese "), Some("Tail WAG "),
    Some("Can Can "), Some("Heel Clicker "), Some("CliffHanger "), Some("SuperMan "), Some("Sad Sack "),
    Some("Hand in Hand "), Some("Paddle Wheel "), Some("Scooter "), Some("Gut Buster "), Some("Magician "),
    Some("Proper Propeller "), Some("Body Board "), Some("Nac Nac "), Some("Indian "), Some("Bar Hop "),
    Some("Nothing "), Some("Blender "), Some("Judo "), Some("Footloose "), Some("Benihana "), Some("The Rake "),
    Some("FrogHop "), Some("Torpedo "), Some("Near Miss "), Some("Cordova "), Some("Hart Attack "), Some("Mulisha "),
    Some("LazyBoy "), Some("Madonna "), Some("Airwalk "), Some("Iron Cross "), Some("Dervish "), Some("Triple H "),
    Some("FloorBoard "), Some("Coffin Roll "), Some("StuntMan "), Some("Worm "), Some("Pirouette Grind "),
    Some("Bronco Buster "), Some("Walking The Dog "), Some("SuperMan Barspin "), Some("Pommel Me "),
    Some("HeadSpin 2 Poseur "), Some("LaLaLa Lock Step "), Some("Guillotine "), Some("Soul Grind "),
    Some("Hang 10 Backflip "), Some("Aerial Spock 540 "), Some("Combo Grab "), None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
];
