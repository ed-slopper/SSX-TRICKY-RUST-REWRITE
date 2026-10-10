# tricky-rs

A from-scratch reimplementation of SSX Tricky in Rust + Bevy. It ships no game content: it reads a
level project folder extracted from your own disc (see `../notes/level-editing.md`).

## Stage 4 (this version): characters, tricks, boost and opponents

```
tricky-rs.exe                      loads ..\levels\gari and the characters in ..\chars
tricky-rs.exe D:\path\to\level     any folder containing Patches.json
```

You ride one of the twelve characters from the disc (Mac by default) against three computer riders.
A 3-2-1 countdown holds everyone in the gate.

| | Keyboard | Gamepad |
|---|---|---|
| Steer | A / D or arrows | left stick / d-pad |
| Tuck, and push off when slow | W / Up | stick or d-pad up |
| Brake | S / Down | stick or d-pad down |
| Jump: hold to crouch, release | Space | A / Cross |
| Boost (uses the meter) | Shift | X / Square |
| In the air: spin | A / D | left stick |
| In the air: back flip / front flip | Q / E | right stick down / up |
| In the air: nose grab, tail grab, method | J, K, L | left shoulder, right shoulder, B / Circle |
| In the air, boost meter full: uber trick | U | Y / Triangle |
| Grind: land on a rail from the air; jump hops off | | |
| Next character | C | |
| Previous / next track | [ and ] or PageUp / PageDown | |
| Restart (new countdown) | Enter | Start |
| Back to last safe spot (costs a little boost, as in the original) | Backspace | Select |
| Next camera (board, near, far, over) | C | Triangle / Y |
| Skip the pre-race intro | Space | Cross / A |
| Free camera on/off | Tab | |
| Rails overlay, hide help | R, F1 | |
| Edge smoothing (4x MSAA) on/off, if it runs slowly | F2 | |

Free camera: click to grab the mouse, WASD, Space / Ctrl up and down, Shift fast, + / - speed, Esc release.

**Tricks:** spins count per half turn, flips per full turn, grabs by how long you hold them; combining them in
one jump pays more. Rotation works as in the original: hold a direction (A / D for spin, Q / E for flip) *while crouched
on Space* to wind up, and the jump then spins or flips about twice as fast (a wound-up flip takes about a second).
Without the wind-up you still rotate, but slowly, and the rotation dies away through the jump. A flip you let go
of carries on to the next full turn by itself. Land more than about 80 degrees off
upright, or still holding a grab, and you wipe out and lose the points. Grinds pay by the second. Points fill the boost meter.

**Uber tricks:** with the meter full, U in the air starts one of your rider's own signature moves (each rider
has four or five, taken from their `bx<Name>Uber.afl`; each press picks the next). They last two to three
seconds, pay 3000, and landing before the move finishes is a wipe-out. Each one landed lights a letter of TRICKY;
all six make boost unlimited for the rest of the run.

