#!/usr/bin/env python3
"""Extract an SSX Tricky (PS2) audio BIG (DATA/AUDIO/<LEVEL>.BIG, AUDIO.BIG, ...) to wav + levelaudio.json.

usage: levelaudio.py LEVEL_AUDIO.BIG OUT_DIR [LEVEL_JSON_DIR]
       (also accepts a bare .bnk (BNKl) or a bare EA stream (SCHl) file)
       LEVEL_JSON_DIR (e.g. lvl/all/gari, holding Instances.json / SSFLogic.json): also list the
       course's placed sounds (emitters, collision sounds, script SoundPlay) in levelaudio.json

What is in the files (from the SLUS_203.26 code, see the notes printed into levelaudio.json):
  * DATA/AUDIO/<LEVEL>.BIG (e.g. GARI.BIG) is NOT a sound-effect bank: it is the course's own
    segmented music track (data/config/intromus.inf -> SONG=<name>.big, played by FUN_00214e60 on
    stream channel 2).  Entries "<Song>-A1..A4", "-B1..B4", "-C1..C8", "end" are EA SCHl streams,
    EA-XA ADPCM (PT tag A0 = 0x0A), stereo, 22050 Hz (no 0x84 tag -> engine default 22050),
    128291 samples (5.818 s) each, "end" half length.  They are queued back to back by index:
    0..3 = A1..A4, 4..7 = B1..B4, 8..15 = C1..C8, 16 = end.
  * The ambient/object sounds (River, Snowcat, Birds, ...) are single-program banks
    "|data\\audio\\<Name>.bnk" inside DATA/AUDIO/AUDIO.BIG (the leading '|' = "inside the big");
    run this tool on AUDIO.BIG to extract them - each BNKl entry goes through bnk2wav.
  * Per-course object/collision/SoundPlay sounds use the in-game bank "BANK" (bank slot 2) named
    by data/config/banks.inf, crowds the "CROWD" bank (slot 3).

Output:
  OUT_DIR/<entry>.wav            decoded stream (SCHl), 16-bit, channels/rate as in the header
  OUT_DIR/<entry>/<entry>_<prog>.wav (+ _loop.wav, <entry>.json)   for BNKl banks via bnk2wav
  OUT_DIR/levelaudio.json        list of entries + music sequencing rules + world-sound id table
"""
import sys, os, re, json, struct, wave
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bnkinfo import parse_pt          # noqa: E402
import bnk2wav                         # noqa: E402

# ---------------------------------------------------------------- BIG archive
def read_big(d):
    """EA BIG ('BIGF'/'BIG4'): u32 LE archive size, u32 BE count, u32 BE header size,
    then count x {u32 BE offset, u32 BE size, cstring name}."""
    if d[:4] not in (b'BIGF', b'BIG4'):
        raise ValueError('not a BIG archive')
    n = struct.unpack_from('>I', d, 8)[0]
    o, out = 16, []
    for _ in range(n):
        off, size = struct.unpack_from('>II', d, o); o += 8
        e = d.index(b'\0', o); name = d[o:e].decode('latin-1'); o = e + 1
        out.append((name, off, size))
    return out

# ---------------------------------------------------------------- EA-XA
XA = [(0, 0), (240, 0), (460, -208), (392, -220)]

def eaxa_channel(buf, nsamp, st):
    """Decode one channel's frames of one SCDl block. st = [hist1, hist2] carried across blocks."""
    out = []; o = 0; h1, h2 = st
    while len(out) < nsamp and o < len(buf):
        info = buf[o]
        if info == 0xEE:                     # raw PCM frame (EA-XA v2)
            h1, h2 = struct.unpack_from('>hh', buf, o + 1)
            for i in range(28):
                out.append(struct.unpack_from('>h', buf, o + 5 + 2 * i)[0])
            h2, h1 = out[-2], out[-1]
            o += 61; continue
        c1, c2 = XA[(info >> 4) & 3]; sh = (info & 15) + 8
        for i in range(28):
            b = buf[o + 1 + (i >> 1)]
            nib = (b >> 4) if (i & 1) == 0 else (b & 15)
            s = ((nib << 28) - (1 << 32) if nib & 8 else (nib << 28)) >> sh
            s = (s + c1 * h1 + c2 * h2 + 128) >> 8
            s = -32768 if s < -32768 else 32767 if s > 32767 else s
            out.append(s); h2, h1 = h1, s
        o += 15
    st[0], st[1] = h1, h2
    return out[:nsamp]

