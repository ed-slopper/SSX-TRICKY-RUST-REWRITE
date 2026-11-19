#!/usr/bin/env python3
"""Talk to a running PCSX2 over PINE, its built-in IPC (Settings > Advanced > PINE, slot 28011).

This is how the project reads the running game (AGENTS.md §14): stock PCSX2, no patched build, no MCP server.
PINE reads and writes memory and saves and loads states; it has no breakpoints or register access.

    python tools/pine.py status                         emulator and game: version, state, id, title
    python tools/pine.py read <addr> [count]            32-bit words from EE memory
    python tools/pine.py floats <addr> [count]          the same words as floats
    python tools/pine.py dump <addr> <bytes> <out.bin>  a block of memory to a file (keep it on your PC)
    python tools/pine.py save <slot> | load <slot>      savestate slot 0-9
    python tools/pine.py watch <addr> <bytes> <seconds> <out.txt>
                                                        poll a block, write every distinct copy with a timestamp
    python tools/pine.py hook on|off                    the pad word hook (F4b3k): each rider's pad word kept
                                                        where `record ... hookpad` reads it (in RAM only)
    python tools/pine.py riders                          the race object and its riders (addresses, place, speed)
    python tools/pine.py record <seconds> <out.txt> [<bytes>] [<off>:<len> ...]
                                                        each tick: the race tick and every rider's first
                                                        <bytes> (default 0x600), kept only when the tick did
                                                        not change while reading (F4b; keep it on your PC);
                                                        each <off>:<len> (hex) adds that window of every rider
                                                        after the pad, read in one message with the rider
                                                        (57b0:64 = the flight prediction, F4h2; an offset
                                                        may be negative: -170:74 = the spin object, F4b3a;
                                                        `>` follows a pointer: 5ae0>610:10 = the handler's
                                                        bump control, F4b3i; `hookpad`: each rider's pad
                                                        word from the hook, F4b3k; `rollers` with L700:78:
                                                        the knocked objects among each rider's pairs,
                                                        F4b3k3; `flags`: every course object's flags
                                                        +0xe8 as they change, F4b3k4); with <bytes> >= 0x700, a
                                                        sample whose rider body looks the last frame's is read
                                                        again (F4h5d; header `posed 1`); the riders'
                                                        characters go to <out>.stats; a new race with other
                                                        characters stops it
    python tools/pine.py clock <object> <seconds> <out.txt>
                                                        each tick: the race tick, course object <object>'s
                                                        controller (+0xe4) and flags (+0xe8), its node's rate,
                                                        time, start and end (controller - 0x30 + 0x20, a
                                                        cAnimObjectNode; F4h5l), every rider's position (the
                                                        floats as their bits) and tick count (+0x5900), read at
                                                        the frame's end as `record` does (keep it on your PC)
    python tools/pine.py counters [<start> <end>]       find tick counters and clocks: words in .data/.bss
                                                        (default 0x31D300-0x40C174) that climb about 60 a
                                                        second, or floats that climb about 1 a second

A game must be booted: PCSX2 refuses PINE commands while none is (reads work while it is paused). PCSX2 serves
one PINE client at a time: if every command times out, another program is holding the connection. Anything read from the game is
the game's data: keep dumps and watch files on your own PC, never in the repo. Board rows F4b, F4h.
"""
import socket
import struct
import sys
import time

PORT = 28011
READ8, READ16, READ32, READ64 = 0, 1, 2, 3
WRITE8, WRITE16, WRITE32, WRITE64 = 4, 5, 6, 7
VERSION, SAVE_STATE, LOAD_STATE, TITLE, ID, UUID, GAME_VERSION, STATUS = 8, 9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF
STATES = {0: "running", 1: "paused", 2: "shutdown"}
BATCH = 4096  # commands per message, well under PINE's buffer


