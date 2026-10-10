#!/usr/bin/env python3
"""Capture calls of original functions in a running PCSX2, through PCSX2-MCP's DebugServer (TCP 21512).

Not recommended any more (AGENTS.md §14): it needs PCSX2-MCP's patched PCSX2, whose debug server wedged with many
breakpoints. Use PINE (`tools/pine.py`) for new work; this stays because it made the F4f and F4g captures.

    python tools/pcsx2_debug.py capture <function> <count> <out.txt>
    python tools/pcsx2_debug.py probe <function> <size> <every> <out.txt>

Sets a breakpoint on the function, and on each hit records the argument registers, the memory blocks the
function reads (CAPTURES below) and, after running to the return, the result in $f0 / $v0; then lets the game run
on. The file is made from the game, so keep it on your own PC (never commit it); `tools/r5900/tests/captured.rs`
replays it through the function runner (TRICKY_CAPTURES=<file>). Board row F4f, `tricky-rs/docs/checking.md`.

File format, one block per call:
    call <addr>
    gpr <n> <hex>        $a0-$a3 (low 64 bits)
    fpr <n> <hex>        $f12-$f19
    mem <addr> <hex>     memory as it was at the call
    ret f0 <hex>         the result
    step <pc> <f0> ... <f31>   (probe only) the float registers just before the instruction at pc runs
    end

`probe` stops each call at the entry and at one more point inside the function (every <every>-th instruction
that is not in a delay slot, a different one per call), so the game is stopped three times per call at most; the
replay test then shows the first point where the runner and the game part. Do not put a breakpoint on every
instruction at once: on 2026-10-10 that wedged PCSX2-MCP's DebugServer twice (the emulator had to be restarted;
breakpoints also survive a game reboot, only a restart of PCSX2 clears them).
"""
import json
import socket
import sys
import time

PORT = 21512

# What each function reads, as (register, extra offsets to follow as pointers, length). Read from the
# decompiler; tricky-rs/docs/checking.md has the field list for Boarder_ForwardDrag.
BOARDER = [("a0", [], 0x10), ("a0", [0xC], 0x480), ("a0", [0xC, 0x464], 0x50)]
CAPTURES = {
    0x109CB8: {"name": "Boarder_ForwardDrag", "reads": BOARDER + [("a1", [], 0x20)]},
    0x109EF8: {"name": "Boarder_SideFriction", "reads": BOARDER + [("a1", [], 0x20)]},
}
GPR_NAMES = {"a0": 4, "a1": 5, "a2": 6, "a3": 7}


class Debug:
    def __init__(self, port=PORT):
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=30)
        self.f = self.sock.makefile("rw")

    def cmd(self, cmd, **params):
        self.f.write(json.dumps({"cmd": cmd, **params}) + "\n")
        self.f.flush()
        reply = json.loads(self.f.readline())
        if not reply.get("ok", False):
            raise RuntimeError(f"{cmd}: {reply}")
        return reply

    def regs(self, category):
        # the reply holds the category (GPR, FPR, ...) next to pc/hi/lo; values are hex, most significant first
        data = self.cmd("read_registers", category=category)["data"]
        cat = next(v for v in data.values() if isinstance(v, dict) and "regs" in v)
        return [int(r["value"], 16) for r in cat["regs"]]

    def read(self, addr, length):
        return bytes.fromhex(self.cmd("read_memory", address=addr, length=length)["hex"])

    def word(self, addr):
        return int.from_bytes(self.read(addr, 4), "little")

    def wait_paused(self, timeout=600):
        """Wait for a debugger stop. A stop in the kernel's idle loop (below 0x100000) is PCSX2 itself paused
        (its pause button, or "Pause On Focus Loss"): resuming then hangs, so keep waiting for the user."""
        t0 = time.time()
        told = False
        while time.time() - t0 < timeout:
            d = self.cmd("status")["data"]
            pc = int(d["pc"], 16)
            if d["paused"] and pc >= 0x100000:
                return pc
            if d["paused"] and not told:
                print("PCSX2 is paused: unpause it (and turn off Settings > Interface > Pause On Focus Loss)")
                told = True
            time.sleep(0.05)
        raise TimeoutError("the breakpoint was not hit (is a race running, and PCSX2 unpaused?)")


def low32(v):
    return v & 0xFFFFFFFF


def record_call(d, fn, spec, gpr, fpr):
    lines = [f"call {fn:06x}"]
    lines += [f"gpr {n} {gpr[n] & (2**64 - 1):016x}" for n in range(4, 8)]
    lines += [f"fpr {n} {low32(fpr[n]):08x}" for n in range(12, 20)]
    for reg, chain, length in spec["reads"]:
        addr = low32(gpr[GPR_NAMES[reg]])
        for off in chain:
            addr = d.word(addr + off)
        lines.append(f"mem {addr:08x} {d.read(addr, length).hex()}")
    return lines