# ---------------------------------------------------------------- SCHl stream
def decode_schl(d, base=0, end=None):
    """Returns (header dict, [pcm per channel]).  Header tags as bnkinfo (80 ver, 82 chans,
    83 codec, 84 rate, 85 nsamples, A0 codec2); engine defaults: 1 channel, 22050 Hz, codec2 5."""
    end = len(d) if end is None else end
    assert d[base:base + 4] == b'SCHl', 'not an SCHl stream'
    hsize = struct.unpack_from('<I', d, base + 4)[0]
    layers = parse_pt(d, base + 8)
    tags = dict(layers[0][0]); tags.update(layers[0][1] or {})
    ch = tags.get(0x82, 1); rate = tags.get(0x84, 22050); codec = tags.get(0xA0, 5)
    hdr = dict(channels=ch, rate=rate, codec2=codec, nsamples=tags.get(0x85, 0),
               tags={f'{k:02x}': v for k, v in tags.items()})
    pcm = [[] for _ in range(ch)]
    state = [[0, 0] for _ in range(ch)]
    o = base + hsize
    while o + 8 <= end:
        tag = d[o:o + 4]; size = struct.unpack_from('<I', d, o + 4)[0]
        if tag == b'SCEl' or size < 8:
            break
        if tag == b'SCDl':
            ns = struct.unpack_from('<I', d, o + 8)[0]
            offs = struct.unpack_from('<%dI' % ch, d, o + 12)
            data0 = o + 12 + 4 * ch
            for c in range(ch):
                a = data0 + offs[c]
                b = data0 + offs[c + 1] if c + 1 < ch else o + size
                buf = d[a:b]
                if codec == 0x0A:                    # EA-XA: per-channel prefix s16le hist1, hist2; then frames
                    state[c] = list(struct.unpack_from('<2h', buf, 0))
                    pcm[c] += eaxa_channel(buf[4:], ns, state[c])
                elif codec == 0x05:                  # PSX ADPCM
                    pcm[c] += bnk2wav.vag(buf, 0, ns)
                elif codec in (0x02, 0x09):          # s8
                    pcm[c] += [((x - 256) if x > 127 else x) * 256 for x in buf[:ns]]
                elif codec in (0x07, 0x01):          # s16 BE
                    pcm[c] += list(struct.unpack_from('>%dh' % ns, buf))
                elif codec in (0x08, 0x00):          # s16 LE
                    pcm[c] += list(struct.unpack_from('<%dh' % ns, buf))
                else:
                    raise ValueError(f'SCHl codec2 {codec:#x} not supported')
        o += size
    return hdr, pcm

def write_wav(path, pcm, rate):
    ch = len(pcm); n = min(len(p) for p in pcm)
    inter = [pcm[c][i] for i in range(n) for c in range(ch)]
    w = wave.open(path, 'wb'); w.setnchannels(ch); w.setsampwidth(2); w.setframerate(rate)
    w.writeframes(struct.pack('<%dh' % len(inter), *inter)); w.close()
    return n

def safe(name):
    name = re.sub(r'\s+(\.[A-Za-z0-9]+)$', r'\1', name.strip())   # 'Wolf .bnk' -> 'Wolf.bnk'
    return re.sub(r'[^A-Za-z0-9_.-]+', '_', name.replace('\\', '/').split('/')[-1]).strip('_') or 'entry'

