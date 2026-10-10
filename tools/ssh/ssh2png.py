#!/usr/bin/env python3
"""Convert EA 'SHPS' (.SSH) PS2 texture archives (SSX Tricky) to PNG.

usage: ssh2png.py FILE.SSH OUT_DIR

Archive:  'SHPS' u32 total_size, u32 count, char[4] version ('G266'/'G278'),
          then count x {char name[4], u32 offset}.
Block hdr (16 bytes): u8 type, u24 next (relative offset to next block of the
          same entry, 0 = last), u16 w, u16 h, u16 cx, u16 cy, u16 px, u16 py
          (px/py upper 4 bits carry flags; 0 in SSX Tricky files).
Image types: 1 = 4bpp CLUT, 2 = 8bpp CLUT, 3 = 16bpp, 4 = 24bpp, 5 = 32bpp RGBA.
Attachments: 0x21/0x22/0x23/0x24 palette (32/16/24-bit, w = entry count),
          0x6F/0x70 long name, 0x7C hotspots / others ignored.
PS2: alpha is GS-style 0..0x80; when an image's alpha never exceeds 0x80 it is
     scaled x2 (clamped to 255).  Some SSX Tricky images (many particles,
     beam, envr) already use full 0..255 alpha and are left as is.
     CLUTs in these files are already stored linearly (CSM1 shuffle undone
     by EA's tools); pass --csm1 to force the 8-15 <-> 16-23 entry swap.
Pixel data in SSX Tricky files is linear (not GS-swizzled); an optional
PSMT8 unswizzle is applied to 8bpp images when the 0x80 bit of the type
is set (never seen in these files).
"""
import os
import struct
import sys
import zlib


def png_write(path, w, h, rgba):
    try:
        from PIL import Image
        Image.frombytes('RGBA', (w, h), bytes(rgba)).save(path)
        return
    except ImportError:
        pass
    raw = b''.join(b'\0' + bytes(rgba[y * w * 4:(y + 1) * w * 4]) for y in range(h))

    def chunk(t, b):
        return struct.pack('>I', len(b)) + t + b + struct.pack('>I', zlib.crc32(t + b) & 0xffffffff)
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


CSM1 = '--csm1' in sys.argv
if CSM1:
    sys.argv.remove('--csm1')


def a2(a):
    return min(255, a * 2)


def fix_alpha(rgba):
    if max(rgba[3::4], default=0) <= 0x80:
        rgba[3::4] = bytes(min(255, a * 2) for a in rgba[3::4])


def unswizzle8(src, w, h):
    out = bytearray(w * h)
    for y in range(h):
        for x in range(w):
            block = (y & ~0xf) * w + (x & ~0xf) * 2
            swap = (((y + 2) >> 2) & 1) * 4
            posy = (((y & ~3) >> 1) + (y & 1)) & 7
            col = posy * w * 2 + ((x + swap) & 7) * 4
            bnum = ((y >> 1) & 1) + ((x >> 2) & 2)
            out[y * w + x] = src[block + col + bnum]
    return out


def read_palette(d, o):
    t = d[o] & 0x7f
    n = struct.unpack_from('<H', d, o + 4)[0] * struct.unpack_from('<H', d, o + 6)[0]
    p = o + 16
    pal = []
    for i in range(n):
        if t == 0x21:
            r, g, b, a = d[p:p + 4]; p += 4
        elif t == 0x22:
            r, g, b = d[p:p + 3]; a = 0x80; p += 3
        elif t == 0x23:
            v = struct.unpack_from('<H', d, p)[0]; p += 2
            r, g, b = (v & 31) << 3, ((v >> 5) & 31) << 3, ((v >> 10) & 31) << 3
            a = 0x80 if v & 0x8000 else 0
        else:
            return None
        pal.append((r, g, b, a if t != 0x23 else (0x80 if a else 0)))
    if CSM1 and len(pal) == 256:  # CSM1 un-shuffle
        q = list(pal)
        for base in range(0, 256, 32):
            q[base + 8:base + 16], q[base + 16:base + 24] = pal[base + 16:base + 24], pal[base + 8:base + 16]
        pal = q
    return pal


