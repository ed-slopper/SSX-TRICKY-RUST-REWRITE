# AGENTS.md: how agents work in this repo

## 1. The goal

All of **SSX Tricky** (PlayStation 2, USA, `SLUS_203.26`) rewritten 1:1 in **Rust** (`tricky-rs/`, Bevy as the
window, GPU and audio host). Every function of the game is re-created from the original binary so it behaves
the same, until the whole game runs from our code and the player's own game files (their own disc).

The board is [`WORK.md`](WORK.md). The rules below let any number of AI agents, working for any number of
people, pick work from it without clashing.

## 2. The reference binary

Every address in this repo, in code, notes, the board and commit messages, belongs to:

| | |
|---|---|
| Game | SSX Tricky, PS2, NTSC-U, serial SLUS-20326 |
| File | `SLUS_203.26` from the disc root |
| ELF md5 | `162580fd65611e72a90deed640a70aef` |
| PCSX2 CRC | `8E7CFF62` |
| Load | one segment, file offset 0x1000 → 0x00100000 (file offset = address − 0xFF000) |
| Functions | 7,425 (Ghidra 12.1.4 with the Emotion Engine extension, language `r5900:LE:32:default`) |

Other versions exist (PAL and Japanese PS2, GameCube, Xbox). **Never mix their addresses with these.** If you
ever read another version, write its name next to every address you take from it and keep it out of the port
until the humans decide (board row F7). If the md5 of your `SLUS_203.26` differs, stop and tell the human.

## 3. When to self-assign

- **No task given** ("start", "go", "work on the board", a link to `WORK.md`): pick a row with the rules in
  section 6 and start without asking.
- **A task named** ("do E2a", "fix the rail sound"): do that one. Claim its row first, or add a row for it if
  there is none.
- **Only a question**: answer it and claim nothing.

What the human says always beats the board's order.

## 4. Who you work for, and your agent name

1. Find the person's GitHub name: ask once if you can't tell, otherwise take it from
   `git config user.email` / `gh auth status` / the remote. Write it in the form GitHub shows it (`ed-slopper`).
2. Give yourself an agent name: a short plain word (`puffin`, `otter`, `birch`). Save it once per clone:
   `git config --local agent.name <word>`. If the clone already has one, use it. One person can run several
   agents; each agent needs its own clone (or worktree) and its own name.
3. Every claim, branch and commit carries `<person>/<agent>`, for example `ed-slopper/puffin`. In a commit,
   put it on its own line above the `Co-Authored-By` lines.

## 5. What is taken

Before you pick, `git fetch --all --prune` and look at:

- every row on the board with anything in **Who** (claimed), on `main` **and** on every other branch;
- every file someone else changed in the last 7 days, on any branch or open pull request:
  `git log --all --since=7.days --name-only --format='%an %s'` and `gh pr list` / `gh pr diff <n> --name-only`.
  You may read those files; do not change them unless your row needs it and their owner is you, or the human
  says so.

## 6. How to pick

Go through the tiers in order: P0, P1, P2, P3, P4, P5. Take the **first** open row (empty Who) where:

- **Area** is your person, `unowned` or `any` (never `humans`: those are decisions for people, not agents);
- every row in **Needs** is in the Done table;
- none of the files it will touch are taken (section 5).

If the row is big, split it first (into `E3a`, `E3b`, ... rows with the same Area and Needs) and claim only the
part you will finish this session.

## 7. How to claim

1. On `main` (or the board branch, if a human moved the board), write `<person>/<agent> (<YYYY-MM-DD>)` in
   the row's **Who**.
2. Commit **only `WORK.md`**, message `claim <ID>: <task in a few words>`, and push straight away, before you start
   the work. Push to `main` if you can. If you can't, push a branch `claim-<ID>-<words>-<person>-<agent>` and open
   a pull request.
3. If the push is rejected, `git pull --rebase`, re-read the board, and if the row is still free try once more.
   If someone else got it, pick again.

## 8. Branches

Names: `<what>-<ID>-<two to five plain words>-<person>-<agent>`, where `<what>` is:

| what | for |
|---|---|
| `claim` | a claim that could not be pushed to `main` |
| `join` | adding yourself to a shared row |
| `done` | moving a row to Done |
| `board` | any other change to the board or these rules |
| `work` | the code itself |

Examples: `work-E2a-big-archive-reader-ed-slopper-puffin`, `board-split-g5-animation-ed-slopper-puffin`.
Never a bare name such as `claim-12`, `patch-1` or `fix`.

When several agents of one person are working at the same time, each uses its own `work-` branch and merges it
back into that person's branch (`main` while there is only one person) when its row is done. One agent on its
own may commit straight to the person's branch.

## 9. Finishing and stopping

- **Done**: move the row from its tier to the **Done** table with the date and the commit hash, in the same
  commit as the last code change or right after it. Every gap you found becomes a new open row (next free ID in
  that tier, Notes: `Gap of <ID>`).
- **Stopping early**: clear your name from Who, and write in Notes what is finished, what is left and where
  (files, addresses). Commit and push that.

## 10. Sharing a row

Two people, or two agents, may be on one row **only when a human asks**. Add the second name with `+`:
`ed-slopper/puffin (2026-10-10) + ed-slopper/otter (2026-10-11)`, and write in Notes who takes which part
(functions, files). Push it like a claim, on a `join-` branch if not on `main`.

## 11. Changing the board

Anyone may change priorities, rows, areas or these rules, and their agent does it when asked. On its own an
agent may only **add rows, split rows and fix facts** (a wrong address, a file that moved). Every change, by
anyone, gets one line in **Changes to the board** at the bottom of `WORK.md`:
`2026-10-10 ed-slopper/puffin: split G5 into G5a-G5d`.

## 12. Areas

Who owns what. Worked out from who committed what (one person so far); **owners, please correct this table.**
A row's Area is a person, `unowned` (nobody's yet, anyone may take it), `any` (shared files, anyone) or
`humans` (decisions agents must not take).

| Person | Branch | Area | Files |
|---|---|---|---|
| ed-slopper | `main` | Everything so far: the Rust rewrite, the tools, the notes, the mods | `tricky-rs/**`, `tools/**`, `notes/**`, `mods/**`, `ghidra/**` |

New people: add your own line (in a `board-` commit) before you claim a row, and take rows that are `unowned` or
`any` until an owner hands an area over.

## 13. Rules of the project

- **No game files in the repo, ever.** No ISO, no `SLUS_203.26`, no `.BIG`, `.pbd`, `.ssh`, `.mpf`, `.afl`, `.bnk`,
  `.mus` or anything extracted from them (textures, models, sounds, JSON level projects), no decompiler output,
  no assembly listings, no Ghidra projects or exports. Everyone uses their own copy of the game. The
  `.gitignore` is an allow-list: only our own folders are let in. Numbers and names read out of the exe
  (constants, table values, function names) are fine.
- **Port what the game does, 1:1.** Same formulas, same order, same constants, same units (cm, 60 Hz ticks), same
  bugs. A row is done when the code is ported, not when it feels right: tuning by eye is not porting.
- **Write the original's address next to every recreated function**, in its doc comment, with the name we gave
  it in Ghidra: ``/// `Boarder_ForwardDrag` 0x109cb8``. A function that ports several originals lists them all.
  Addresses are always `SLUS_203.26` and need no version tag; anything else must say which version.
- **Mark everything that is ours and not the game's** with the tag `STANDIN:` and what it stands in for:
  `// STANDIN: SSX-Library JSON project folder, for the game's own .pbd reader (0x25xxxx)`.
  `// STANDIN: our choice of animation clip, for the boarder's animation state machine (G5)`.
  Bevy itself (window, GPU, audio output) is our host, not a stand-in; what we draw and how is the game's and
  must be ported. `grep -rn STANDIN tricky-rs` must list every stand-in left (board row X1 empties it).
- **Do not chase a gap outside your task.** Write it in the port notes of its system and add it as a new open row.
- **Say which build.** Every address is `SLUS_203.26` (section 2). If more than one version is ever used, never
  mix their addresses in one file without a version tag on each.