**Things on the track:** glass panes, fences and signs that the game marks as breakable vanish when you ride
through them and their broken pieces (the game's own shard models) are thrown forward. The floating pickups work:
x2 / x3 / x5 gems multiply your next landed trick, the others give a burst of speed or fill the boost meter.
Markers and barrels get knocked about. Enter puts everything back.

**Wipe-outs** play the forward bail and then the get-up, about two seconds in all, with the controls locked.
**The start** uses the gate clip: riders hold the gate through the countdown and push out on GO.

**Characters** are the game's own meshes, skeleton and textures, skinned on the CPU, and they move with the
game's own animations, decoded from `bxanim.afl` (248 clips, see `../notes/animation-format.md`). The clips in
use: base and crouch riding cycles, heel-side and toe-side turns (standing and tucked), take-off, landing, nose
grab, tail grab, method, the flip and spin tucks, the rail-slide cycle, the forward bail and get-up, the gate
start, and each rider's uber tricks. Which clip plays
when is my state logic, not the game's, so transitions will not match the original exactly. Without
`chars/anims/bx.json` it falls back to a pose built in code.

**Opponents** follow the course's racing line with different amounts of tucking. Riders shove each other apart
on contact. If one wedges itself against something for seven seconds it is put back on the line.

**Knockable objects:** path markers, crash bags and small billboards (the objects the level data marks as
movable) fly off when a rider hits them, bounce and come to rest; Enter puts them back. They are simulated as
balls, so they tumble rather than slide and can pass through walls.

**Race timer:** the clock starts when you leave the gate and stops at the end of the course's main racing
line; best time is kept until you quit. On the open freestyle tracks (pipedream, untracked, megaplex, trick) the
"course" is just the first racing line, so the finish comes early.

What is simulated: rail grinding, gravity along the slope, edge grip (carving), drag, braking, jumps, landing, bouncing off
walls, rocks, trees and buildings, riding on flat object tops, respawn if you fall out of the world.
The terrain you collide with is exactly the terrain that is drawn. Objects use the game's own collision meshes.

The physics constants at the top of `src/rider.rs` are tuned by guesswork, not taken from the game yet.

### Self-test

`TRICKY_SIM=600 tricky-rs <level>` runs no window: an autopilot follows the game's race line from the gate and
prints progress, camera smoothness and time spent on rails. It reaches the finish on all eleven tracks without
ever falling through the world (it is a poor rider and gets stuck on obstacles 0 to 14 times per track).
`TRICKY_RAILTEST=1` with it drops a rider onto the five longest rails and reports each grind;
`TRICKY_TRICKTEST=1` drops a rider from 22 m doing five scripted things and prints how each landing scored.

## Building from source

Needs Rust (https://rustup.rs). First build takes several minutes.

```
cargo run --release -- ..\levels\gari
```

## Source layout

- `src/level.rs` - project-folder loader (JSON + OBJ), Bezier evaluation. Stays in game coordinates (X/Y ground, Z up).
- `src/collide.rs` - triangle grid: ground-under-point and push-sphere-out-of-walls queries.
- `src/rails.rs` - grindable rails built from the level's splines.
- `src/props.rs` - knockable objects and rider-to-rider contact.
- `src/character.rs` - character model loading, the code-driven pose, CPU skinning.
- `src/rider.rs` - rider physics, race timing and chase camera as plain functions, plus the headless self-test.
- `src/main.rs`  - Bevy app: textures with mipmaps, terrain/object/sky meshes, input, cameras, HUD.

## What the viewer taught us about the data

- World units are about 1 cm; the app scales by 0.01.
- Patch `Points` are 16 control points, row-major; `UVPoints` are the four corner UVs; `LightMapPoint`
  is the patch's x, y, w, h rectangle (0..1) in lightmap page `LightmapID`, 8x8 texels per patch.
- Terrain lightmaps are subtractive, not multiplicative: the pages store orange where there is shade and
  the game subtracts it, which is what makes Tricky's shadows blue. The viewer multiplies by (1 - lightmap),
  identical on white snow.
- Objects are lit with ambient + up to three directional lights stored per instance, 128 = 1.0.

## Not done yet

The game's own animation state machine (it has 248 clips; about 25 are wired up), checkpoints/laps and event rules, audio, objects with
collision mode 2 (stands, billboards, pickups: about 350 on Garibaldi, not solid yet), animated/scripted objects (they are drawn
in their rest pose), particles, lights/halos, exact PS2 blending modes for transparent materials.

## Physics

The riding numbers are the original game's, read out of the decompiled `SLUS_203.26` (rider with middling
stats, ordinary snow): top speed 100 km/h on the snow and 120 km/h boosting or in the air; gravity 13 m/s^2 on
the snow, 8.5 rising and 19 falling in the air; the game's drag curve (standing up adds drag, crouching on W or
Space removes it); the rider pushes himself up to 52 km/h; carving turns at up to 17 m/s^2 sideways and costs no
speed; jumps are 6.3 to 7.2 m/s with a 0.66 s crouch; a full boost meter lasts 22 s. Still mine, not the game's:
per-character stats, the other snow types (ice, powder), rail speeds and trick scores.

## The game around the riding

The game opens on a menu (Up / Down event, Left / Right rider, Enter start, Esc back to it at any time):

- **World Circuit - Race**: six riders in the gate, three heats (quarter-final, semi-final, final). The first
  three go through; the final gives gold, silver or bronze. The opponents get better each heat, do spins and
  grabs off the jumps and use the boost they earn.
