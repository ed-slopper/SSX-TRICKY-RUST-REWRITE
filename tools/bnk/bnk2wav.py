#!/usr/bin/env python3
"""Export an SSX Tricky EA sound bank (BNKl) to wav files for tricky-rs.
usage: bnk2wav.py BANK.bnk OUT_DIR PREFIX
Writes PREFIX_<program>.wav (the whole sample of layer 0, played once), PREFIX_<program>_loop.wav
(just the loop, for looping) and PREFIX.json: per program {root, bend, vol, detune, rate, loop} from its
first layer (tags 07 root note, 0A bend range semitones, 0E volume, 10 detune cents).
Additions (backward compatible): codec2 0x0A (EA-XA ADPCM, used by the world-sound / speech banks)
and multi-channel samples (tag 89 = file offset of channel 2) are decoded; json also has chans,
pitch_rand (tag 11, cents, +-) and vol_rand (tag 0F).  Programs with several layers (the game
starts every layer together) also get PREFIX_<program>_l<n>.wav per extra layer, a premixed
PREFIX_<program>_mix.wav (each layer pitched by (60-root) semitones + detune, scaled by vol/127) and
a "layers" list in the json."""
import sys, os, json, struct, wave
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bnkinfo import bank

COEF = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)]
def vag(d, o, n):
    out = []; h1 = h2 = 0
    while len(out) < n and o + 16 <= len(d):
        sh = d[o] & 15; f = (d[o] >> 4) & 7; c1, c2 = COEF[min(f, 4)]
        for b in d[o + 2:o + 16]:
            for nib in (b & 15, b >> 4):
                s = (nib - 16 if nib > 7 else nib) << 12
                s = (s >> sh) + ((h1 * c1 + h2 * c2 + 32) >> 6)
                s = max(-32768, min(32767, s)); out.append(s); h2, h1 = h1, s
        o += 16
    return out[:n]

XA = [(0, 0), (240, 0), (460, -208), (392, -220)]
def eaxa(d, o, n, st=None):
    """EA-XA: 15-byte frames of 28 samples (header: hi nibble coef, lo nibble shift-8), 0xEE = raw frame."""
    out = []; h1, h2 = st if st else (0, 0)
    while len(out) < n and o < len(d):
        info = d[o]
        if info == 0xEE:
            h1, h2 = struct.unpack_from('>hh', d, o + 1)
            out += list(struct.unpack_from('>28h', d, o + 5)); h2, h1 = out[-2], out[-1]; o += 61; continue
        c1, c2 = XA[(info >> 4) & 3]; sh = (info & 15) + 8
        for i in range(28):
            b = d[o + 1 + (i >> 1)]
            nib = (b >> 4) if (i & 1) == 0 else (b & 15)
            s = ((nib << 28) - (1 << 32) if nib & 8 else (nib << 28)) >> sh
            s = (s + c1 * h1 + c2 * h2 + 128) >> 8
            s = -32768 if s < -32768 else 32767 if s > 32767 else s
            out.append(s); h2, h1 = h1, s
        o += 15
    if st is not None: st[0], st[1] = h1, h2
    return out[:n]

def s32(v): return v - (1 << 32) if v >= 1 << 31 else v

def write(path, pcm, rate):
    """pcm: list of samples (mono) or list of per-channel lists."""
    chans = pcm if pcm and isinstance(pcm[0], list) else [pcm]
    n = min(len(c) for c in chans)
    inter = [chans[c][i] for i in range(n) for c in range(len(chans))]
    w = wave.open(path, 'wb'); w.setnchannels(len(chans)); w.setsampwidth(2); w.setframerate(rate)
    w.writeframes(struct.pack('<%dh' % len(inter), *inter)); w.close()