# ---------------------------------------------------------------- game tables (SLUS_203.26)
# World-sound id -> what the game plays.  Sources: SoundId_ToBankProgram FUN_0022d8a8,
# SoundId_IsSwap FUN_0022e388, SoundId_SwapBankFile FUN_0022c648, emitter start FUN_0022bf60.
#   bank "BANK"  = in-game bank slot 2 (per-course bank from data/config/banks.inf)
#   bank "CROWD" = slot 3 (crowd.inf);  bank "SWAP" = slot 6, one small bank file loaded on demand
LEVELBANK_PROG = {
    2: 0x22, 3: 2, 4: 0x30, 5: 1, 6: 0x20, 7: 0x32, 8: 0x29, 9: 0x30, 10: 0x31, 11: 0x21, 12: 0x33,
    13: 0x35, 14: 0x36, 15: 0x33, 16: 0x23, 17: 0x25, 18: 0x13, 19: 0x28, 20: 0x35, 21: 0x35,
    22: 0x34, 23: 0x16, 24: 0x18, 25: 0x12, 26: 0x53, 27: 0x46, 28: 0x18, 29: 0x51, 30: 3,
    31: 0x10, 32: 0x52, 33: 0x32, 34: 0x19, 35: 0x3a, 36: 0x4f, 37: 0x32, 38: 0x61, 39: 0x11,
    40: 0x17, 41: 0x60, 42: 0x61, 43: 0x33, 44: 0x15, 45: 0x14, 46: 0x16, 47: 0x2d, 48: 0x30,
    49: 0x31, 50: 0x2a, 51: 0x38, 52: 0x33, 53: 0x36, 54: 0x60, 55: 0x13, 56: 0x47, 57: 0x24,
    58: 0, 59: 0x2b, 60: 0x21, 61: 0x23, 62: 0x21, 63: 0x40, 64: 0x17, 65: 0x37, 66: 0x41,
    67: 0x70, 68: 0x26, 69: 0x60, 70: 1, 71: 0x22, 72: 0x31, 73: 0x31, 74: 0x34, 75: 0x34,
    76: 0x39, 77: 0x36, 78: 0x35,
    0x94: 6, 0x95: 7, 0x96: 8, 0x97: 9, 0x98: 0x73, 0x99: 0x74, 0x9a: 0x75, 0x9b: 0x76,
    0x9c: 0x77, 0x9d: 0x78, 0x9e: 0x79, 0xb3: 10, 0xb4: 11, 0xb5: 12, 0xb6: 13,
}
SWAP_FILE = {
    0x4f: 'Dynkicker1', 0x50: 'Dynkicker2', 0x51: 'GoGo1', 0x52: 'GoGo2', 0x53: 'Gravelpit',
    0x54: 'Helicopter2', 0x55: 'Megaplexfan', 0x56: 'Movingramp', 0x57: 'Octocar1', 0x58: 'River',
    0x59: 'RiverSmall', 0x5a: 'Snowmachine', 0x5b: 'Snowmachinemist', 0x5c: 'Snowcat',
    0x5d: 'Steam1', 0x5e: 'Steam2', 0x5f: 'Trafficloop', 0x60: 'Truckidle', 0x67: 'Birds1',
    0x68: 'Cowbell', 0x69: 'CowbellsWoo', 0x6a: 'Dog1', 0x6b: 'Tunnel1', 0x6c: 'Woo1', 0x6d: 'Woo2',
    0x6e: 'Bird_Crows', 0x6f: 'Bird_Eagle', 0x70: 'Bird_Flap', 0x71: 'Bird_Owl',
    0x72: 'Bird_Woodpecker', 0x73: 'Coyote', 0x74: 'Wind1', 0x75: 'Wind2', 0x76: 'Wolf',
    0x77: 'Plane_Far', 0x78: 'Birds_Summer', 0x79: 'Birds_Vulture', 0x7a: 'Birds_Tropical',
    0x7b: 'Boat', 0x7c: 'TV1', 0x7d: 'TV2', 0x7e: 'TV3', 0x7f: 'TV4', 0x80: 'Dog2', 0x81: 'Parkade',
    0x82: 'MineShaft', 0x83: 'RattleSnake', 0x84: 'Seagulls1', 0x85: 'Seagulls2',
    0x86: 'Trafficloop', 0x87: 'Penguins', 0x88: 'Birds_Nightingale', 0x89: 'Dog3',
    0x8a: 'Birds_Hawk', 0x8b: 'Birds_Lark', 0x8c: 'Birds_Pigeon', 0x8d: 'Birds_Falcon',
    0x8e: 'Birds_Woodpecker2', 0x8f: 'Birds_Sparrow', 0x90: 'Bird_Crows2', 0x91: 'Bird_Crows3',
    0x92: 'Bird_Eagle2', 0x93: 'Bird_Eagle3', 0x9f: 'SkyTrain_Outdoors', 0xa0: 'SkyTrainStation',
    0xa1: 'Mall', 0xa2: 'Sewer', 0xa3: 'Office', 0xa4: 'Port', 0xa5: 'Alleyway_1',
    0xa6: 'Alleyway_2', 0xa7: 'Alleyway_3', 0xa8: 'Alleyway_4', 0xa9: 'Alleyway_5',
    0xaa: 'Alleyway_6', 0xab: 'Alleyway_7', 0xac: 'Alleyway_8', 0xad: 'ElectricRoof',
    0xae: 'ElectricRoof2', 0xaf: 'Magnetic_Turbo_Donut', 0xb0: 'Super_Conducting_Collider',
    0xb1: 'CopCar', 0xb2: 'Waterfall', 0xb7: 'Construction1', 0xb8: 'Construction2', 0xb9: 'UFO',
}