- **World Circuit - Show-off**: alone on the course for points; gold / silver / bronze at 60,000 / 35,000 /
  15,000 (my numbers, not the game's).
- **Free Ride**: no opponents, just the clock.

**Rails:** A / D turns the board on the rail. Let go and it settles square (50-50) or sideways (boardslide,
which pays more); every half turn on the rail adds points, and landing on a rail sideways keeps the board sideways.

**More from the original's rules:** land a half spin and you ride away switch (tail first) instead of being
turned round; landing crooked or tilted costs up to a quarter of your speed; letting go of a flip levels you to
the nearest upright (so hold it past half way); hitting a wall at over 70 km/h is a wipe-out; F shoves a rival
alongside you over, for boost. While crouched for a jump the display shows what you are winding up.

## Riders, tricks and scoring from the original

Read out of the game's executable:

- **Rider stats.** Each of the twelve riders has the game's own edging, speed, stability and tricks numbers,
  jump and wind-up strength, weight and board type; they feed the same formulas the game uses (drag, braking,
  spin and flip speed, jump height, how crooked a landing can be, how fast the meter fills). In the menu, L
  switches between **rookie** (as a new game starts) and **master** (fully trained, best board).
- **Tricks.** The four grab keys J, L, U, O stand for L1, R1, L2, R2. Alone and together they give each rider's
  own fifteen grabs. K (or Shift) while holding a grab tweaks it for double points, and when uber tricks are
  available it starts that grab's uber trick instead (only the first four or five grabs have one).
- **Uber tricks** become available for 20 seconds each time the meter fills. Six landed spell TRICKY.
- **Points.** 475 per half spin, 1,700 per flip, 288 for the first grab of a jump (more for each further
  different one), 305 a second for holding it times the grab's rate (1 to 2.5), ubers at 18 to 20 times that
  rate, rails about 1,000 a second plus 1,220 for getting on and 1,360 for tricking off. A trick repeated among
  your last five pays a half, then a third. 10,000 points fill the meter; a wipe-out takes a tenth of it.
- **Snow types.** The course's own surface types now use the game's table (grip, drag, gravity), so ice,
  powder and the groomed runs ride differently.

The show-off medal scores (150,000 / 80,000 / 40,000) are still mine.

## Sound and snow

**Sound** comes from the game's own files, decoded into an `audio` folder next to `levels` (it is not part of
this repository; it is made from your disc). With the folder missing the game is simply silent. M turns the
music off and on, N the effects.

- `ride.wav`, `air.wav`, `land.wav`, `jump.wav`, `grind.wav`, `rail_on.wav` from `zboard.bnk` (the board bank:
  streams 4, 1, 2, 3, 54, 53), `boost.wav`, `crash.wav`, `glass.wav`, `menu_move.wav`, `menu_ok.wav`,
  `pickup.wav` from `zBxsfx.bnk` (streams 5, 8, 25, 10, 11, 23), `crowd.wav` from `Crowd.bnk`, `tricky.wav`
  from `tricky.bnk`, all inside `DATA/AUDIO/AUDIO.BIG`. The banks carry no names, so which stream is which
  sound was picked by length, looping and tone, not read from the game: some may be the wrong sound.
