#!/usr/bin/env python3
"""ssxpatch - apply code mods to SSX Tricky (USA, SLUS-203.26).

Needs only Python 3 (no packages).  Run from the project folder:

  python tools/ssxpatch.py list
  python tools/ssxpatch.py build debug_menu unlock_all      -> build/SSX Tricky (USA) [mod].iso
  python tools/ssxpatch.py pnach debug_menu unlock_all      -> mods/pcsx2/*.pnach
  python tools/ssxpatch.py elf   debug_menu                 -> build/SLUS_203.26 (patched ELF only)
  python tools/ssxpatch.py build unlock_all --replace DATA/MODELS/GARI.BIG=levels/build/GARI.BIG
  python tools/ssxpatch.py verify "build/SSX Tricky (USA) [mod].iso"

The original ISO is never modified; `build` writes a patched copy.
"""
import argparse, hashlib, json, os, shutil, struct, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_ISO = os.path.join(ROOT, "SSX Tricky (USA).iso")
ELF_NAME = b"SLUS_203.26;1"
SECTOR = 2048


def load_mods():
    with open(os.path.join(ROOT, "mods", "mods.json"), encoding="utf-8") as f:
        return json.load(f)


def find_record(f, path):
    """Return (directory-record byte offset, lba, size) for a path like DATA/MODELS/GARI.BIG."""
    f.seek(16 * SECTOR)
    pvd = f.read(SECTOR)
    lba, size = struct.unpack_from("<I", pvd, 158)[0], struct.unpack_from("<I", pvd, 166)[0]
    parts = [x.upper().encode() for x in path.replace("\\", "/").strip("/").split("/")]
    for depth, part in enumerate(parts):
        last = depth == len(parts) - 1
        f.seek(lba * SECTOR)
        d = f.read(size)
        i, hit = 0, None
        while i < len(d):
            n = d[i]
            if n == 0:
                i = (i // SECTOR + 1) * SECTOR
                continue
            e = d[i:i + n]
            name = e[33:33 + e[32]]
            if name == part or (last and name == part + b";1"):
                hit = (lba * SECTOR + i, struct.unpack_from("<I", e, 2)[0], struct.unpack_from("<I", e, 10)[0])
                break
            i += n
        if not hit:
            sys.exit("%s not found in ISO" % path)
        _, lba, size = hit
    return hit


def replace_file(f, path, data):
    """Replace a file's contents. Same-or-smaller data is written in place; larger data is
    appended at the end of the image and the directory record is pointed at it."""
    rec, lba, size = find_record(f, path)
    room = (size + SECTOR - 1) // SECTOR * SECTOR
    if len(data) <= room:
        f.seek(lba * SECTOR)
        f.write(data + b"\0" * (room - len(data)))
        new_lba = lba
    else:
        f.seek(0, 2)
        end = (f.tell() + SECTOR - 1) // SECTOR * SECTOR
        new_lba = end // SECTOR
        f.seek(end)
        f.write(data + b"\0" * (-len(data) % SECTOR))
        total = f.tell() // SECTOR
        f.seek(16 * SECTOR + 80)
        f.write(struct.pack("<I", total) + struct.pack(">I", total))
    f.seek(rec + 2)
    f.write(struct.pack("<I", new_lba) + struct.pack(">I", new_lba) + struct.pack("<I", len(data)) + struct.pack(">I", len(data)))
    return new_lba


def find_in_iso(f, name):
    """Return (byte offset, size) of a root-directory file in an ISO9660 image."""
    f.seek(16 * SECTOR)
    pvd = f.read(SECTOR)
    if pvd[1:6] != b"CD001":
        sys.exit("not an ISO9660 image")
    lba, size = struct.unpack_from("<I", pvd, 158)[0], struct.unpack_from("<I", pvd, 166)[0]
    f.seek(lba * SECTOR)
    d = f.read(size)
    i = 0
    while i < len(d):
        n = d[i]
        if n == 0:
            i = (i // SECTOR + 1) * SECTOR
            continue
        e = d[i:i + n]
        if e[33:33 + e[32]] == name:
            return struct.unpack_from("<I", e, 2)[0] * SECTOR, struct.unpack_from("<I", e, 10)[0]
        i += n
    sys.exit("%s not found in ISO" % name.decode())


class Elf:
    def __init__(self, data):
        self.data = bytearray(data)
        if self.data[:4] != b"\x7fELF":
            sys.exit("not an ELF")
        phoff, = struct.unpack_from("<I", self.data, 28)
        phnum, = struct.unpack_from("<H", self.data, 44)
        self.segs = []
        for i in range(phnum):
            t, off, va, _, fsz, _ = struct.unpack_from("<6I", self.data, phoff + i * 32)
            if t == 1:
                self.segs.append((va, off, fsz))

    def off(self, va):
        for v, o, n in self.segs:
            if v <= va < v + n:
                return o + va - v
        sys.exit("address %08X is not file-backed" % va)

    def word(self, va):
        return struct.unpack_from("<I", self.data, self.off(va))[0]

    def apply(self, mod_name, mod):
        """Returns list of (file offset, new bytes).  Refuses if original words don't match."""
        out = []
        for p in mod["patches"]:
            a, o, n = int(p["addr"], 16), int(p["orig"], 16), int(p["new"], 16)
            cur = self.word(a)
            if cur == n:
                continue
            if cur != o:
                sys.exit("%s: %08X holds %08X, expected %08X - wrong game version or conflicting mod"
                         % (mod_name, a, cur, o))
            struct.pack_into("<I", self.data, self.off(a), n)
            out.append((self.off(a), struct.pack("<I", n)))
        return out

    def pcsx2_crc(self):
        crc = 0
        for (w,) in struct.iter_unpack("<I", bytes(self.data[:len(self.data) // 4 * 4])):
            crc ^= w
        return crc


def pick(cfg, names):
    if not names:
        sys.exit("name at least one mod (see: ssxpatch.py list)")
    for n in names:
        if n not in cfg["mods"]:
            sys.exit("unknown mod '%s' (see: ssxpatch.py list)" % n)
    return [(n, cfg["mods"][n]) for n in names]


def read_elf(iso):
    with open(iso, "rb") as f:
        off, size = find_in_iso(f, ELF_NAME)
        f.seek(off)
        return off, f.read(size)


def cmd_list(a, cfg):
    for n, m in cfg["mods"].items():
        print("%-14s %s" % (n, m["title"]))
        print("               %s\n" % m["description"])


def patched_elf(a, cfg):
    off, raw = read_elf(a.iso)
    md5 = hashlib.md5(raw).hexdigest()
    if md5 != cfg["elf_md5"]:
        sys.exit("SLUS_203.26 in this ISO has md5 %s, expected %s (USA retail)" % (md5, cfg["elf_md5"]))
    elf = Elf(raw)
    writes = []
    for n, m in (pick(cfg, a.mods) if a.mods or not getattr(a, "replace", None) else []):
        writes += elf.apply(n, m)
        print("applied %-14s (%d words)" % (n, len(m["patches"])))
    return off, elf, writes


def cmd_elf(a, cfg):
    _, elf, _ = patched_elf(a, cfg)
    out = a.out or os.path.join(ROOT, "build", "SLUS_203.26")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as f:
        f.write(elf.data)
    print("wrote", out)


def cmd_build(a, cfg):
    off, elf, writes = patched_elf(a, cfg)
    out = a.out or os.path.join(ROOT, "build", "SSX Tricky (USA) [%s].iso" % "+".join(a.mods or ["files"]))
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if os.path.abspath(out) == os.path.abspath(a.iso):
        sys.exit("refusing to overwrite the original ISO")
    total = os.path.getsize(a.iso)
    if not (os.path.exists(out) and os.path.getsize(out) >= total and a.reuse):
        print("copying ISO (%.1f GB)..." % (total / 2**30))
        shutil.copyfile(a.iso, out)
    with open(out, "r+b") as f:
        for o, b in writes:
            f.seek(off + o)
            f.write(b)
        for spec in a.replace or []:
            isopath, local = spec.split("=", 1)
            with open(local, "rb") as g:
                data = g.read()
            lba = replace_file(f, isopath, data)
            print("replaced %s with %s (%d bytes at sector %d)" % (isopath, local, len(data), lba))
    print("wrote", out)
    print("patched-ELF PCSX2 CRC: %08X" % elf.pcsx2_crc())


def cmd_pnach(a, cfg):
    mods = pick(cfg, a.mods)
    outdir = a.out or os.path.join(ROOT, "mods", "pcsx2")
    os.makedirs(outdir, exist_ok=True)
    # new PCSX2 (1.7 / 2.x): serial_CRC.pnach with [sections] you tick in Game Properties > Cheats
    # old PCSX2 (<= 1.6):     CRC.pnach, flat list, everything on when "Enable Cheats" is ticked
    for name, sections in (("SLUS-20326_%s.pnach" % cfg["pcsx2_crc"], True), ("%s.pnach" % cfg["pcsx2_crc"], False)):
        lines = ["gametitle=SSX Tricky (USA) SLUS-20326", ""]
        for n, m in mods:
            if sections:
                lines += ["[%s]" % m["title"], "description=%s." % m["description"].split(". ")[0].rstrip(".")]
            else:
                lines.append("// " + m["title"])
            for p in m["patches"]:
                lines.append("// " + p["asm"])
                lines.append("patch=0,EE,2%07X,extended,%08X" % (int(p["addr"], 16) & 0x1FFFFFF, int(p["new"], 16)))
            lines.append("")
        with open(os.path.join(outdir, name), "w", encoding="utf-8", newline="\r\n") as f:
            f.write("\n".join(lines))
        print("wrote", os.path.join(outdir, name))


def cmd_verify(a, cfg):
    _, raw = read_elf(a.target)
    elf = Elf(raw)
    print("ELF md5 %s   PCSX2 CRC %08X" % (hashlib.md5(raw).hexdigest(), elf.pcsx2_crc()))
    for n, m in cfg["mods"].items():
        st = set()
        for p in m["patches"]:
            w = elf.word(int(p["addr"], 16))
            st.add("on" if w == int(p["new"], 16) else "off" if w == int(p["orig"], 16) else "UNKNOWN")
        print("  %-14s %s" % (n, st.pop() if len(st) == 1 else "PARTIAL/" + ",".join(sorted(st))))


def main():
    cfg = load_mods()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list")
    for c in ("build", "elf", "pnach"):
        s = sub.add_parser(c)
        s.add_argument("mods", nargs="*")
        s.add_argument("--iso", default=DEFAULT_ISO)
        s.add_argument("--out")
        if c == "build":
            s.add_argument("--reuse", action="store_true", help="patch an existing output instead of recopying")
            s.add_argument("--replace", action="append", metavar="ISOPATH=FILE",
                           help="replace a file on the disc, e.g. DATA/MODELS/GARI.BIG=levels/build/GARI.BIG (repeatable)")
    v = sub.add_parser("verify")
    v.add_argument("target")
    a = ap.parse_args()
    {"list": cmd_list, "build": cmd_build, "elf": cmd_elf, "pnach": cmd_pnach, "verify": cmd_verify}[a.cmd](a, cfg)


if __name__ == "__main__":
    main()