def parse(d):
    if d[:4] != b'SHPS':
        raise ValueError('not an SHPS archive')
    _size, count = struct.unpack_from('<II', d, 4)
    ver = d[12:16].decode('latin1')
    entries = []
    for i in range(count):
        name = d[16 + 8 * i:20 + 8 * i].decode('latin1').rstrip('\0')
        off = struct.unpack_from('<I', d, 20 + 8 * i)[0]
        entries.append((name, off))
    return ver, entries


def decode_entry(d, off):
    t = d[off]
    w, h = struct.unpack_from('<HH', d, off + 4)
    fmt = t & 0x7f
    swz = bool(t & 0x80)
    pix = off + 16
    pal = None
    longname = None
    o = off
    nxt = d[o + 1] | d[o + 2] << 8 | d[o + 3] << 16
    while nxt:
        o += nxt
        bt = d[o] & 0x7f
        if bt in (0x21, 0x22, 0x23, 0x24) and pal is None:
            pal = read_palette(d, o)
        elif bt in (0x6f, 0x70):
            longname = d[o + 4:o + 68].split(b'\0')[0].decode('latin1') or None
        nxt = d[o + 1] | d[o + 2] << 8 | d[o + 3] << 16
    rgba = bytearray(w * h * 4)
    if pal is not None:
        pal = pal + [(0, 0, 0, 0)] * (256 - len(pal))
    if fmt == 2:
        src = d[pix:pix + w * h]
        if swz:
            src = unswizzle8(src, w, h)
        for i, v in enumerate(src):
            rgba[i * 4:i * 4 + 4] = bytes(pal[v])
    elif fmt == 1:
        raw = d[pix:pix + (w * h + 1) // 2]
        idx = []
        for b in raw:
            idx += (b & 15, b >> 4)
        for i in range(w * h):
            rgba[i * 4:i * 4 + 4] = bytes(pal[idx[i]])
    elif fmt == 5:
        for i in range(w * h):
            r, g, b, a = d[pix + i * 4:pix + i * 4 + 4]
            rgba[i * 4:i * 4 + 4] = bytes((r, g, b, a))
    elif fmt == 4:
        for i in range(w * h):
            r, g, b = d[pix + i * 3:pix + i * 3 + 3]
            rgba[i * 4:i * 4 + 4] = bytes((r, g, b, 0x80))
    elif fmt == 3:
        for i in range(w * h):
            v = struct.unpack_from('<H', d, pix + i * 2)[0]
            rgba[i * 4:i * 4 + 4] = bytes(((v & 31) << 3, ((v >> 5) & 31) << 3,
                                           ((v >> 10) & 31) << 3, 0x80 if v & 0x8000 else 0))
    else:
        raise ValueError('unsupported image type 0x%02x' % t)
    fix_alpha(rgba)
    return t, w, h, rgba, longname


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__.split('\n\n')[1])
    d = open(sys.argv[1], 'rb').read()
    out = sys.argv[2]
    os.makedirs(out, exist_ok=True)
    ver, entries = parse(d)
    print('%s: %s, %d images' % (sys.argv[1], ver, len(entries)))
    used = {}
    for name, off in entries:
        t, w, h, rgba, ln = decode_entry(d, off)
        fn = name
        if fn in used:
            used[fn] += 1
            fn = '%s_%d' % (fn, used[fn])
        else:
            used[fn] = 0
        png_write(os.path.join(out, fn + '.png'), w, h, rgba)
        print('  %-6s type=0x%02x %4dx%-4d%s' % (fn, t, w, h, ('  "%s"' % ln) if ln else ''))


if __name__ == '__main__':
    main()
