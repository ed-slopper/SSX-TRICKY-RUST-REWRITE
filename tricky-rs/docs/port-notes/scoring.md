# Meter, scoring, tricks: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Meter, uber, scoring (score object boarder+0x5820, `Score_*` 0x155410–0x157490)

- Points = round10(mult·units·6786.545); meter += units·0.6786545·(0.97878+0.35966·a1d)/(repeats+1) (negative too).
- Units: half spin 0.0699543; flip 0.2499898; grab start (n+1)·0.0424975; hold rate·0.000750064/tick;
  uber clip seconds·0.0450038·rate; rail per tick f(v)·0.0025006 (f = v/(3200−3v), 0 under 50, 1 from 800 cm/s);
  rail spin +0.0699543 per new max of |round180|; onto rail 0.18; off rail 0.19998.
- Leaving a rail scores the rail trick with no repeat check; > 1 s adds 1 to the chain; leaving switch/fakie gives
  the next air trick ×1.3.
- Landing (only if an air trick was done): repeat ring of 5 divides by (n+1); chain +1; on a snow landing with
  chain ≥ 2 add 4000/8000/12000/16000 (2/3/4/5+), then reset; air time ≥ 4 s adds (int)(t−3)·1000. Bonuses are
  unmultiplied, after the repeat division.
- Boost held drains 0.00075015/tick (22.2 s full). Human passive leak (15.4835 − 15.2104·a1b)·9.1667e-6/tick.
- Uber timer = 20 s whenever meter gain happens at a full meter; −1/60 per tick in every state, never below one
  tick in the air. TRICKY (cumulative ubers > 5) sets meter 1, timer 1. Bail −0.10, cap 0.666, timer 0.
- Pickups ×2/×3/×5 only while airborne or on a rail, reset at landing.

## Trick names and points (`Score_BuildTrickId` 0x157490, `Score_FormatTrickName` 0x1551c0)

- Parts in order, each with a trailing space, no "+": rail type, rail half-turns, prefix ("Switch ", "Rail To ",
  "Rail To Switch ", "Late "), side ("BS " spin > 0, "FS "), spin degrees (no flip), flip count ("Double ",
  "Triple "), flip type ("Front Flip ", "Back Flip ", "Rodeo " back + 540/flip, "Misty " front + 540/flip),
  special name (table 0x320cf0 by side, half turns, flips: Mindless, Deathwish, Banzai, ...), spin after the flips
  (when not exactly 540 per flip), grab 1, "To Late " + grab 2 (3+: "Combo Grab"), suffix ("Air" for a plain grab,
  "To Fakie", "To Rail"). More than 1800 of spin or 4 flips: "???".
- Points = units × 0.6786545 × 10000 × multiplier, rounded to 10. No off-axis bonus. Grab hold rates by grab
  (Indy/Method/Mute/Stalefish 1.0 … Cross/Experimental 2.5), tweaks 2×, ubers 18/20.
- Late tricks (`SpinState_StartFromStick` 0x102158): with the stick let go, no grab, and both |ω| < 0.7, a
  press restarts rotation at 5.2531(0.5449+0.6742T) per axis (no 2/3 on flips); the trick so far is banked
  (Score_Takeoff) and the rest is named "Late …" with no repeat check.
- Multiplier: 1, ×1.3 only for a trick off a rail left switch, pickups replace it (max). Repeats: last 5 IDs,
  points and meter ÷ (n+1). No landing-quality grades.

## Scoring (the rest)

- Modes: 0 knockdown practice, 1 free ride, 2 practice race, 3 practice show-off, 4 circuit race, 5 circuit
  show-off, 6 lesson, 7 timed race. Trick points are the same in every mode; nothing is banked after the finish.
- Points, boost, multiplier and show-off-time pickups only work in show-off; speed and spin boosts everywhere.
  Points pickups never reach the score total. Breaking things scores nothing; a knockdown gives +1 meter only.
- TRICKY = more than 5 cleanly landed ubers; lasts the event: meter and uber timer held at 1, no drain, no uber
  bail penalty, no points multiplier. Bail: chain 0, pending trick lost, meter −0.10. Stumble −0.02.
- Trick-trigger zones (controller 0x15): run a script if the rider's score rises by a threshold within a time.

## Trick book (DATA/TUTORIAL/TRICKDEF.DAT, `Score_TrickBookCheck` 0x1579e0)

- 12 sets (one per rider, in character order) × 30 records × 28 bytes; 6 chapters of 5. Only the first
  unfinished chapter counts. A trick matches on its id word (combo name, grabs, spin size) and flip bits
  (count and type); the spin's direction, a switch takeoff and how it lands ("Air", "To Fakie") don't matter.
- trickId: byte 0 named combo (Crippler … Roadkill, 44 "???"), byte 1 grab 1, byte 2 grab 2 (1–26 grabs, 27–52
  tweaked, 53–100 ubers, 101 Combo Grab), bits 24–27 spin ×180 without flips, 28–31 spin with flips.
  flags: 0–3 crash name, 4–7 rail spin, 8–11 flip type (front, back, rodeo, misty), 12–15 rail type, 16–19
  takeoff (switch, rail to, rail to switch, late), 20–23 ending (Air, To Fakie, To Rail), 24–26 flip count,
  27–29 FS/BS, 30–31 two or more grabs.
- Chapters: 1 grab Airs, 2 tweaks and a 360, 3 flips and 540–720s, 4 360 flips, Misty/Rodeo, 900–1080, 5 doubles
  and named combos, 6 the rider's five ubers (the last its signature: Eddie Worm, Kaori Pirouette Grind,
  Luther Bronco Buster, Mac Walking The Dog, Moby SuperMan Barspin, Zoe Pommel Me, JP HeadSpin 2 Poseur,
  Elise LaLaLa Lock Step, Psymon Guillotine, Seeiah Soul Grind, Brodi Hang 10 Backflip, Marisol Aerial Spock 540).
- Each rider's uber names by the grab they start from are in src/trickdata.rs (UBER_NAMES); an uber's name
  replaces its grab's in the trick name.

## Uber sets per board

- Each rider has a set of ubers for each board type (bx/fr/ex <Chr>Uber.afl), on Indy, Method, Mute and Stalefish
  with different styles (MT/OT/SK); the signature uber (UT_SIG<CHR>) is only in the set of the rider's own
  board type: freestyle for Eddie, JP, Kaori, Mac, Seeiah; alpine for Brodi, Marisol; BX for the rest.