def decode(d, smp):
    """-> list of channels (each a list of s16)."""
    n = smp.get(0x85, 0); ofs = smp.get(0x88, 0); ch = smp.get(0x82, 1); codec = smp.get(0xA0, 5)
    offs = [ofs] + [smp.get(0x89 + k - 1, 0) for k in range(1, ch)]
    out = []
    for o in offs:
        if codec == 9:
            raw = d[o + 16:o + 16 + n]
            out.append([((b - 256) if b > 127 else b) * 256 for b in raw])
        elif codec == 0x0A:
            out.append(eaxa(d, o, n))
        else:
            out.append(vag(d, o, n))
    return out

def cut(pcm, a, b): return [c[a:b] for c in pcm]
def one(pcm): return pcm[0] if len(pcm) == 1 else pcm

def layer_meta(pt, smp):
    ls, le = s32(smp.get(0x86, 0xFFFFFFFF)), s32(smp.get(0x87, 0xFFFFFFFF))
    return dict(root=pt.get(0x07, 60), bend=pt.get(0x0A, 0), vol=pt.get(0x0E, 127),
                detune=s32(pt.get(0x10, 0)) if pt.get(0x10, 0) < 1 << 31 else 0,
                rate=smp.get(0x84, 22050), loop=ls >= 0 and le > ls, chans=smp.get(0x82, 1),
                pitch_rand=pt.get(0x11, 0), vol_rand=pt.get(0x0F, 0)), ls, le

def mix(layers, rate_out):
    """Premix layers (each: (pcm channels, meta)) at the pitch they play at for note 60."""
    tracks = []
    for pcm, m in layers:
        ratio = 2 ** ((60 - m['root']) / 12 + m['detune'] / 1200) * m['rate'] / rate_out
        g = m['vol'] / 127
        res = []
        for c in pcm:
            n = int(len(c) / ratio)
            res.append([g * (c[min(int(i * ratio), len(c) - 1)] * (1 - (i * ratio) % 1) +
                             c[min(int(i * ratio) + 1, len(c) - 1)] * ((i * ratio) % 1)) for i in range(n)])
        tracks.append(res)
    nch = max(len(t) for t in tracks); n = max(len(t[0]) for t in tracks)
    out = [[0.0] * n for _ in range(nch)]
    for t in tracks:
        for c in range(nch):
            src = t[min(c, len(t) - 1)]
            oc = out[c]
            for i, v in enumerate(src): oc[i] += v
    return [[max(-32768, min(32767, int(v))) for v in c] for c in out]

def main(bank_path, out, prefix):
    d = open(bank_path, 'rb').read()
    _, _, _, progs = bank(bank_path)
    os.makedirs(out, exist_ok=True)
    meta = {}
    for i, layers in progs.items():
        pt, smp = layers[0]
        if not smp: continue
        pcm = decode(d, smp)
        m, ls, le = layer_meta(pt, smp)
        write(f'{out}/{prefix}_{i}.wav', one(cut(pcm, 0, le + 1) if le >= 0 else pcm), m['rate'])
        if ls >= 0 and le > ls:
            write(f'{out}/{prefix}_{i}_loop.wav', one(cut(pcm, ls, le + 1)), m['rate'])
        meta[i] = m
        if len(layers) > 1:
            lay = [(pcm, m)]
            m['layers'] = [dict(m, file=f'{prefix}_{i}.wav')]
            for li, (pt2, smp2) in enumerate(layers[1:], 1):
                if not smp2: continue
                p2 = decode(d, smp2); m2, ls2, le2 = layer_meta(pt2, smp2)
                p2 = cut(p2, 0, le2 + 1) if le2 >= 0 else p2
                write(f'{out}/{prefix}_{i}_l{li}.wav', one(p2), m2['rate'])
                lay.append((p2, m2)); m['layers'].append(dict(m2, file=f'{prefix}_{i}_l{li}.wav'))
            for x in m['layers']: x.pop('layers', None)
            write(f'{out}/{prefix}_{i}_mix.wav', one(mix(lay, m['rate'])), m['rate'])
            m['mix'] = f'{prefix}_{i}_mix.wav'
    json.dump(meta, open(f'{out}/{prefix}.json', 'w'), indent=0)
    print(bank_path, len(meta), 'programs')

if __name__ == '__main__':
    main(*sys.argv[1:4])