- `music.ogg` is "Smartbomb" (one of Garibaldi's three songs): the 317 one-bar pieces of `smartbomb.mus` in
  `DATA/AUDIO/MUSIC.BIG`, in file order. The game stitches the bars together as you ride; this is one fixed
  nine-minute pass through them.
- Decoded with vgmstream (`vgmstream-cli -i -s <stream> -o out.wav file.bnk`), music joined with ffmpeg.

The ride sound follows speed and how hard you carve or brake; the air, rail and boost loops fade in and out.

**Snow spray** comes off the board's digging edge when carving, forward when braking, and in a ring on landing.

**Rails, as the game animates them:** A / D turns the board a quarter turn at a time (regular, sideways,
tail-first), and the body uses the game's own frontside and backside rail stances. Grinds are named as in the
game: 50/50 Rail, BS Rail, FS Rail, Switch 50/50 Rail.

**More of the game's animation clips** are in use: leaning into the wind-up while crouched, tucking into a spin
or flip before its cycle, the tweak of each grab, a shove (F), and standing up over the finish line.

**Rivals shove back.** In a race, a rival who has been alongside you for a moment gives you a shove; the heavier
and steadier rider stays up (the game's own weight and stability numbers decide), so shove first (F) or keep
clear. They do it more often in the later heats.

Wipe-outs and landings now pick from the game's set of clips: falling forwards, backwards or to either side with
the matching get-up, and a recovery wobble after a crooked or tilted landing. With `music2.ogg` and `music3.ogg`
in the `audio` folder (Garibaldi's other two songs, `systemover` and `adamsrev` on the disc, made the same way
as `music.ogg`) each new run moves on to the next song.

**Tracks and their music.** `[` and `]` change track, in the menu too. Each track plays the songs `DATA/CONFIG/MUSICMAP.INF`
lists for it, kept once each as `audio/music/<name on the disc>.ogg` (all nineteen are decoded: `smartbomb`,
`systemover`, `adamsrev`, `ginandsin`, ...). `go.wav` (stream 39 of
`zBxsfx.bnk`) plays on GO; `menu_ok.wav` is the countdown beep.

**Out of bounds.** The course's invisible reset zones (`ResetZone`, `..._Reset_...`, `CrowdTrap` objects) are no
longer walls: touching one puts you back at your last good spot, as the game does, with `reset.wav`. Trigger and
emitter objects (fireworks triggers and so on) are not solid any more; one of them was blocking Snowdream's gate.

**Sound files as now used** (in `audio/`): `ride`, `air`, `grind`, `slide` (loops; `slide` plays while braking),
`land`, `jump`, `rail_on`, `boost` (once, when a boost starts), `levelup` (the boost meter passing a third, two
thirds, full), `reset`, `countdown` (3, 2, 1; GO is the same beep an octave up until a `go.wav` exists),
`pickup`, `tricky`, `menu_move`, `crowd`, and optional `crash.wav`, `glass.wav`, `go.wav`. `audio/unlabeled/`
holds every distinct sound of the board, effects, Garibaldi, crowd and TRICKY banks, numbered, for identifying.

**The courses' own sounds** (`audio/world/`, every bank of `DATA/AUDIO/AUDIO.BIG` decoded by
`tools/bnk/levelaudio.py` + `tools/bnk/worldsounds.py`, plus each level's `audio/levelaudio.json`): the crowds, snow
cats, birds, rivers, cowbells and chants placed on the course's objects fade in and out as the camera passes, with
the game's six volume curves; running into fences, signs, flags and the like knocks with that object's own sound
(louder the faster you hit it, quieter further from the camera); and the fireworks bang when their trigger is
touched. With these files the stand-in `crowd.wav` stays quiet.

**The course's own music in the intro.** With `audio/course/<track>/seg_00..16.wav` (each course's
`DATA/AUDIO/<COURSE>.BIG`, decoded by `tools/bnk/levelaudio.py`) the flyover plays the course theme as the game
strings it together (A1, another A, two B, then C parts), the closing part when the intro ends or is skipped, and
the race song is held back until GO.

**The game's fonts.** With `chars/fonts/` (from `DATA/FONTS/TITLE.SFN` and `MENU.SFN`, made by
`tools/sfn/sfn2png.py`) the in-race place, time, points, speed, multiplier and trick names are drawn in the
original fonts, sizes, colours and places, with the 2-pixel drop shadow.

**Best results** (best time, best score, race and show-off medals per track) are kept in `tricky-save.json` next
to the program and shown in the menu.

**Opponents** now aim further ahead the faster they go, brake for bends they cannot make, and rejoin on the
racing line after an out-of-bounds reset.

**Pause.** Esc during a run pauses it (Resume / Restart / Quit to menu) instead of dropping straight to the menu.
Letting go of a grab now drops it at once.

## Track editor (first version)

In the free camera (Tab), F4 turns the editor on. A small white cross marks where the centre of the view meets
the world.

| | |
|---|---|
| Pick the object nearest the cross | F |
| Move it to the cross | T |
| Slide it / raise and lower it (Shift = faster) | arrow keys / Y and H |
| Turn it (a twelfth of a turn) | Z, X |
| Smaller / bigger | , and . |
| Copy it to the cross | C |
| Remove it | Delete |
| Save and reload the track | Enter |

