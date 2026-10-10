#!/usr/bin/env python3
"""Which R5900 instructions a set of original functions use, with everything they call (row F4).

    python tools/insn_census.py <SLUS_203.26> [--depth N] 0x109cb8 0x109ef8 ...

--depth 0 looks at the named functions only (listing what they call); the default follows every call.

Reads your own copy of the executable (never commit it), takes each function's extent from
tricky-rs/docs/function-index.csv, follows `jal` calls, and prints the instructions used and how often, the
functions reached, and the indirect calls (`jalr`) the runner would have to stub. Only mnemonics and counts
are printed, nothing of the game's code. This is what the function runner (row F4a) has to implement.
"""
import collections
import csv
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "tricky-rs" / "docs" / "function-index.csv"

PRIMARY = {2: "j", 3: "jal", 4: "beq", 5: "bne", 6: "blez", 7: "bgtz", 8: "addi", 9: "addiu", 10: "slti",
           11: "sltiu", 12: "andi", 13: "ori", 14: "xori", 15: "lui", 20: "beql", 21: "bnel", 22: "blezl",
           23: "bgtzl", 24: "daddi", 25: "daddiu", 26: "ldl", 27: "ldr", 30: "lq", 31: "sq", 32: "lb", 33: "lh",
           34: "lwl", 35: "lw", 36: "lbu", 37: "lhu", 38: "lwr", 39: "lwu", 40: "sb", 41: "sh", 42: "swl",
           43: "sw", 44: "sdl", 45: "sdr", 46: "swr", 47: "cache", 49: "lwc1", 51: "pref", 54: "lqc2", 55: "ld",
           57: "swc1", 62: "sqc2", 63: "sd"}
SPECIAL = {0: "sll", 2: "srl", 3: "sra", 4: "sllv", 6: "srlv", 7: "srav", 8: "jr", 9: "jalr", 10: "movz",
           11: "movn", 12: "syscall", 13: "break", 15: "sync", 16: "mfhi", 17: "mthi", 18: "mflo", 19: "mtlo",
           20: "dsllv", 22: "dsrlv", 23: "dsrav", 24: "mult", 25: "multu", 26: "div", 27: "divu", 32: "add",
           33: "addu", 34: "sub", 35: "subu", 36: "and", 37: "or", 38: "xor", 39: "nor", 40: "mfsa", 41: "mtsa",
           42: "slt", 43: "sltu", 44: "dadd", 45: "daddu", 46: "dsub", 47: "dsubu", 52: "teq", 56: "dsll",
           58: "dsrl", 59: "dsra", 60: "dsll32", 62: "dsrl32", 63: "dsra32"}
REGIMM = {0: "bltz", 1: "bgez", 2: "bltzl", 3: "bgezl", 16: "bltzal", 17: "bgezal", 24: "mtsab", 25: "mtsah"}
COP1_S = {0: "add.s", 1: "sub.s", 2: "mul.s", 3: "div.s", 4: "sqrt.s", 5: "abs.s", 6: "mov.s", 7: "neg.s",
          22: "rsqrt.s", 24: "adda.s", 25: "suba.s", 26: "mula.s", 28: "madd.s", 29: "msub.s", 30: "madda.s",
          31: "msuba.s", 36: "cvt.w.s", 40: "max.s", 41: "min.s", 48: "c.f.s", 50: "c.eq.s", 52: "c.lt.s",
          54: "c.le.s"}
MMI = {0: "madd", 1: "maddu", 4: "plzcw", 16: "mfhi1", 17: "mthi1", 18: "mflo1", 19: "mtlo1", 24: "mult1",
       25: "multu1", 26: "div1", 27: "divu1", 32: "madd1", 33: "maddu1", 48: "pmfhl", 49: "pmthl", 52: "psllh",
       54: "psrlh", 55: "psrah", 60: "psllw", 62: "psrlw", 63: "psraw"}