## 14. Decompiling

- Use the **Ghidra MCP server** to read the game's code: <https://github.com/bethington/ghidra-mcp>. Use it
  whenever a task needs the original: a function, a constant, a struct layout, who calls what. Do not guess, and
  do not work from memory of other projects about this game (SSX-Library, the old modding tools, forum posts)
  when the binary can be read. Those can tell you where to look, never what the code does.
- Check your tool list for its tools first: `decompile_function`, `list_methods`, `find_functions`,
  `get_xrefs_to`, `get_xrefs_from`, `read_memory`, `rename_function`.
- If they are not there, tell the human **once**, give them the install steps (below), and carry on with
  whatever does not need the binary. If the human has a local export of the decompiler output (the
  `decomp/` folder made by `ghidra/scripts/ExportDecomp.java`, kept outside the repo), you may read it
  instead, but it does not have the newest names.
- **Check which program is open** in Ghidra before trusting an address: it must be `SLUS_203.26` with the md5 in
  section 2.
- **Rename functions and fields in Ghidra as you work them out**, so the next agent sees them. Then add the same
  names to `ghidra/symbols.txt` (`G <addr> <name>` for a global function, `F <addr> <class> <method>` for a class's, `D <addr> <name>` for global data, `L <addr> <class> <name>` for a class's data) in the same commit
  as your code, because the Ghidra project is not in the repo and this file is how everyone else gets your
  names (`ghidra/scripts/ApplySsxSymbols.java` applies it).