Saving rewrites the level project's `Instances.json` (the first save keeps the original beside it as
`Instances.json.bak`; put that back to undo everything) and reloads the track, so Tab takes you straight back to
riding the changed course. Objects only so far: the snow itself (`Patches.json`), rails and the racing line are
not editable yet. Picking goes by each object's origin, so aim at the base of a tree or sign, not its top.
`tools/ssxlevel build` turns the same project folder back into a `.BIG` for the real game.

## Boards

Every rider's twelve boards are in, with their real names, graphics, shapes and stat bonuses. In the menu, Up /
Down picks a row (Event, Rider, Board, Track, Training) and Left / Right changes it. The board row shows the
board's name and kind; the stats line under it changes with the board (the game mixes 80% rider with 20% board).

- **Shapes:** BX, freestyle and alpine boards are three different models (2.0 m, 1.7 m and 2.6 m long), from
  `board.mpf` in `DATA/CHAR/BRDPS2.BIG`: `chars/board_bx.json`, `board_fr.json`, `board_al.json` (and the goofy
  versions, exported but not used yet).
- **Graphics:** `chars/<rider>/bord1.png` to `bord12.png`, from `<rider>N_bord.ssh` in `TEXPS2.BIG`.
- **Names, kinds and bonuses** come from the board table in the executable (0x332148) and are in
  `src/trickdata.rs`. Alpine boards also use the game's lower-drag, stronger-push formulas.
- All twelve are available from the start (the game unlocks them as you earn experience). Rivals ride their first
  board as rookies and their UBERBOARD as masters.

## 1:1 with the original (in progress)

The riding is being rebuilt to match the decompiled game, rule by rule; `docs/original-rules.md` has every rule
and constant found so far, with the function addresses. Done in this pass:

- **Riding:** the original's carving model. Steering tips the board's support force sideways (the "lean"), the
  board then yaws round after the direction of travel, and a hard carve costs a lot of speed. Steering, crouch and
  brake are analog and move at the game's rates; slow speeds steer weakly. Drag acts along the board and grows
  under load; the edge only weakly resists sideways slip; a spring holds the board on the snow and it leaves the
  ground when the snow drops away by more than a few centimetres. The rider pushes himself along only roughly
  down the course, and boost only works going straight. Riding backwards slowly swings the board round (switch).
- **Jumping and the air:** the crouch charges exponentially; the jump fires a moment after you let go; tiny
  wind-ups count for nothing and a nearly pure spin or flip wind-up is made pure. In the air the stick drives
  the rotation inside a band that narrows through the jump; let go and the rotation finishes itself (to the next
  half turn for spins, full turn for flips, back only when within 40 degrees). Without a wind-up nothing turns
  until you let go and press again. Crash limits are no longer symmetric, a nose-down or very fast landing is a
  hard landing, and the landing no longer straightens the board for you.
- **Rails:** A / D balances: for 0.6 s the board centres itself, then it drifts off one side unless you lean
  against it, and too far off you fall off (no crash). Shift + A / D turns the board a quarter turn. Boost on a
  rail is a kick per press (held only while crouched). No friction or speed limit on rails.
- **Scoring:** chain bonuses (4,000 to 16,000 for 2 to 5+ tricks in a row), air time bonus, rail tricks never
  count as repeats, rail half turns pay as you make them, leaving a rail fakie gives the switch bonus, any gain
  at a full meter re-opens the uber window, multiplier pickups only count in the air or on a rail.
- **Opponents and races:** opponents follow the course's own AI path events (target speeds, jump zones), steer
  and cruise by the game's rules, re-pick their skill every second by who is ahead, and use the game's catch-up
  (their physics runs 0.7x to 1.5x depending on the gap to you, inside a lead window per starting slot). The
  countdown is 2.5 s. Show-off runs against the track's clock (Garibaldi 120 s, Alaska 135 s, others 90 s) with
  the game's medal scores per track.
- **Start gate:** hold Up / W in the gate during the countdown to lean forward; rock back with Down / S about
  half a second before GO for a slingshot start (up to about twice the normal 20 km/h push-out). Each
  character's gate stat sets how fast they rock. Computer riders rock and time it by skill.