BRANCHES = {"j", "jal", "jr", "jalr", "beq", "bne", "blez", "bgtz", "beql", "bnel", "blezl", "bgtzl", "bltz",
            "bgez", "bltzl", "bgezl", "bc1f", "bc1t", "bc1fl", "bc1tl"}


def probe_points(d, fn, size, every):
    """Instructions a breakpoint can stop at: not delay slots."""
    sys.path.insert(0, str(__import__("pathlib").Path(__file__).parent))
    from insn_census import decode
    code = d.read(fn, size)
    words = [int.from_bytes(code[i:i + 4], "little") for i in range(0, size, 4)]
    slots = {fn + 4 * (i + 1) for i, w in enumerate(words) if decode(w) in BRANCHES}
    points = [fn + 4 * i for i in range(1, len(words)) if fn + 4 * i not in slots]
    return points[::every]


def probe(fn, size, every, out):
    d = Debug()
    spec = CAPTURES[fn]
    points = probe_points(d, fn, size, every)
    print(f"{len(points)} probe points")
    d.cmd("set_breakpoint", address=fn, description="probe entry")
    with open(out, "a", encoding="ascii") as f:
        for n, x in enumerate(points):
            if d.wait_paused() != fn:
                d.cmd("resume")
                continue
            gpr, fpr = d.regs(0), d.regs(2)
            lines = record_call(d, fn, spec, gpr, fpr)
            lines.append(f"step {fn:06x} " + " ".join(f"{low32(v):08x}" for v in fpr))
            ra = low32(gpr[31])
            d.cmd("set_breakpoint", address=x, description="probe")
            d.cmd("set_breakpoint", address=ra, description="probe return")
            d.cmd("resume")
            pc = d.wait_paused(60)
            if pc == x:
                lines.append(f"step {x:06x} " + " ".join(f"{low32(v):08x}" for v in d.regs(2)))
                d.cmd("remove_breakpoint", address=x)
                d.cmd("resume")
                pc = d.wait_paused(60)
            else:
                d.cmd("remove_breakpoint", address=x)  # not on this call's path
            if pc != ra:
                raise RuntimeError(f"stopped at {pc:#x}, expected the return {ra:#x}")
            fpr = d.regs(2)
            d.cmd("remove_breakpoint", address=ra)
            lines += [f"ret f0 {low32(fpr[0]):08x}", "end"]
            f.write("\n".join(lines) + "\n")
            f.flush()
            print(f"{n + 1}/{len(points)}: probe {x:#x} {'hit' if any(l.startswith(f'step {x:06x}') for l in lines) else 'not on path'}")
            d.cmd("resume")
    d.cmd("remove_breakpoint", address=fn)
    d.cmd("resume")


def capture(fn, count, out):
    d = Debug()
    spec = CAPTURES[fn]
    d.cmd("set_breakpoint", address=fn, description=f"{spec['name']} capture")
    seen = 0
    with open(out, "a", encoding="ascii") as f:
        while seen < count:
            pc = d.wait_paused()
            if pc != fn:
                d.cmd("resume")
                continue
            gpr, fpr = d.regs(0), d.regs(2)
            if low32(gpr[28]) != 0x3C38F0:
                raise RuntimeError(f"$gp is {gpr[28]:#x}, not 0x3c38f0: is this SLUS_203.26?")
            lines = [f"call {fn:06x}"]
            lines += [f"gpr {n} {gpr[n] & (2**64 - 1):016x}" for n in range(4, 8)]
            lines += [f"fpr {n} {low32(fpr[n]):08x}" for n in range(12, 20)]
            for reg, chain, length in spec["reads"]:
                addr = low32(gpr[GPR_NAMES[reg]])
                for off in chain:
                    addr = d.word(addr + off)
                lines.append(f"mem {addr:08x} {d.read(addr, length).hex()}")
            ra = low32(gpr[31])
            d.cmd("set_breakpoint", address=ra, temporary=True, description="capture return")
            d.cmd("resume")
            if d.wait_paused() != ra:
                raise RuntimeError("stopped somewhere else before the return")
            gpr, fpr = d.regs(0), d.regs(2)
            d.cmd("remove_breakpoint", address=ra)
            lines += [f"ret f0 {low32(fpr[0]):08x}", f"ret v0 {gpr[2] & (2**64 - 1):016x}", "end"]
            f.write("\n".join(lines) + "\n")
            seen += 1
            print(f"{seen}/{count}: {spec['name']} a0={low32(int(lines[1].split()[2], 16)):#x} "
                  f"f12={lines[5].split()[2]} -> f0={low32(fpr[0]):08x}")
            d.cmd("resume")
    d.cmd("remove_breakpoint", address=fn)
    d.cmd("resume")


if __name__ == "__main__":
    if len(sys.argv) == 5 and sys.argv[1] == "capture":
        capture(int(sys.argv[2], 16), int(sys.argv[3]), sys.argv[4])
    elif len(sys.argv) == 6 and sys.argv[1] == "probe":
        probe(int(sys.argv[2], 16), int(sys.argv[3], 16), int(sys.argv[4]), sys.argv[5])
    else:
        sys.exit(__doc__)