def world_sound_table():
    t = {}
    for i in range(0, 0xba):
        if i in SWAP_FILE:
            e = dict(bank='SWAP', file=f'data/audio/{SWAP_FILE[i]}.bnk', program=0)
            if i in (0x5f, 0x86):
                e['alt_file'] = 'data/audio/Traffic_Loop2.bnk'   # used while a toggle flag is 0
        elif i in (0x61, 0x62, 0x63):
            e = dict(bank='CROWD', program=i - 0x61)
        elif i == 0x66:
            e = dict(bank='SWAP', file='<random spectator speech bank, list from an .inf parsed by FUN_0022dce0>',
                     program=0)
        elif i in LEVELBANK_PROG:
            e = dict(bank='BANK', program=LEVELBANK_PROG[i])
        elif i == 0x3a:
            e = dict(bank='BANK', program=0)
        else:
            continue
        if i in (0x10, 0x1c, 0x39):
            e['needs_hit'] = True   # emitter only plays after the instance was hit (FUN_0022e240/e270)
        t[i] = e
    return t

MUSIC_RULES = {
    'source': 'FUN_00214e60 (start), FUN_0020ddb0 state machine (refill), FUN_00214e08 (finish)',
    'index': {'A1..A4': [0, 3], 'B1..B4': [4, 7], 'C1..C8': [8, 15], 'end': 16},
    'race_start': 'queue A1, then A[2+rand3], then B[1+rand4], B[1+rand4]  (restart: just one random B)',
    'while_racing': 'when fewer than 2 segments are queued, queue C[1+rand8] (game modes 3,7,8,11: C1..C8 in order, looping)',
    'race_end': 'flush the queue and queue "end" (index 16)',
    'volume': 'music category volume (Audio_CategoryVol 1)',
    'gapless': 'segments are back-to-back parts of one piece; play them gaplessly',
}

AMBIENT_NOTES = {
    'emitters': 'Instances[i].Sounds.ExternalSounds[] (U0=type, SoundIndex=world sound id, U2..U4 offset from Location, U5 radius, U6 curve)',
    'curves': {'0': '1-x^2', '1': '1-x/(1.5-0.5x)', '2': '1-x', '3': '(1-x)/(1+0.5x)', '4': '(1-x)^2',
               '5': '1 if x<=0.7 else (1-x)/0.3'},
    'collision': 'Instances[i].Sounds.CollisonSound = world sound id played on impact',
    'script': 'SSFLogic op 8 SoundPlay = program number in bank BANK (slot 2), played at the instance',
}

# ---------------------------------------------------------------- main
def handle_bank(blob, name, out, entry):
    sub = os.path.join(out, safe(name).rsplit('.', 1)[0])
    os.makedirs(sub, exist_ok=True)
    tmp = os.path.join(sub, '_bank.bnk')
    open(tmp, 'wb').write(blob)
    prefix = safe(name).rsplit('.', 1)[0]
    bnk2wav.main(tmp, sub, prefix)
    os.remove(tmp)
    meta = json.load(open(os.path.join(sub, prefix + '.json')))
    entry.update(type='bank', dir=os.path.relpath(sub, out), json=f'{os.path.relpath(sub, out)}/{prefix}.json',
                 programs={k: dict(v, file=f'{os.path.relpath(sub, out)}/{prefix}_{k}.wav',
                                   loop_file=f'{os.path.relpath(sub, out)}/{prefix}_{k}_loop.wav' if v.get('loop') else None)
                           for k, v in meta.items()})