# VU0 macro mode (COP2 with bit 25 set). funct 60-63 select the second table by (bits 10..6) << 2 | funct & 3.
_XYZW = "xyzw"
VU0 = {**{i: f"v{op}{_XYZW[i & 3]}" for op, base in (("add", 0), ("sub", 4), ("madd", 8), ("msub", 12),
                                                    ("max", 16), ("mini", 20), ("mul", 24)) for i in range(base, base + 4)},
       28: "vmulq", 29: "vmaxi", 30: "vmuli", 31: "vminii", 32: "vaddq", 33: "vmaddq", 34: "vaddi", 35: "vmaddi",
       36: "vsubq", 37: "vmsubq", 38: "vsubi", 39: "vmsubi", 40: "vadd", 41: "vmadd", 42: "vmul", 43: "vmax",
       44: "vsub", 45: "vmsub", 46: "vopmsub", 47: "vmini", 48: "viadd", 49: "visub", 50: "viaddi", 52: "viand",
       53: "vior", 56: "vcallms", 57: "vcallmsr"}
VU0_2 = {**{i: f"v{op}{_XYZW[i & 3]}" for op, base in (("adda", 0), ("suba", 4), ("madda", 8), ("msuba", 12),
                                                      ("mula", 24)) for i in range(base, base + 4)},
         16: "vitof0", 17: "vitof4", 18: "vitof12", 19: "vitof15", 20: "vftoi0", 21: "vftoi4", 22: "vftoi12",
         23: "vftoi15", 28: "vmulaq", 29: "vabs", 30: "vmulai", 31: "vclipw", 32: "vaddaq", 33: "vmaddaq",
         34: "vaddai", 35: "vmaddai", 36: "vsubaq", 37: "vmsubaq", 38: "vsubai", 39: "vmsubai", 40: "vadda",
         41: "vmadda", 42: "vmula", 44: "vsuba", 45: "vmsuba", 46: "vopmula", 47: "vnop", 48: "vmove",
         49: "vmr32", 52: "vlqi", 53: "vsqi", 54: "vlqd", 55: "vsqd", 56: "vdiv", 57: "vsqrt", 58: "vrsqrt",
         59: "vwaitq", 60: "vmtir", 61: "vmfir", 62: "vilwr", 63: "viswr", 64: "vrnext", 65: "vrget",
         66: "vrinit", 67: "vrxor"}

MMI0 = {0: "paddw", 1: "psubw", 2: "pcgtw", 3: "pmaxw", 4: "paddh", 5: "psubh", 6: "pcgth", 7: "pmaxh",
        8: "paddb", 9: "psubb", 10: "pcgtb", 16: "paddsw", 17: "psubsw", 18: "pextlw", 19: "ppacw", 20: "paddsh",
        21: "psubsh", 22: "pextlh", 23: "ppach", 24: "paddsb", 25: "psubsb", 26: "pextlb", 27: "ppacb",
        30: "pext5", 31: "ppac5"}
MMI1 = {1: "pabsw", 2: "pceqw", 3: "pminw", 4: "padsbh", 5: "pabsh", 6: "pceqh", 7: "pminh", 10: "pceqb",
        16: "padduw", 17: "psubuw", 18: "pextuw", 20: "padduh", 21: "psubuh", 22: "pextuh", 24: "paddub",
        25: "psubub", 26: "pextub", 27: "qfsrv"}
MMI2 = {0: "pmaddw", 2: "psllvw", 3: "psrlvw", 4: "pmsubw", 8: "pmfhi", 9: "pmflo", 10: "pinth", 12: "pmultw",
        13: "pdivw", 14: "pcpyld", 16: "pmaddh", 17: "phmadh", 18: "pand", 19: "pxor", 20: "pmsubh",
        21: "phmsbh", 26: "pexeh", 27: "prevh", 28: "pmulth", 29: "pdivbw", 30: "pexew", 31: "prot3w"}
MMI3 = {0: "pmadduw", 3: "psravw", 8: "pmthi", 9: "pmtlo", 10: "pinteh", 12: "pmultuw", 13: "pdivuw",
        14: "pcpyud", 18: "por", 19: "pnor", 26: "pexch", 27: "pcpyh", 30: "pexcw"}