class Pine:
    def __init__(self, port=PORT):
        if sys.platform == "win32":
            self.sock = socket.create_connection(("127.0.0.1", port), timeout=5)
            return
        # Linux and macOS: a Unix socket in the runtime folder (pcsx2.sock, or pcsx2.sock.<slot> off the default);
        # the Flatpak build's runtime folder is seen from outside under .flatpak/net.pcsx2.PCSX2/xdg-run
        import os
        name = "pcsx2.sock" if port == PORT else f"pcsx2.sock.{port}"
        run = os.environ.get("XDG_RUNTIME_DIR") or os.environ.get("TMPDIR") or "/tmp"
        paths = [os.path.join(run, name), os.path.join(run, ".flatpak", "net.pcsx2.PCSX2", "xdg-run", name)]
        for path in paths:
            if os.path.exists(path):
                self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                self.sock.settimeout(5)
                self.sock.connect(path)
                return
        raise ConnectionError(f"no PINE socket at {' or '.join(paths)} (is PCSX2 running with PINE on?)")

    def _recv(self, n):
        out = b""
        while len(out) < n:
            chunk = self.sock.recv(n - len(out))
            if not chunk:
                raise ConnectionError("PCSX2 closed the connection")
            out += chunk
        return out

    def batch(self, commands):
        """Send several (opcode, args) in one message; returns the reply bytes after the result code."""
        body = b"".join(struct.pack("<B", op) + args for op, args in commands)
        self.sock.sendall(struct.pack("<I", 4 + len(body)) + body)
        size, = struct.unpack("<I", self._recv(4))
        reply = self._recv(size - 4)
        if reply[0] != 0:
            raise RuntimeError("PINE refused the command (is a game running?)")
        return reply[1:]

    def call(self, op, args=b""):
        return self.batch([(op, args)])

    def string(self, op):
        data = self.call(op)
        n, = struct.unpack_from("<I", data)
        return data[4:4 + n].rstrip(b"\0").decode("utf-8", "replace")

    def status(self):
        return STATES.get(struct.unpack("<I", self.call(STATUS))[0], "?")

    def read32(self, addr):
        return struct.unpack("<I", self.call(READ32, struct.pack("<I", addr)))[0]

    def write32(self, addr, value):
        self.call(WRITE32, struct.pack("<II", addr, value))

    def read_block(self, addr, length):
        """`length` bytes from `addr`, 8 at a time, many reads per message."""
        out = b""
        end = addr + length
        a = addr - addr % 8
        while a < end:
            n = min(BATCH, (end - a + 7) // 8)
            out += self.batch([(READ64, struct.pack("<I", a + 8 * k)) for k in range(n)])
            a += 8 * n
        skip = addr % 8
        return out[skip:skip + length]

    def read_blocks(self, ranges):
        """Several (addr, length) blocks in as few messages as possible (one, when they fit), so they are read
        as close together in time as the emulator allows."""
        cmds, cuts = [], []
        for addr, length in ranges:
            a = addr - addr % 8
            n = (addr + length - a + 7) // 8
            cmds += [(READ64, struct.pack("<I", a + 8 * k)) for k in range(n)]
            cuts.append((8 * n, addr % 8, length))
        out = b"".join(self.batch(cmds[i:i + BATCH]) for i in range(0, len(cmds), BATCH))
        blocks, at = [], 0
        for size, skip, length in cuts:
            blocks.append(out[at + skip:at + skip + length])
            at += size
        return blocks

    def save_state(self, slot):
        self.call(SAVE_STATE, struct.pack("<B", slot))

    def load_state(self, slot):
        self.call(LOAD_STATE, struct.pack("<B", slot))


# Found 2026-10-10 over PINE (tricky-rs/docs/checking.md, F4b): stats entries are fixed in .data, one per rider
# slot; a rider struct points at its entry from +0x464. The race object holds the riders at +0xC4, their count at
# +0x88 and the race tick at +0x18 (60 a second).
STATS_TABLE, STATS_STRIDE = 0x32DB70, 0x84

# The pad word hook (F4b3k). `cBoarder::vf7` 0x117198 asks the rider's controller for the tick's pad word
# (`cPlayer::GetControls` 0x151250, `cComputer::GetControls` 0x1384b8) on its own stack and hands it to
# `Boarder_UpdateControl` 0x11c9a0 (`jal` at 0x117b78, a0 the handler, a1 the pad, s1 the rider): nothing keeps it.
# The hook sends that `jal` through a stub in the zero gap after .vutext (0x31D2A0, 96 bytes, notes/findings.md)
# that copies the pad's first word and the rider's speed-sample count +0x5900 (the tick it is for: `Boarder_AddSpeedSample`
# 0x155dc8 adds 1 before) into the gap after .data (0x35F6D0, 48 bytes: 8 bytes for each slot +0x460 0..5), then jumps
# on to `Boarder_UpdateControl`. Only temporaries (t0..t2) are touched, which the call clobbers anyway, so the game runs
# as before. In RAM only, until the game is booted again.
HOOK_AT, HOOK_JAL, HOOK_CODE, HOOK_DATA, HOOK_SLOTS = 0x117B78, 0x0C047268, 0x31D2A0, 0x35F6D0, 6
RIDER_SLOT = 0x460


def hook_code():
    """The stub's words (MIPS: t0 8, t1 9, t2 10, a1 5, s1 17)."""
    def i(op, rs, rt, imm):
        return op << 26 | rs << 21 | rt << 16 | imm & 0xFFFF
    lw, sw = (lambda rt, off, base: i(0x23, base, rt, off)), (lambda rt, off, base: i(0x2B, base, rt, off))
    lo = HOOK_DATA - 0x360000  # lui t1, 0x36 ; the slot's 8 bytes at -0x930
    return [
        lw(8, RIDER_SLOT, 17),               # lw    t0, 0x460(s1)     the rider's slot
        i(0x0B, 8, 9, HOOK_SLOTS),           # sltiu t1, t0, 6
        i(0x04, 9, 0, 7),                    # beq   t1, zero, +7      (to the j)
        8 << 16 | 8 << 11 | 3 << 6,          # sll   t0, t0, 3         (delay slot)
        i(0x0F, 0, 9, 0x36),                 # lui   t1, 0x36
        9 << 21 | 8 << 16 | 9 << 11 | 0x21,  # addu  t1, t1, t0
        lw(10, 0, 5),                        # lw    t2, 0(a1)         the pad word
        sw(10, lo, 9),                       # sw    t2, -0x930(t1)
        lw(10, 0x5900, 17),                  # lw    t2, 0x5900(s1)    the speed-sample count
        sw(10, lo + 4, 9),                   # sw    t2, -0x92c(t1)
        0x08000000 | 0x11C9A0 >> 2,          # j     Boarder_UpdateControl
        0,                                   # nop
    ]


def hook(p, on):
    """Put the pad word hook in (or take it out): the stub first, then the call to it."""
    if p.read32(HOOK_AT) not in (HOOK_JAL, 0x0C000000 | HOOK_CODE >> 2):
        sys.exit(f"{HOOK_AT:#x} is not the call to Boarder_UpdateControl: not SLUS_203.26?")
    if on:
        for k in range(2 * HOOK_SLOTS):
            p.write32(HOOK_DATA + 4 * k, 0)
        for k, w in enumerate(hook_code()):
            p.write32(HOOK_CODE + 4 * k, w)
        p.write32(HOOK_AT, 0x0C000000 | HOOK_CODE >> 2)
    else:
        p.write32(HOOK_AT, HOOK_JAL)
RACE_TICK, RACE_COUNT, RACE_RIDERS = 0x18, 0x88, 0xC4
# the course id (0 garibald ... 3 mesablanca ...: `Camera_GetCourseCmlName` 0x178330's order; F4h5k)
COURSE_ID = 0x32DE88
# the course object (`gWorld`)
WORLD = 0x346488
RIDER_POS, RIDER_VEL, RIDER_TIMESCALE, RIDER_PLACE, RIDER_STATS = 0x140, 0x150, 0x12C, 0x110, 0x464
WORLD = 0x346488  # the course's world (`World_QueryInstances` 0x25b878's first argument)
RIDER_TICKS = 0x5900  # rider+0x5820 (score) +0xe0: the speed-sample count, +1.0 a tick
# The pad: Sony's pad library writes each controller's data into a double buffer (two records 0x80 apart):
# status, mode (0x73 analog, 0x79 pressure), buttons (2 bytes, active low), right x, right y, left x, left y,
# then 12 pressures (right left up down triangle circle cross square L1 R1 L2 R2). Found at 0x845940 / 0x8459C0.
PAD_BYTES = 20


def find_pad(p):
    """The first controller's double buffer: two pad records 0x80 apart in the same mode."""
    import re
    ram = p.read_block(0x100000, 0x1F00000)
    for m in re.finditer(rb"[\x00\x01][\x73\x79]", ram):
        o = m.start()
        if o % 0x40 == 0 and ram[o + 0x81:o + 0x82] == ram[o + 1:o + 2]:
            return 0x100000 + o
    return None


def find_race(p):
    """The race object: scan RAM for riders (by their stats pointer), then for the array that holds them all."""
    ram = p.read_block(0, 0x2000000)
    words = struct.unpack(f"<{len(ram) // 4}I", ram)
    entries = {STATS_TABLE + k * STATS_STRIDE for k in range(12)}
    riders = {4 * i - RIDER_STATS for i, w in enumerate(words) if w in entries and 4 * i > 0x400000
              and words[(4 * i - RIDER_STATS + 0x420) // 4] <= 2}
    for i in range(len(words) - 2):
        if words[i] in riders:
            race = 4 * i - RACE_RIDERS
            n = words[(race + RACE_COUNT) // 4]
            if 1 <= n <= 8 and all(words[i + k] in riders for k in range(n)):
                return race, [words[i + k] for k in range(n)]
    return None, []


def stale_bodies(prev, now, fresh):
    """The riders whose body (rider+0x480..0x700: `Boarder_UpdateBroadphase` 0x126748's, posed by `cPostAI::vf1`
    0x114730 after every rider ticked) looks like the last frame's while the rider moved (F4h5a, F4h5d): byte for byte
    the previous sample's; or lagging (its centre less the position is the last fresh sample's moved back by the step);
    or the previous body moved whole by something other than the step (only the frame's pushes, F4b3y). The same tests
    as `Twins` in `tricky-rs/game/tests/ground_motion.rs`. `prev`, `now`: the riders' blocks of the samples a tick
    apart; `fresh`: each rider's last block not found stale (or None)."""
    def v(b, o):
        return struct.unpack_from("<3f", b, o)

    def sub(a, b):
        return [x - y for x, y in zip(a, b)]

    def norm(a):
        return sum(x * x for x in a) ** 0.5

    bad = []
    for i, (p, r) in enumerate(zip(prev, now)):
        if r[0x140:0x14c] == p[0x140:0x14c]:
            continue
        step = sub(v(r, 0x140), v(p, 0x140))
        if r[0x480:0x700] == p[0x480:0x700]:
            bad.append(i)
            continue
        if fresh[i] is not None:
            off = sub(v(r, 0x480), v(r, 0x140))
            was = sub(v(fresh[i], 0x480), v(fresh[i], 0x140))
            es = norm([o + s - w for o, s, w in zip(off, step, was)])
            if norm(step) > 2.0 and es < 3.0 and norm(sub(off, was)) > 2.0 * es + 2.0:
                bad.append(i)
                continue
        count = min(struct.unpack_from("<I", r, 0x498)[0], 18)
        if count and r[0x498:0x49c] == p[0x498:0x49c]:
            shift = sub(v(r, 0x480), v(p, 0x480))
            whole = all(all(abs(x - y) < 0.05 for x, y in zip(sub(v(r, 0x4a0 + 0x20 * s), v(p, 0x4a0 + 0x20 * s)), shift))
                        for s in range(count))
            if whole and norm(sub(shift, step)) > 0.5:
                bad.append(i)
    return bad


def find_counters(p, start=0x31D300, end=0x40C174, snaps=4, gap=0.5):
    """Words that rise steadily with time across a few snapshots: ints at ~60/s (ticks), floats at ~1/s (seconds)."""
    shots = []
    for _ in range(snaps):
        t = time.time()
        shots.append((t, p.read_block(start, end - start)))
        time.sleep(gap)
    n = (end - start) // 4
    ints = [struct.unpack(f"<{n}I", s[:4 * n]) for _, s in shots]
    flts = [struct.unpack(f"<{n}f", s[:4 * n]) for _, s in shots]
    dts = [b[0] - a[0] for a, b in zip(shots, shots[1:])]
    found = []
    for k in range(n):
        di = [ints[j + 1][k] - ints[j][k] for j in range(snaps - 1)]
        if all(d > 0 for d in di):
            rates = [d / dt for d, dt in zip(di, dts)]
            if all(30 <= r <= 90 for r in rates):
                found.append((start + 4 * k, "int", ints[-1][k], sum(rates) / len(rates)))
                continue
        df = [flts[j + 1][k] - flts[j][k] for j in range(snaps - 1)]
        if all(d > 0 for d in df) and all(abs(v) < 1e7 for v in (flts[0][k], flts[-1][k])):
            rates = [d / dt for d, dt in zip(df, dts)]
            if all(0.5 <= r <= 1.5 for r in rates):
                found.append((start + 4 * k, "float", flts[-1][k], sum(rates) / len(rates)))
    return found


def main(argv):
    p = Pine()
    cmd = argv[:1]
    if cmd == ["status"]:
        for label, get in (("PCSX2", lambda: p.string(VERSION)), ("state", p.status), ("id", lambda: p.string(ID)),
                           ("title", lambda: p.string(TITLE)), ("version", lambda: p.string(GAME_VERSION))):
            try:
                print(f"{label}:", get())
            except RuntimeError:
                print(f"{label}: (refused: is a game running?)")
    elif cmd in (["read"], ["floats"]) and len(argv) >= 2:
        addr, count = int(argv[1], 16), int(argv[2]) if len(argv) > 2 else 1
        words = struct.unpack(f"<{count}I", p.read_block(addr, 4 * count))
        for k, w in enumerate(words):
            shown = f"{struct.unpack('<f', struct.pack('<I', w))[0]:.6g}" if cmd == ["floats"] else f"{w:#010x}"
            print(f"{addr + 4 * k:#010x}  {shown}")
    elif cmd == ["dump"] and len(argv) == 4:
        data = p.read_block(int(argv[1], 16), int(argv[2], 0))
        open(argv[3], "wb").write(data)
        print(f"{len(data)} bytes to {argv[3]}")
    elif cmd in (["save"], ["load"]) and len(argv) == 2:
        (p.save_state if cmd == ["save"] else p.load_state)(int(argv[1]))
        print(f"{cmd[0]} slot {argv[1]}: done")
    elif cmd == ["watch"] and len(argv) == 5:
        addr, length, seconds = int(argv[1], 16), int(argv[2], 0), float(argv[3])
        last, kept, polls, t0 = None, 0, 0, time.time()
        with open(argv[4], "w", encoding="ascii") as f:
            while time.time() - t0 < seconds:
                data = p.read_block(addr, length)
                polls += 1
                if data != last:
                    f.write(f"{time.time() - t0:.4f} {data.hex()}\n")
                    last, kept = data, kept + 1
        print(f"{polls} polls, {kept} distinct copies of {length} bytes at {addr:#x} in {argv[4]}")
    elif cmd == ["hook"] and len(argv) == 2 and argv[1] in ("on", "off"):
        hook(p, argv[1] == "on")
        print(f"pad word hook {argv[1]}: {HOOK_AT:#x} = {p.read32(HOOK_AT):#010x}")
    elif cmd == ["riders"]:
        race, riders = find_race(p)
        if race is None:
            sys.exit("no race found (is a race running?)")
        print(f"race object {race:#x}, tick {p.read32(race + RACE_TICK)}, {len(riders)} riders")
        for r in riders:
            vel = struct.unpack("<3f", p.read_block(r + RIDER_VEL, 12))
            ts, = struct.unpack("<f", p.read_block(r + RIDER_TIMESCALE, 4))
            speed = sum(v * v for v in vel) ** 0.5
            print(f"  rider {r:#x}  place {p.read32(r + RIDER_PLACE)}  {speed:7.1f} cm/s  timescale {ts:.4f}")
    elif cmd == ["record"] and len(argv) >= 3:
        seconds, out = float(argv[1]), argv[2]
        size = int(argv[3], 0) if len(argv) >= 4 else 0x600
        # a window's offset may follow pointers: `5ae0>600:230` reads the word at rider+0x5ae0 and then 0x230 bytes
        # at that word + 0x600 (each `>k` steps through one more pointer)
        # `L700:78` walks the list whose head is at rider+0x700 (`Boarder_RiderCollisions` 0x123fc8's pairs: a node
        # has the two heads at +0, +4 and their next nodes at +8, +0xc, its side `Obj_HasId` 0x110838 = node+4 is
        # ours): each node as its address and 0x10 bytes, as many as fit in 0x78 (zeros after the last)
        # `hookpad` puts the pad word hook in (F4b3k, `hook`) and keeps each rider's 8 bytes of it (its pad word and the
        # speed-sample count of the tick it was made in; header `extra hookpad 8`)
        # `rollers` (F4b3k3, with an `L700` list): the `cRollerNode`s among each rider's pairs (the pair's other proxy,
        # proxy +0x84 its node, node +0x14 == 0 a roller, as `Boarder_InstanceCollisions` 0x125088 finds them): a
        # count byte, then each roller as its address, its instance (+0x28), 0x150 bytes from +0x30 (the rigid
        # body, mass, timers, the centre of mass +0x130, asleep +0x140, the sphere tree's head +0x150) and 0x20 from
        # +0x2a0 (its box); header `extra rollers 0 instances <base>`, the base of
        # the course's instances (`Course_GetInstance` 0x254a28: *(*(0x346488 + 4) + 0x48) + 0x100 × index)
        hooked = "hookpad" in argv[4:]
        rolled = "rollers" in argv[4:]
        # `flags` (F4b3k4): every course object's flags (+0xe8; `Course_GetInstance` 0x254a28: *(*(0x346488 + 4) +
        # 0x48) + 0x100 × index, the count at *(0x346488 + 4) + 0xc), read after the batch while the tick holds; each
        # line ends `F` and the objects whose flags changed since the last line kept, `index:flags` (hex, comma-
        # separated; `-` for none; the first line all of them); header `flags <count>`
        flagged = "flags" in argv[4:]
        # `clock` (F4b3k2): the race's clock (race +0x14, a float of seconds, which `AIComputer_ControlsCountdown`
        # 0x138628 reads), read with the race tick; each line ends ` C ` and its bits (hex); header `clock 1`
        clocked = "clock" in argv[4:]
        wins_args = [w for w in argv[4:] if w not in ("hookpad", "rollers", "flags", "clock")]
        lists = [(w, int(w[1:].split(":")[0], 16), int(w.split(":")[1], 16)) for w in wins_args if w.startswith("L")]
        if rolled and not any(o == 0x700 for _, o, _ in lists):
            sys.exit("rollers needs the pair list L700:78")
        extra = [(o, [int(x, 16) for x in o.split(">")], int(n, 16))
                 for o, n in (w.split(":") for w in wins_args if not w.startswith("L"))]
        race, riders = find_race(p)
        if race is None:
            sys.exit("no race found (is a race running?)")
        pad = find_pad(p)
        n_inst, flag_at, last_flags = 0, [], None
        if flagged:
            head_at = p.read32(WORLD + 4)
            n_inst, base = p.read32(head_at + 0xc), p.read32(head_at + 0x48)
            flag_at = [(base + 0x100 * j + 0xe8, 4) for j in range(n_inst)]
        if hooked:
            hook(p, True)
            slots = [p.read32(r + RIDER_SLOT) for r in riders]
            assert all(s < HOOK_SLOTS for s in slots), slots
        # the riders' characters (rider+0x464 → 0x100 bytes) beside the recording, as the tests read them
        # (`<out>.stats` for `<out>.txt`, lines `rider stats-address hex`; TRICKY_STATS)
        def characters():
            return [(at, p.read_block(at, 0x100)) for at in (p.read32(r + RIDER_STATS) for r in riders)]
        chars = characters()
        with open(out.rsplit(".", 1)[0] + ".stats", "w", encoding="ascii") as f:
            for r, (at, b) in zip(riders, chars):
                f.write(f"{r:x} {at:x} {b.hex()}\n")
        kept, last, t0, counts = 0, None, time.time(), None
        k = len(riders)
        prev, fresh, seen, left = None, [None] * k, 0, 0
        with open(out, "w", encoding="ascii") as f:
            f.write(f"race {race:x} riders {' '.join(f'{r:x}' for r in riders)} bytes {size:x} pad {pad or 0:x}"
                    + "".join(f" extra {o} {n:x}" for o, _, n in extra)
                    + "".join(f" extra {w.split(':')[0]} {n:x}" for w, _, n in lists)
                    + (" extra hookpad 8" if hooked else "")
                    + (f" extra rollers 0 instances {p.read32(p.read32(WORLD + 4) + 0x48):x}" if rolled else "")
                    + (" posed 1" if size >= 0x700 else "")
                    + (f" flags {n_inst}" if flagged else "")
                    + (" clock 1" if clocked else "")
                    + f" course {p.read32(COURSE_ID)}" + "\n")
            while time.time() - t0 < seconds:
                tick = p.read32(race + RACE_TICK)
                if tick == last:
                    continue
                if last is not None and tick < last and characters() != chars:
                    # (a save state loaded keeps the characters: go on)
                    print(f"the race tick went back ({last} → {tick}) with other characters: a new race; stopped")
                    break
                # The race tick moves at the start of the frame, before the riders tick (slot 5 first, slot 0 last:
                # cBoarderHandler objects in list 8, then cPostAI in list 11 moves their bodies). Wait until every
                # rider has ticked (its speed-sample count +0x5900, a float that `Boarder_AddSpeedSample` 0x155dc8
                # adds 1 to each tick, has moved), at most 30 ms, so the sample is of the frame's end.
                until = time.time() + 0.03
                while counts is not None and time.time() < until:
                    now = p.read_blocks([(r + RIDER_TICKS, 4) for r in riders])
                    if all(a != b for a, b in zip(now, counts)):
                        break
                def read_batch():
                    # every rider's block and windows (and the pad) in one batch between two reads of the race tick, so
                    # they come from the same frame as near as the emulator allows (PINE reads while the EE runs)
                    # (a window through pointers: its pointers read in the same batch, its bytes in one more each
                    # level, all before the race tick is read again)
                    at = [(r + c[0]) & 0xFFFFFFFF for _, c, _ in extra for r in riders]
                    deep = [len(c) - 1 for _, c, _ in extra for r in riders]
                    plain = [(a, n) if d == 0 else (a, 4) for a, d, n in zip(at, deep, [n for _, _, n in extra for r in riders])]
                    ranges = [(race + RACE_TICK - 4 if clocked else race + RACE_TICK, 8 if clocked else 4)] + [(r, size) for r in riders] + plain
                    got = p.read_blocks(ranges + ([(pad, PAD_BYTES)] if pad else []) + ([(HOOK_DATA, 8 * HOOK_SLOTS)] if hooked else []))
                    table = got.pop() if hooked else None
                    wins = got[1 + k:1 + k + len(plain)]
                    steps = [c[1:] for _, c, _ in extra for r in riders]
                    lens = [n for _, _, n in extra for r in riders]
                    level = 0
                    while any(level < len(s) for s in steps):
                        todo = [i for i, s in enumerate(steps) if level < len(s)]
                        for i in todo:
                            at[i] = (struct.unpack("<I", wins[i])[0] + steps[i][level]) & 0x1FFFFFF  # (a stray pointer stays in RAM)
                        more = p.read_blocks([(at[i], lens[i] if level == len(steps[i]) - 1 else 4) for i in todo])
                        for i, b in zip(todo, more):
                            wins[i] = b
                        level += 1
                    walked = []
                    if lists:
                        heads = [(r + o, n) for _, o, n in lists for r in riders]
                        cur = [struct.unpack("<I", b)[0] for b in p.read_blocks([(h, 4) for h, _ in heads])]
                        walked = [b"" for _ in heads]
                        for _ in range(16):
                            todo = [i for i, c in enumerate(cur) if c and len(walked[i]) + 0x14 <= heads[i][1]]
                            if not todo:
                                break
                            nodes = p.read_blocks([(cur[i] & 0x1FFFFFF, 0x10) for i in todo])
                            for i, b in zip(todo, nodes):
                                walked[i] += struct.pack("<I", cur[i]) + b
                                side = struct.unpack("<I", b[4:8])[0] == heads[i][0]
                                cur[i] = struct.unpack("<I", b[8 + 4 * side:12 + 4 * side])[0]
                        walked = [w + bytes(n - len(w)) for w, (_, n) in zip(walked, heads)]
                    rollers = []
                    if rolled:
                        li = next(j for j, (_, o, _) in enumerate(lists) if o == 0x700)
                        found = []  # (rider, the other proxy)
                        for i in range(k):
                            w = walked[li * k + i]
                            for at in range(0, len(w) - 0x13, 0x14):
                                node = w[at:at + 0x14]
                                if node[:4] == bytes(4):
                                    break
                                a, b = struct.unpack("<II", node[4:12])
                                found.append((i, a if b == riders[i] + 0x700 else b))
                        owners = [struct.unpack("<I", x)[0] & 0x1FFFFFF for x in p.read_blocks([((o + 0x84) & 0x1FFFFFF, 4) for _, o in found])] if found else []
                        # (+0x14 the node's type, +0x28 its instance)
                        kinds = p.read_blocks([(o + 0x14, 0x18) for o in owners]) if owners else []
                        rolls = [(i, o, k2[0x14:0x18]) for (i, _), o, k2 in zip(found, owners, kinds) if struct.unpack("<I", k2[:4])[0] == 0]
                        bodies = p.read_blocks([r for _, o, _ in rolls for r in ((o + 0x30, 0x150), (o + 0x2a0, 0x20))]) if rolls else []
                        per = [[] for _ in range(k)]
                        for n, (i, o, inst) in enumerate(rolls):
                            per[i].append(struct.pack("<I", o) + inst + bodies[2 * n] + bodies[2 * n + 1])
                        rollers = [bytes([len(x)]) + b"".join(x) for x in per]
                    clock = got[0][:4] if clocked else b""
                    if struct.unpack("<I", got[0][-4:])[0] != tick or p.read32(race + RACE_TICK) != tick:
                        return None  # the game moved on while we read
                    hooks = [table[8 * s:8 * s + 8] for s in slots] if hooked else []
                    fl = [b"".join(p.read_blocks(flag_at))] if flagged else []
                    if flagged and p.read32(race + RACE_TICK) != tick:
                        return None
                    return got[1:1 + k] + ([got[-1]] if pad else []) + wins + walked + hooks + rollers + fl + ([clock] if clocked else [])
                # A body read while `cPostAI::vf1` 0x114730 poses the riders (0 to n-1, after every rider ticked) is the
                # last frame's (F4h5a: ~2 % of rider samples, mostly riders 5 and 4): read again, while the tick holds,
                # until no rider's body looks stale (`stale_bodies`), at most 12 ms; else keep the first read
                blocks = read_batch()
                if blocks is None:
                    continue  # the game moved on while we read: drop the sample
                if size >= 0x700 and prev is not None and prev[0] + 1 == tick:
                    until = time.time() + 0.012
                    bad = stale_bodies(prev[1], blocks[:k], fresh)
                    first = len(bad)
                    while bad and time.time() < until:
                        again = read_batch()
                        if again is None:
                            break
                        now = stale_bodies(prev[1], again[:k], fresh)
                        if len(now) <= len(bad):
                            blocks, bad = again, now
                    seen += first
                    left += len(bad)
                    for i in range(k):
                        if i not in bad:
                            fresh[i] = blocks[i]
                prev = (tick, blocks[:k])
                tail = ""
                if clocked:
                    tail += " C " + struct.unpack("<I", blocks.pop())[0].to_bytes(4, "big").hex()
                if flagged:
                    now = struct.unpack(f"<{n_inst}I", blocks.pop())
                    changed = [(j, x) for j, x in enumerate(now) if last_flags is None or last_flags[j] != x]
                    tail += " F " + (",".join(f"{j:x}:{x:x}" for j, x in changed) or "-")
                    last_flags = now
                f.write(f"tick {tick} " + " ".join(b.hex() for b in blocks) + tail + "\n")
                last, kept = tick, kept + 1
                counts = p.read_blocks([(r + RIDER_TICKS, 4) for r in riders])
        print(f"{kept} ticks of {len(riders)} riders in {seconds:g} s to {out}; bodies read stale {seen}, "
              f"still stale after reading again {left}")
    elif cmd == ["clock"] and len(argv) == 4:
        obj, seconds, out = int(argv[1]), float(argv[2]), argv[3]
        race, riders = find_race(p)
        if race is None:
            sys.exit("no race found (is a race running?)")
        # the course object gWorld 0x346488: +4 the course header, its +0x48 the instances (0x100 bytes each)
        inst = p.read32(p.read32(WORLD + 4) + 0x48) + 0x100 * obj
        last, kept, counts, t0 = None, 0, None, time.time()
        with open(out, "w", encoding="ascii") as f:
            f.write(f"clock object {obj} at {inst:#x} riders {len(riders)} course {p.read32(COURSE_ID)}\n")
            while time.time() - t0 < seconds:
                tick = p.read32(race + RACE_TICK)
                if tick == last:
                    continue
                # as `record`: the race tick moves at the start of the frame; wait until every rider has ticked
                # (+0x5900 moved, at most 30 ms) so the positions are the frame's end
                until = time.time() + 0.03
                while counts is not None and time.time() < until:
                    now = p.read_blocks([(r + RIDER_TICKS, 4) for r in riders])
                    if all(a != b for a, b in zip(now, counts)):
                        break
                got = p.read_blocks([(inst + 0xe4, 8)] + [(r + RIDER_POS, 12) for r in riders] + [(r + RIDER_TICKS, 4) for r in riders])
                ctrl, flags = struct.unpack("<2I", got[0])
                node = p.read_block(ctrl - 0x30 + 0x20, 16) if ctrl else b"\0" * 16
                counts = got[1 + len(riders):]
                if p.read32(race + RACE_TICK) != tick:
                    continue
                words = " ".join(f"{w:08x}" for w in struct.unpack("<4I", node))
                pos = " ".join(",".join(f"{w:08x}" for w in struct.unpack("<3I", b)) for b in got[1:1 + len(riders)])
                ticks = ",".join(f"{struct.unpack('<f', c)[0]:.0f}" for c in counts)
                f.write(f"{tick} {ctrl:08x} {flags:08x} {words} {pos} {ticks}\n")
                last, kept = tick, kept + 1
        print(f"{kept} ticks of object {obj} in {seconds:g} s to {out}")
    elif cmd == ["counters"]:
        start, end = (int(argv[1], 16), int(argv[2], 16)) if len(argv) == 3 else (0x31D300, 0x40C174)
        if p.status() != "running":
            sys.exit("PCSX2 is not running the game (paused?): counters need the game to move")
        for addr, kind, value, rate in find_counters(p, start, end):
            print(f"{addr:#010x}  {kind:5s}  now {value:<14.6g} rising {rate:.2f}/s")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
