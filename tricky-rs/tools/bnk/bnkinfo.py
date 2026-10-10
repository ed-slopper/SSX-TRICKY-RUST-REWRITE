#!/usr/bin/env python3
"""SSX Tricky (PS2) EA BNKl bank parser.

Layout:
  0x00 'BNKl'  u16 version(5)  u16 nprog  u32 hdrsize(=first sample data)  u32 ?  u32 total data size
  0x14 u32 slot[nprog]   offset RELATIVE TO THE SLOT'S OWN ADDRESS (20+4*i); 0 = empty
Each program: 'PT' u16 version(5), then tag stream:
  tag(u8) len(u8) value(big-endian, len bytes)
  0xFC = 1-byte padding (no len), 0xFD = start of sample sub-header,
  0xFE = next layer/variant (program-level tags may follow again), 0xFF = end.
Sample tags: 80 ver, 82 channels, 83 codec, 84 rate, 85 nsamples, 86 loop start,
  87 loop end (inclusive sample idx), 88 data offset (absolute file offset), 8C flags, A0 codec2 (5 VAG default, 9 = s8 PCM w/ 16-byte
  inline header {u32 0x900, i32 loopStart, i32 loopEnd, u32 nsamp}).
Pointer tags (12,17,19,1d,20): table addr = file offset of the tag's value bytes + value.
"""
import struct, sys

# Meanings from the game's parser SND_ParsePTLayer (FUN_002d5410) / SND_StartLayerVoice (FUN_002d59d8).
# Defaults (tag absent) in brackets.
NAMES = {
    0x01: 'velLo[0]', 0x02: 'velHi[127]', 0x03: 'keyLo[0]', 0x04: 'keyHi[127]',
    0x06: 'prio?[0]', 0x07: 'rootNote[60]', 0x08: '08[-1]', 0x09: '09[1]',
    0x0a: 'bendRangeSemis[0]', 0x0b: '0b', 0x0c: 'pan[64]', 0x0d: 'panRand[0]',
    0x0e: 'vol[127]', 0x0f: 'volRand[0]', 0x10: 'detuneCents[0]', 0x11: 'pitchRandCents[0]',
    0x12: 'volCurvePtr', 0x13: '13', 0x17: 'bendCurvePtr', 0x19: 'envPtr', 0x1c: '1c[127]',
    0x1d: 'tremTablePtr', 0x1e: 'tremLen', 0x1f: 'tremRandPhase',
    0x20: 'vibTablePtr', 0x21: 'vibLen', 0x22: 'vibDepthCents', 0x23: 'vibRandPhase',
    0x24: '24', 0x25: '25[1]',
    0x80: 'ver', 0x82: 'chans[1]', 0x83: 'codec', 0x84: 'rate[22050]', 0x85: 'nsamp',
    0x86: 'loopStart[-1]', 0x87: 'loopEnd[-1]', 0x88: 'dataofs', 0x89: '89', 0x8a: '8a',
    0x8c: 'flags[8;0x100=swmix]', 0xa0: 'codec2[5=VAG;9=s8pcm]', 0xa1: 'a1[1]',
}


def parse_pt(d, o):
    assert d[o:o + 2] == b'PT', hex(o)
    o += 4
    layers = [[{}, None]]
    cur = layers[-1][0]
    while o < len(d):
        t = d[o]; o += 1
        if t == 0xFC:
            continue
        if t == 0xFF:
            break
        if t == 0xFE:
            layers.append([{}, None]); cur = layers[-1][0]; continue
        if t == 0xFD:
            layers[-1][1] = {}; cur = layers[-1][1]; continue
        n = d[o]; o += 1
        cur[t] = int.from_bytes(d[o:o + n], 'big'); o += n
    return layers


def bank(path):
    d = open(path, 'rb').read()
    assert d[:4] == b'BNKl'
    ver, n = struct.unpack_from('<HH', d, 4)
    hdr = struct.unpack_from('<III', d, 8)
    progs = {}
    for i in range(n):
        s = 20 + 4 * i
        r = struct.unpack_from('<I', d, s)[0]
        if r:
            progs[i] = parse_pt(d, s + r)
    return ver, n, hdr, progs


def fmt(tags):
    return ' '.join(f'{NAMES.get(k, "%02x" % k)}[{k:02x}]={v:#x}' if k in (0x88,) else
                    f'{NAMES.get(k, "%02x" % k)}[{k:02x}]={v}' for k, v in tags.items())


if __name__ == '__main__':
    for p in sys.argv[1:] or ['zboard.bnk', 'zbxsfx.bnk']:
        ver, n, hdr, progs = bank(p)
        print(f'== {p}: ver {ver} nprog {n} hdr {[hex(x) for x in hdr]} used {len(progs)}')
        alltags = {}
        for i, layers in progs.items():
            for li, (pt, smp) in enumerate(layers):
                print(f'{i:3d}.{li} {fmt(pt)} | {fmt(smp or {})}')
                for k, v in list(pt.items()) + list((smp or {}).items()):
                    alltags.setdefault(k, set()).add(v)
        print('-- tag value sets:')
        for k in sorted(alltags):
            vs = sorted(alltags[k])
            print(f'  {k:02x} {NAMES.get(k, "")}: n={len(vs)} {vs[:16]}{"..." if len(vs) > 16 else ""}')