def decode(w):
    op, rs, rt, fn, sa = w >> 26, (w >> 21) & 31, (w >> 16) & 31, w & 63, (w >> 6) & 31
    if w == 0:
        return "nop"
    if op == 0:
        return SPECIAL.get(fn, f"special.{fn}")
    if op == 1:
        return REGIMM.get(rt, f"regimm.{rt}")
    if op == 16:
        if rs == 16 and fn == 0x38:
            return "ei"
        if rs == 16 and fn == 0x39:
            return "di"
        return {0: "mfc0", 4: "mtc0"}.get(rs, f"cop0.{rs}.{fn}")
    if op == 17:
        if rs == 16:
            return COP1_S.get(fn, f"cop1.s.{fn}")
        if rs == 20:
            return "cvt.s.w" if fn == 32 else f"cop1.w.{fn}"
        if rs == 8:
            return ("bc1f", "bc1t", "bc1fl", "bc1tl")[rt & 3]
        return {0: "mfc1", 2: "cfc1", 4: "mtc1", 6: "ctc1"}.get(rs, f"cop1.{rs}")
    if op == 18:
        if rs & 16:
            if fn >= 60:
                return VU0_2.get((sa << 2) | (fn & 3), f"vu0.{sa}.{fn}")
            return VU0.get(fn, f"vu0.{fn}")
        return {1: "qmfc2", 2: "cfc2", 5: "qmtc2", 6: "ctc2", 8: "bc2"}.get(rs, f"cop2.{rs}")
    if op == 28:
        sub = {8: MMI0, 40: MMI1, 9: MMI2, 41: MMI3}.get(fn)
        if sub is not None:
            return sub.get(sa, f"mmi.{fn}.{sa}")
        return MMI.get(fn, f"mmi.{fn}")
    return PRIMARY.get(op, f"op.{op}")


def main(elf_path, roots, depth):
    data = Path(elf_path).read_bytes()
    phoff, = struct.unpack_from("<I", data, 28)
    phnum, = struct.unpack_from("<H", data, 44)
    segs = []
    for i in range(phnum):
        t, off, va, _, fsz, _ = struct.unpack_from("<6I", data, phoff + i * 32)
        if t == 1:
            segs.append((va, off, fsz))

    def word(a):
        for va, off, n in segs:
            if va <= a < va + n:
                return struct.unpack_from("<I", data, off + a - va)[0]
        raise ValueError(hex(a))

    with open(INDEX, newline="", encoding="utf-8") as f:
        fns = {int(r["address"], 16): (int(r["size"]), r["name"], r["status"]) for r in csv.DictReader(f)}

    todo, seen = [(r, 0) for r in roots], set()
    count = collections.Counter()
    where = collections.defaultdict(set)
    indirect = collections.Counter()
    calls = collections.defaultdict(set)
    while todo:
        a, d = todo.pop()
        if a in seen or a not in fns:
            continue
        seen.add(a)
        size, name, _ = fns[a]
        for pc in range(a, a + size, 4):
            w = word(pc)
            m = decode(w)
            count[m] += 1
            where[m].add(name)
            if m == "jal":
                target = ((pc + 4) & 0xF0000000) | ((w & 0x03FFFFFF) << 2)
                calls[name].add(target)
                if d < depth:
                    todo.append((target, d + 1))
            elif m == "jalr":
                indirect[name] += 1

    print(f"{len(seen)} functions reached from {len(roots)}:")
    for a in sorted(seen):
        size, name, status = fns[a]
        out = ", ".join(fns[c][1] if c in fns else hex(c) for c in sorted(calls[name]))
        print(f"  0x{a:06x} {size:5d} {name} ({status})" + (f" calls {out}" if out else ""))
    print(f"\n{sum(count.values())} instructions, {len(count)} kinds:")
    for m, n in count.most_common():
        print(f"  {m:12s} {n:5d}  {', '.join(sorted(where[m])[:3])}{' ...' if len(where[m]) > 3 else ''}")
    print("\nindirect calls (jalr) to stub:", dict(indirect) or "none")


if __name__ == "__main__":
    args = sys.argv[1:]
    depth = 99
    if "--depth" in args:
        i = args.index("--depth")
        depth = int(args[i + 1])
        del args[i:i + 2]
    if len(args) < 2:
        sys.exit(__doc__)
    main(args[0], [int(x, 16) for x in args[1:]], depth)
