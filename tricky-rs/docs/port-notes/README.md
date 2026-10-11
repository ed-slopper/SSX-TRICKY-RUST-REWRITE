# Port notes: how the original works (read from SLUS_203.26)

Notes from the decompiled executable, gathered to make tricky-rs match the game 1:1. Units in the game are cm,
cm/s, and a fixed 60 Hz tick (`dt = timescale/60`, timescale at boarder+0x12c, normally 1). Functions are named
in the Ghidra project (ghidra-mcp). "Proven" = read from code; "inferred" = interpretation.

One file per system (board row F6, split from the former `original-rules.md`):

- [boarder.md](boarder.md): Riding on the snow (Ground riding; Animation timing)
- [air.md](air.md): Jumps, air, tricks, landing (Jump, air, tricks, landing)
- [rails.md](rails.md): Rails (Rails; Rails)
- [scoring.md](scoring.md): Meter, scoring, tricks (Meter, uber, scoring; Trick names and points; Scoring; Trick book; Uber sets per board)
- [wipeout.md](wipeout.md): Wipe-outs (Wipe-outs; Wipe-out body; Wipe-out motion)
- [ai.md](ai.md): AI and rivals (AI, race, camera; Rivals and grudges; Rivalry)
- [race.md](race.md): Race, course and finish (Race lines, course events; Tokyo Megaplex laps; Pre-race scenes, surfaces, meter; Start gate; The finish; Post-race scenes)
- [camera.md](camera.md): Cameras (Chase camera; Camera scripts)
- [world.md](world.md): World objects (World objects; World controllers; Object collision; Keyframed objects; World animations)
- [scripts.md](scripts.md): Level scripts (Level scripts)
- [effects.md](effects.md): Board and particle effects (Board effects; Particle emitters)
- [audio.md](audio.md): Sound and music (Sound and music; Rider sounds; Sound banks; Level sounds; Music)
- [hud.md](hud.md): HUD and fonts (HUD; HUD sprites; Fonts)
- [frontend.md](frontend.md): World Circuit and unlocks (World Circuit; Unlocks)

## Unresolved

Ragdoll sphere places (masses known); song names (music.inf / musicmap.inf); post-race scene row filters; spark and dust
sprite rendering; HUD text (american.loc).
