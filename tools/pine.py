#!/usr/bin/env python3
"""Talk to a running PCSX2 over PINE, its built-in IPC (Settings > Advanced > PINE, slot 28011).

    python tools/pine.py status                 emulator status, game id, title, version
    python tools/pine.py read <addr> [count]    read 32-bit words from EE memory
    python tools/pine.py floats <addr> [count]  the same words as floats

Reads only what you ask for and prints numbers; nothing of the game is written anywhere. Used for board rows
F4b and F4f when the PCSX2-MCP tools are not loaded (tricky-rs/docs/checking.md).
"""
import socket
import struct
import sys

PORT = 28011
READ8, READ16, READ32, READ64 = 0, 1, 2, 3
VERSION, TITLE, ID, UUID, GAME_VERSION, STATUS = 8, 0xB, 0xC, 0xD, 0xE, 0xF
STATES = {0: "running", 1: "paused", 2: "shutdown"}


class Pine:
    def __init__(self, port=PORT):
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=3)

    def call(self, op, args=b""):
        msg = struct.pack("<IB", 5 + len(args), op) + args
        self.sock.sendall(msg)
        head = self._recv(4)
        size, = struct.unpack("<I", head)
        body = self._recv(size - 4)
        if body[0] != 0:
            raise RuntimeError(f"PINE command {op:#x} failed")
        return body[1:]

    def _recv(self, n):
        out = b""
        while len(out) < n:
            chunk = self.sock.recv(n - len(out))
            if not chunk:
                raise ConnectionError("PCSX2 closed the connection")
            out += chunk
        return out

    def string(self, op):
        data = self.call(op)
        n, = struct.unpack_from("<I", data)
        return data[4:4 + n].rstrip(b"\0").decode("utf-8", "replace")

    def status(self):
        return STATES.get(struct.unpack("<I", self.call(STATUS))[0], "?")

    def read32(self, addr):
        return struct.unpack("<I", self.call(READ32, struct.pack("<I", addr)))[0]


def main(argv):
    p = Pine()
    if argv[:1] == ["status"]:
        # PCSX2 refuses PINE commands while no game is running
        for label, get in (("PCSX2", lambda: p.string(VERSION)), ("state", p.status), ("id", lambda: p.string(ID)),
                           ("title", lambda: p.string(TITLE)), ("version", lambda: p.string(GAME_VERSION))):
            try:
                print(f"{label}:", get())
            except RuntimeError:
                print(f"{label}: (refused: is a game running?)")
    elif argv[:1] in (["read"], ["floats"]) and len(argv) >= 2:
        addr, count = int(argv[1], 16), int(argv[2]) if len(argv) > 2 else 1
        for k in range(count):
            w = p.read32(addr + 4 * k)
            shown = f"{struct.unpack('<f', struct.pack('<I', w))[0]:.6g}" if argv[0] == "floats" else f"{w:#010x}"
            print(f"{addr + 4 * k:#010x}  {shown}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