def handle_stream(d, off, size, name, out, entry):
    hdr, pcm = decode_schl(d, off, off + size)
    fn = safe(name) + '.wav'
    n = write_wav(os.path.join(out, fn), pcm, hdr['rate'])
    entry.update(type='stream', file=fn, channels=hdr['channels'], rate=hdr['rate'], codec2=hdr['codec2'],
                 samples=n, seconds=round(n / hdr['rate'], 4), loop=False, header_tags=hdr['tags'])
    m = re.search(r'-([ABC])(\d+)$', name)
    if m:
        entry['segment'] = m.group(1) + m.group(2)
    elif name.lower() == 'end':
        entry['segment'] = 'end'

def placements(level_dir):
    """Placed sounds of a course, from the level JSON (game units = cm, z up)."""
    ws = world_sound_table()
    ins = json.load(open(os.path.join(level_dir, 'Instances.json')))['Instances']
    emit, coll = [], {}
    for i, x in enumerate(ins):
        snd = x.get('Sounds') or {}
        if not x.get('IncludeSound'):
            continue
        cs = snd.get('CollisonSound', 0)
        if cs:
            coll.setdefault(cs, []).append(i)
        for k, r in enumerate(snd.get('ExternalSounds') or []):
            loc = x['Location']
            e = dict(instance=i, name=x['InstanceName'], record=k, type=r['U0'], sound=r['SoundIndex'],
                     pos=[loc[0] + r['U2'], loc[1] + r['U3'], loc[2] + r['U4']], radius=r['U5'],
                     curve=int(r['U6']) if r['U0'] == 0 else None, target=ws.get(r['SoundIndex']))
            if r['U0'] == 1:
                e['note'] = 'type 1 ellipsoid: exporter only kept radii x,y (U5,U6); z radius/axis/curve missing'
            emit.append(e)
    script = []
    try:
        lg = json.load(open(os.path.join(level_dir, 'SSFLogic.json')))
        for h, hd in enumerate(lg.get('EffectHeaders', [])):
            for j, op in enumerate(hd.get('Effects', [])):
                if op.get('MainType') == 8:
                    script.append(dict(header=h, op=j, program=op['SoundPlay']))
    except FileNotFoundError:
        pass
    return dict(emitters=emit,
                collision_sounds={str(k): dict(instances=len(v), target=ws.get(k)) for k, v in sorted(coll.items())},
                script_sounds=script)

def main(src, out, level_dir=None):
    os.makedirs(out, exist_ok=True)
    d = open(src, 'rb').read()
    if d[:4] in (b'BIGF', b'BIG4'):
        items = read_big(d)
    else:
        items = [(os.path.basename(src), 0, len(d))]
    entries = []
    for idx, (name, off, size) in enumerate(items):
        magic = d[off:off + 4]
        e = dict(index=idx, name=name, offset=off, size=size, magic=magic.decode('latin-1'))
        try:
            if magic == b'SCHl':
                handle_stream(d, off, size, name, out, e)
            elif magic == b'BNKl':
                handle_bank(d[off:off + size], name, out, e)
            else:
                e['type'] = 'unknown'
                open(os.path.join(out, safe(name)), 'wb').write(d[off:off + size])
                e['file'] = safe(name)
        except Exception as ex:          # keep going, report it
            e['error'] = repr(ex)
        print(f"{idx:3d} {name:40s} {e.get('type', '?'):7s} {e.get('file', e.get('dir', ''))} {e.get('error', '')}")
        entries.append(e)
    doc = dict(source=os.path.basename(src), entries=entries,
               kind='course_music' if entries and all(x.get('type') == 'stream' for x in entries) else 'banks')
    if doc['kind'] == 'course_music':
        doc['music_rules'] = MUSIC_RULES
    doc['ambient_notes'] = AMBIENT_NOTES
    doc['world_sounds'] = {str(k): v for k, v in world_sound_table().items()}
    if level_dir:
        doc['placements'] = placements(level_dir)
    json.dump(doc, open(os.path.join(out, 'levelaudio.json'), 'w'), indent=1)
    print(f'{len(entries)} entries -> {out}/levelaudio.json')

if __name__ == '__main__':
    if len(sys.argv) not in (3, 4):
        sys.exit(__doc__)
    main(*sys.argv[1:4])