- **Grudges:** each rider has a friend, a foe and a liking for each of the others (the game's tables). Knock a
  computer rider about enough and it holds a grudge: it chases you, shoves you when alongside and taunts you.
  Only riders with a grudge shove; getting even cools them down.
- **Snow:** the board spray now follows the game: carving throws a sheet of snow off the edge (much more in
  powder, little on hard snow and ice), a thin trail in the air and on rails, a fan when braking, a splash on hard
  landings.
- **The finish:** over the line every rider brakes, coasts to a stop, stands up and cheers or sulks by result;
  the results come up 4 s after the line, as in the game.
- **After the race:** once you have stopped, the game's own post-race scenes play at the finish area: a rival
  who has a grudge against you has words first, then your character's win or lose scene with the others around
  (the clips come from `chars/anims/finish.json`, extracted from ANM.BIG). Space skips. Then the results.
- **Board tracks:** riders leave faint tracks in the snow (deeper on ice, fainter in powder) that fade out
  behind them.
- **Career:** races are two heats and a final; career points (15/10/5 per medal, best medal per track) train
  your rider from the starting attributes to the caps and give a rank (Master at 240); computer riders improve
  with you. The HUD clock now shows hundredths, the place turns gold when leading, trick names show in gold.
- **Unlocks:** as in a new game: Mac, Moby, Elise and Eddie to start, one more rider per gold medal; Garibaldi,
  Snowdream and Elysium open, each medal opens the next track (Alaska opens Untracked and Pipedream); boards by
  career points. Setting Training to "master" unlocks everything (like the original's cheat).
- **Rivals:** the post-race rival scene now follows the game's rivalry score (grudges and getting even);
  taunts follow the original's rule. Multiplier pickups only work in show-off; no points after the finish line.
- **Trick book:** each rider's 30 trick book entries in six chapters, from the disc's TRICKDEF.DAT (made into
  `chars/trickbook.json` by `tools/trickbook/trickbook.py SLUS_203.26 TRICKDEF.DAT chars`). Land the tricks of
  the current chapter to tick them off ("TRICK BOOK: ..." under the trick name); the menu shows the chapter and
  the next trick; finishing the book unlocks the rider's UBERBOARD. Progress is kept in `tricky-book.json`.
- **Ubers:** each rider's own uber names (e.g. Eddie's Indy uber is the Gut Buster) and their signature uber
  where its animation exists; the uber's name replaces the grab's.
- **Level scripts:** the course's own scripts (SSFLogic.json) now run their main effects: each event hides what
  the original hides (race-only objects in show-off, the start gate in free ride) and switches the show-off
  rails off in races; anything whose touch script resets the rider is a reset zone (crowds, rivers, backdrop
  trees); boost pads push riders along (Alaska, Merqury); teleports work; the Merqury subway train and
  Snowdream's ski lift chairs ride their splines (with the lift cables drawn).
- **Moving scenery:** the level's keyframed objects play their animations as in the game (fans, penguins,
  sailboats, Aloha's side-to-side barriers, helicopters), and broken glass, junk and letters fly apart with each
  object's own debris settings from the level scripts.
- **World animations:** signs, check-point tops, LCD logos and shop signs flip through their texture
  flipbooks, the start lights count down red, yellow, green with the countdown, boost pads, rivers, waterfalls,
  conveyors and jumbotron tops scroll their textures, flags and banners wave, and the scripts' one-shot clips
  play when set off (Mesablanca's falling trees, Merqury's sewer gates) — all from the level scripts.
  Crowds in the stands animate cell by cell from `chars/crowd/` (DATA/TEXTURES/CROWD.SSH), Megaplex's glass
  panes crack when ridden on and break on a hard hit, and knocking down Merqury's garbage cans lights the strike
  lights one by one and then the STRIKE sign.
- **Wipe-outs as in the game:** the body tumbles as one rigid body (the game's masses, gravity, drag and
  bounce rules), plays the impact clip for the side it lands on, rolls onto its back or front and slides to a
  stop, then gets up facing down the course with one of the six get-up clips — or, if it lands upright, rides
  straight on; a knock in the air with little spin recovers into an ordinary jump. Stuck for 3 s (7 s in all),
  you are put back on the course. `TRICKY_CRASHAT=s` forces a wipe-out at race time s.
- **Objects as the game treats them:** an object with a surface type is ridden like ground (Megaplex's glass
  floors and frames, ramps, platforms); others are bounced off; scenery without "player bounce" is not solid.
  Keyframed objects collide where they are drawn (Megaplex's iris doors open from their buttons, flippers,
  ramps, bumpers, Merqury's trains), Megaplex's glass floors crack and break under you, and a restart puts the
  world back as it was.
- **Interactive race music:** with `audio/pf/` (all of DATA/AUDIO/MUSIC.BIG, made by `tools/music/musicbig.py`
  and `pack.py`) each song plays as the game's Pathfinder graph: bars chosen by your place (or, alone, how full
  the boost meter is), shortcut zones switching sections, an outro at the finish, and the song's beat-synced
  in-air loops over big air and its own Tricky phrase.
- **Pickups as in the game:** the multiplier gems spin and stay where they are when you go through them (the
  game never removes them), as do the speed and trick boost pads.
- **The game's own board sounds:** with `audio/bnk/` (the original zboard and zbxsfx banks exported by
  `tools/bnk/bnk2wav.py BANK.bnk audio/bnk board|sfx`: each program's sample, its loop, and its root note, bend
  range and volume, which set the pitch as the game's sound driver does) and `audio/snow.inf` (DATA/CONFIG/SNOW.INF) next to the levels, the board
  plays the original glide, carve and scrape loops for each surface (packed snow, powder, ice, rock, metal, wood,
  rails...), with volume and pitch worked out by the game's own snow sound programs from the board's load, slip,
  dig and lean; takeoff and landing sounds per surface by impact; crash, grab, boost (by meter level),
  pickup, reset and uber-ready sounds; the wind comes up only in big air as the music ducks.
- **The game's HUD art:** with `chars/hud/` (HUDGAME.SSH's sheets as PNGs and the sprite table) the boost
  meter, the TRICKY letters and the uber orb are the game's own sprites, placed as in the game.
