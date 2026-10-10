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
    python tools/pine.py riders                          the race object and its riders (addresses, place, speed)
    python tools/pine.py record <seconds> <out.txt> [<bytes>]
                                                        each tick: the race tick and every rider's first
                                                        <bytes> (default 0x600), kept only when the tick did
                                                        not change while reading (F4b; keep it on your PC)
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
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=5)

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

    def save_state(self, slot):
        self.call(SAVE_STATE, struct.pack("<B", slot))

    def load_state(self, slot):
        self.call(LOAD_STATE, struct.pack("<B", slot))


# Found 2026-10-10 over PINE (tricky-rs/docs/checking.md, F4b): stats entries are fixed in .data, one per rider
# slot; a rider struct points at its entry from +0x464. The race object holds the riders at +0xC4, their count at
# +0x88 and the race tick at +0x18 (60 a second).
STATS_TABLE, STATS_STRIDE = 0x32DB70, 0x84
RACE_TICK, RACE_COUNT, RACE_RIDERS = 0x18, 0x88, 0xC4
RIDER_POS, RIDER_VEL, RIDER_TIMESCALE, RIDER_PLACE, RIDER_STATS = 0x140, 0x150, 0x12C, 0x110, 0x464


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
    elif cmd == ["record"] and len(argv) in (3, 4):
        seconds, out = float(argv[1]), argv[2]
        size = int(argv[3], 0) if len(argv) == 4 else 0x600
        race, riders = find_race(p)
        if race is None:
            sys.exit("no race found (is a race running?)")
        kept, last, t0 = 0, None, time.time()
        with open(out, "w", encoding="ascii") as f:
            f.write(f"race {race:x} riders {' '.join(f'{r:x}' for r in riders)} bytes {size:x}\n")
            while time.time() - t0 < seconds:
                tick = p.read32(race + RACE_TICK)
                if tick == last:
                    continue
                blocks = [p.read_block(r, size) for r in riders]
                if p.read32(race + RACE_TICK) != tick:
                    continue  # the game moved on while we read: drop the sample
                f.write(f"tick {tick} " + " ".join(b.hex() for b in blocks) + "\n")
                last, kept = tick, kept + 1
        print(f"{kept} ticks of {len(riders)} riders in {seconds:g} s to {out}")
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
