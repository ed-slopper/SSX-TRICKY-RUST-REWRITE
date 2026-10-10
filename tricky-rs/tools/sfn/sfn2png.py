#!/usr/bin/env python3
"""Convert SSX Tricky (PS2) 'FNTS' .SFN bitmap fonts to an RGBA PNG atlas + JSON metrics.

usage: sfn2png.py FONT.SFN OUT_PREFIX      -> OUT_PREFIX.png, OUT_PREFIX.json

File layout (little endian), reversed from Font_LoadSfn 0x19c858 / FUN_0019c910 / FUN_0019cca8:
  0x00 char[4] 'FNTS'
  0x04 u32     size-ish (not read by the game)
  0x08 u16     version: >=200 -> 12-byte glyph records, <200 -> 11-byte records
  0x0a u16     glyph count (records sorted by code)
  0x0c u16     9   (not read)          0x0e u16  1 if a kerning table follows, else 0 (not read)
  0x10 s8      origin_x  (font+0x10; added to the pen start, * scale_x)
  0x11 s8      origin_y  (font+0x14; added to the pen start, * scale_y)
  0x12 u8      0         0x13 u8 nominal size (27/16/8; not read)
  0x14 u32     glyph table offset (0x20)
  0x18 u32     kerning table offset, 0 = none (NOT read by the game: no kerning at runtime)
  0x1c u32     bitmap block offset
Glyph record: u16 code, u8 w, u8 h, u16 x, u16 y, s8 advance, s8 xoff, s8 yoff [, u8 pad if v>=200]
Bitmap block: u8 type(1 = 4bpp), u8[3], u16 w, u16 h, u8[8]; then w*h/2 bytes, low nibble = left pixel.
  The game uploads it into a square texture of side max(w, smallest pow2 > h) with a generated
  16-entry CLUT: RGB 0x80, A = i*128/15 (GS alpha, 0x80 = opaque).  PNG: white, alpha doubled.
Kerning block: u32 n, n x {u16 left, u16 right, s8 adjust, u8 pad[3]}.

Drawing (cPS2GraphicsMan vf73 0x1de0e0 via Font_DrawString 0x19d0c0), sx/sy = scale * font_scale:
  pen_x = x + origin_x*sx ; pen_y = y + origin_y*sy
  for each char: g = lookup(code); if missing -> skipped, NO advance
     quad = (pen_x + xoff*sx, pen_y + yoff*sy, w*sx, h*sy), uv = (x, y, x+w, y+h) texels
     pen_x += advance*sx            (no extra spacing, no kerning, space is a normal glyph)
"""
import json
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


# Per-font scale the game applies after loading (font+0x18 sx, font+0x1c sy).
GAME_SCALE = {
    'TITLE': (1.4, 1.3),   # gApp+0x5c: HUD / in-race text
    'MENU': (1.8, 1.4),    # gApp+0x60: menus, trick names, start screen
}


def parse(d):
    if d[:4] != b'FNTS':
        raise ValueError('not an FNTS (.SFN) font')
    ver, count = struct.unpack_from('<HH', d, 8)
    origin_x, origin_y = struct.unpack_from('<bb', d, 0x10)
    size_hint = d[0x13]
    goff, koff, boff = struct.unpack_from('<III', d, 0x14)
    rec = 12 if ver >= 200 else 11
    glyphs = []
    for i in range(count):
        o = goff + i * rec
        code, w, h, x, y = struct.unpack_from('<HBBHH', d, o)
        adv, xoff, yoff = struct.unpack_from('<bbb', d, o + 8)
        glyphs.append(dict(code=code, x=x, y=y, w=w, h=h, xoff=xoff, yoff=yoff, advance=adv))
    btype = d[boff]
    bw, bh = struct.unpack_from('<HH', d, boff + 4)
    if btype != 1:
        raise ValueError('unsupported bitmap type %d' % btype)
    pix = d[boff + 16:boff + 16 + bw * bh // 2]
    kern = []
    if koff:
        n = struct.unpack_from('<I', d, koff)[0]
        for i in range(n):
            l, r, a = struct.unpack_from('<HHb', d, koff + 4 + 8 * i)
            kern.append((l, r, a))
    return dict(version=ver, origin=(origin_x, origin_y), size_hint=size_hint, glyphs=glyphs,
                bw=bw, bh=bh, pix=pix, kern=kern)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src, out = sys.argv[1], sys.argv[2]
    d = open(src, 'rb').read()
    f = parse(d)
    bw, bh = f['bw'], f['bh']
    # CLUT exactly as FUN_0019cca8 builds it, then PS2 alpha (0..0x80) doubled -> 0..255.
    alpha = [min(255, ((i << 7) // 15) * 2) for i in range(16)]
    rgba = bytearray(bw * bh * 4)
    for i in range(bw * bh):
        b = f['pix'][i >> 1]
        v = (b >> 4) if i & 1 else (b & 15)
        rgba[i * 4:i * 4 + 4] = bytes((255, 255, 255, alpha[v]))
    png_write(out + '.png', bw, bh, rgba)

    pot = 1
    while pot <= bh:
        pot <<= 1
    tex = max(bw, pot)
    gl = f['glyphs']
    # font+0xc: max(h + yoff) computed at load; HUD uses it as the line height for anchoring.
    line_height = max((g['h'] + g['yoff'] for g in gl), default=0)
    name = src.replace('\\', '/').rsplit('/', 1)[-1].rsplit('.', 1)[0].upper()
    j = {
        'source': src.replace('\\', '/').rsplit('/', 1)[-1],
        'version': f['version'],
        'atlas': {'w': bw, 'h': bh, 'tex_size': tex},
        'line_height': line_height,
        'size_hint': f['size_hint'],
        'origin': {'x': f['origin'][0], 'y': f['origin'][1]},
        'game_scale': dict(zip(('x', 'y'), GAME_SCALE.get(name, (1.0, 1.0)))),
        'missing_glyph': 'skipped, no advance',
        'glyphs': {str(g['code']): {k: g[k] for k in ('x', 'y', 'w', 'h', 'xoff', 'yoff', 'advance')}
                   for g in gl},
        'kerning': [{'left': l, 'right': r, 'adjust': a} for l, r, a in f['kern']],
        'kerning_used_by_game': False,
    }
    with open(out + '.json', 'w') as fp:
        json.dump(j, fp, indent=1)
    print('%s: v%d %d glyphs, atlas %dx%d (tex %d), line_height %d, %d kern pairs'
          % (j['source'], f['version'], len(gl), bw, bh, tex, line_height, len(f['kern'])))


if __name__ == '__main__':
    main()