- **Particles:** the levels' particle emitters run as in the game (snow blowers, torches, spray, the bursts
  when you hit a gem, a sign or glass), with the game's motion and colour rules; drawn with a plain soft sprite
  with the game's particle sprites when `chars/particles/<name>.png` are there (made from DATA/TEXTURES/PARTICLE.SSH
  with `tools/ssh/ssh2png.py PARTICLE.SSH chars/particles`).
- **Uber sets per board:** each board type has the rider's own uber animations (`chars/<rider>/uber_fr.json`,
  `uber_ex.json`, from fr/ex<Chr>Uber.afl), and every rider's signature uber is there on their own board type.
- **Rivals:** how each rider feels about you carries from heat to heat in a circuit, worn down between heats.
- **HUD:** the boost meter is the original's column of fourteen segments in red, orange and yellow bands; the
  speed is bottom left.
- **Wipe-outs:** hitting the snow from a tumble plays the original's impact clips.
- **Sound:** four countdown beeps (0.5 s apart), no beep at GO, the music ducks in big air while the wind comes
  up, and the boost sound follows the meter level.

## Pre-race intro

Before the countdown the original flies the camera through the course and then shows the riders in the start
gate, from the course's camera scripts. tricky-rs plays the same scripts if it finds them: copy `DATA/CAMERA`
from the game disc as `levels/<track>/Camera/track.cml` (GARIBALD.CML for gari, SNOWDREA for snowdream, ELYSIUM,
MESABLAN, MERQURY, ALOHA, TOKYO for megaplex, ALASKA, PIPEDREA, UNTRACKE, TRICK) plus `commonob.cml` and
`scripts.cml` (COMMONOB.CML, SCRIPTS.CML). Space skips it.

In a race the riders also act out the original's staging scene (stretching, talking, polishing boards) and then
loosen up in the gate. Those clips are not in the riding set: extract them from `DATA/CHAR/ANM.BIG` with
`tools/afl/anmbig.py ANM.BIG out stg_com_1.afl stg_com_2.afl stg_com_3.afl stg_com_4.afl GAT_6COM_1.afl
GAT_6COM_2.afl GAT_6COM_3.afl GAT_6COM_4.afl`, convert each with `tools/afl/aflexport.py`, and merge the clip
lists into `chars/anims/scenes.json`. Without it the riders just wait in the gate.

Freestyle and alpine boards have their own riding animations in the original (`franim.afl`, `exanim.afl`), and
wipe-outs share `cmanim.afl` (sliding on the back or front, tumbling). Extract those the same way into
`chars/anims/fr.json`, `ex.json` and `cm.json`; without them every board uses the BX set.
