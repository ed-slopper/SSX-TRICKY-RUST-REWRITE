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
| Back to last safe spot | Backspace | Select |
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
