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
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