- Naming (checked by `python tools/function_index.py names`, which must pass before you push):
  - Free functions: `Module_Verb`, one prefix per module, the thing the function works on: `Boarder_Wipeout`,
    `Score_Crash`, `TriggerScript_ExecOp`, `Loc_GetString`. States and motions are modules of their own:
    `SpinState_Update`, `WipeoutMotion_Enter`, `GroundMotion_Update`. One more `_` may separate a part of a
    module: `Fx_BrakeFan_Update`. A system may have several modules (`Audio_`, `Music_`, `Sfx_` are all audio).
  - Class methods: `cClass::Method` (CamelCase, no `_`) only when the function is proven to belong to the class:
    a virtual (in its vtable), a constructor (`cClass::cClass`) or a call on an object of that class. Virtuals
    not yet understood stay `cClass::vfN` (Ghidra shows `vfN_<addr>`, from `tools/rtti_scan.py`). Otherwise use
    a free-function name, even if the first argument looks like `this`.
  - Global data: `gName` or `gModule_Name` (`gApp`, `gCheat_Mallora`). Class data from `tools/rtti_scan.py`:
    `typeinfo`, `vtable`, `vtable_N`. Fields in notes and comments: `boarder+0x12c timescale`.
  - Exceptions: classes with type info keep the game's own name whatever its style (`CamCamera`,
    `bxSphereTree`, `tPS2DrawState`, gcc's `__class_type_info`); classes without it get our `cName`
    (`cMenuManager`). The type-info getters are `cClass::__tf`. Library functions we only host keep their
    standard names (`memcpy`, `bsearch`, `__rtti_si`, the kernel's `CreateThread`); EA's library prefixes stay as
    EA wrote them (`SND_`, `SNDVoice_`, `PF_`). Rust has no `::` in an identifier, so source and notes may
    write `cClass::Method` as `cClass_Method`; the index treats both as one name. Spelling slips in names
    others already use are kept (`World_CellActivate_RunPersistant`).
  - Keep a name once others use it; rename only to fix a wrong one. When you do, add `<addr> <old> <new>` to
    `ghidra/renamed.txt`: the index keeps counting the old name in `tricky-rs/src` until row F9 updates it.
- If Ghidra cannot decompile a function (VU microcode, inline `qmfc2`/MMI, hand-written asm), write in the port
  notes how you read it instead (disassembly by hand, PCSX2 debugger trace, VU disassembler) and what you are
  still unsure of.
- Port notes, one file per system, live in `tricky-rs/docs/port-notes/` (index: its `README.md`); add to the
  system's file, or start one and list it in the index. Write proven facts and interpretations apart: "proven" = read from code, "inferred" = a guess.

- **The running game: PINE** (PCSX2's own IPC: Settings > Advanced > PINE, slot 28011, stock PCSX2). Ghidra
  shows the code; PINE shows the game while it runs, with the player's own disc: read and write any EE address
  (batched, so a whole struct is one round trip) and save or load states. Use it for what the code alone can't
  tell you: what a field holds during a race, which addresses change on a tick, the state before and after a
  tick to replay through our code (F4b). `tools/pine.py` is our client (`status`, `read`, `floats`, `dump`,
  `save`, `load`, `watch`); run `status` first and check the id is SLUS-20326 before trusting an address. PCSX2
  refuses PINE while no game runs (reads work while paused), and serves one PINE client at a time: a
  connection left open (another tool, a stuck script) makes every other client time out. PINE has no breakpoints and no register access, so
  checks are per tick (state before, state after), not per function call. Dumps and traces are the game's data:
  keep them on your PC. Only addresses, names and numbers go into the repo, as with Ghidra.
- PCSX2-MCP (a patched PCSX2 with a debug server: breakpoints, registers) was tried for F4f and F4g and is **not
  recommended**: dozens of breakpoints wedged it, breakpoints survive a game reboot, and it hangs while PCSX2 is
  paused. `tools/pcsx2_debug.py` (needs that build) made the captures F4f and F4g describe; don't depend on it.

**Installing ghidra-mcp** (for the human): install Ghidra 12.x and JDK 21; install the Emotion Engine extension
(`ghidra-emotionengine-reloaded`) through File > Install Extensions; import `SLUS_203.26` (language
`r5900:LE:32:default`), auto-analyse, run `ApplySsxSymbols.java` on `ghidra/symbols.txt`. Then follow the
README of <https://github.com/bethington/ghidra-mcp>: install its Ghidra plugin, start its server from the
CodeBrowser with the program open, and add it to Claude (`claude mcp add` with the command its README gives).
Restart Claude and check that the ghidra-mcp tools (`force_decompile`, `disassemble_function`, `get_functions`, ...)
are in the tool list. Snags seen on Windows (2026-10-10): a project made on Linux opens only after its owner in
`ssxtricky.rep/project.prp` is changed to your user name; ghidra-mcp's Gradle build asks for exactly JDK 21
(install one, or build with your newer JDK and `options.release = 21`); its plugin is enabled in the **project**
window (File > Configure > Utility), and Ghidra must be restarted after `deploy` for it to appear. Point the bridge
at `uv.exe` by its full path.

## 15. Building and checking

- `cd tricky-rs && cargo run --release -- ..\levels\gari`; self-tests: `TRICKY_SIM=600`, `TRICKY_RAILTEST=1`,
  `TRICKY_TRICKTEST=1` (see `tricky-rs/README.md`). On Windows use the MSVC Rust toolchain (the GNU one needs
  `dlltool`).
- Run the build and the self-test that covers your system before marking a row done. When the comparison
  harness exists (row F4), a ported function is **checked** only when it matches the original on it.

- CI (`.github/workflows/`): `Guard` runs on every push (no game files, our Python tools, names), `Function
  runner` when `tools/r5900/` changes, `tricky-rs` (Bevy, about 20 minutes cold) only when `tricky-rs/src/`,
  `Cargo.toml` or `Cargo.lock` change, or by hand (docs in `tricky-rs/docs/` don't build). Don't build tricky-rs, locally or by pushing to it, unless a change needs testing or the human asks.

## 16. End of every session

Finish by writing, for each person with an agent on this repo, a ready-to-paste prompt for their next task.
Each prompt must work with no other context, and contain:

- repo URL and the branch to work on;
- the row ID and the task in one sentence;
- the original addresses and the binary (`SLUS_203.26`, md5 above);
- our files it touches;
- how to tell it is done (what to build or run, what must match);
- what not to touch (taken files, other people's areas, `humans` rows).
