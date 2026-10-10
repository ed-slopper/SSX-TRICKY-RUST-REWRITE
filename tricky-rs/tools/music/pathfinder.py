#!/usr/bin/env python3
"""SSX Tricky (PS2) EA "Pathfinder" interactive music: shared library for mpf2json / mus2wav / musicbig.

Everything here was reverse engineered from SLUS_203.26 (Pathfinder lib 0x2bec00-0x2c2040, Ghidra names
PF_*; game glue Music_* / Audio_* around 0x20e000-0x226000).  See tools/music/README section in the
docstrings of mpf2json.py for the format, and play_path()/next_node() below for the runtime rules.

MPF ("PFDx" little-endian magic 0x50464478 = bytes 'xDFP', version 0x0103) - parsed by PF_RegisterMPF
0x2c0b08.  All "offsets" are in 4-byte words from the start of the file.
  0x00 u32 magic   0x04 u16 version 0x103   0x06 u16 ?(0xb0)   0x08 u32 0
  0x0c u8  instance slot (0; the lib holds 4 instances x 24 tracks)
  0x0d u8  nTracks            0x0e u8 nColumns (event table columns = section ids)
  0x0f u8  nEvents (11)       0x10 u8 nActions       0x11 u8 nRouters
  0x12 u16 nNodes             0x14..0x23 zero
  0x24 u16 nodeOfs[nNodes]    (word offset of each node)
  nodes (12 bytes + 4 per branch):
    +0 s16 seg     >0: segment (1-based) in the track's segment table = one SCHl stream in the .mus
                   0: section START marker (plays nothing), -1: section END marker (plays nothing)
    +2 u8  track   +3 u8 section id (low 7 bits = event-table column; 0x80 = section-start marker; 0xff on END)
    +4 u8  ?(1, 8/16/32 in the menus)  +5 u8 nSub (sync subdivisions, 4)  +6 s8 sync mode (0/4; on START
          markers: 0x7f = cut at node end, -1 = cut at next subdivision, else beat-matched)  +7,+8 sync params
    +9 u8  loop count (END markers: 255 = loop "forever" (127 passes), see next_node)   +10 u8 router id (0 = none)
    +11 u8 nBranches, then nBranches x {s8 lo, s8 hi, u16 dest}: first entry with lo <= pathvar <= hi wins
  event table: u8 [nTracks][nEvents][nColumns] action index (padded to 4)
  actions: nActions x {u8 vol (0xff = keep), u8 flags, s16 target node (-1 none)}
           flags 0x01/0x02 = transition (0x02 with target -1 = stop), 0x80 = immediate (flush the queued
           audio and cut now), 0x40 = fade/hold (unused in Tricky), target with START flag -> no "return"
  routers: u32 ofs[nRouters+1]; router k (1-based) = u32 entries in words [ofs[k-1], ofs[k]) each
           (from<<16 | to): when the node being left has router k, a successor 'from' is replaced by 'to'.
           ofs[nRouters] = word offset of the track table.
  track table: u32 ofs[nTracks] -> segment table per track: u32 musOffset/4, u32 length_ms per segment.

MUS: concatenation of EA SCHl streams (one per segment; 'SCHl' PT header: 0x80 ver 2, 0x82 2 channels,
  0x84 rate 36000 (slaybreak 35999), 0x85 nsamples, 0xA0 codec 0x0A = EA-XA ADPCM v2, 0x8C 4, 0x06 101,
  0x0B 2) followed by SCCl (block count), SCDl blocks and SCEl.  Each SCDl block = u32 nsamples, u32
  chanOfs[2] (relative to byte 20), and per channel s16le hist1, s16le hist2 + ceil(n/28) 15-byte EA-XA
  frames, so every block decodes independently.
"""
import os, re, struct, json

try:
    import numpy as np
except ImportError:          # pragma: no cover
    np = None

# --------------------------------------------------------------------------- containers
class Source:
    """Random access over one file or several concatenated parts (MUSIC.BIG.00 + .01 ...)."""
    def __init__(self, paths):
        if isinstance(paths, str):
            paths = [paths]
            base = paths[0]
            if base.endswith('.00'):
                k = 1
                while os.path.exists(base[:-3] + '.%02d' % k):
                    paths.append(base[:-3] + '.%02d' % k); k += 1
        self.parts = []
        o = 0
        for p in paths:
            n = os.path.getsize(p); self.parts.append((o, n, p)); o += n
        self.size = o
        self.fh = {}

    def read(self, off, n):
        out = bytearray()
        for base, size, p in self.parts:
            if off + n <= base or off >= base + size:
                continue
            a = max(off, base); b = min(off + n, base + size)
            f = self.fh.get(p) or self.fh.setdefault(p, open(p, 'rb'))
            f.seek(a - base); out += f.read(b - a)
        return bytes(out)


def read_big(src):
    """EA BIG ('BIGF'/'BIG4'): u32 size, u32 BE count, u32 BE header size, then {u32 BE off, u32 BE size,
    cstring name}.  -> {lower-case basename: (name, off, size)}"""
    head = src.read(0, 16)
    if head[:4] not in (b'BIGF', b'BIG4'):
        raise ValueError('not a BIG archive')
    n, hsize = struct.unpack_from('>II', head, 8)
    d = src.read(0, hsize + 16)
    o, out = 16, {}
    for _ in range(n):
        off, size = struct.unpack_from('>II', d, o); o += 8
        e = d.index(b'\0', o); name = d[o:e].decode('latin-1'); o = e + 1
        key = re.split(r'[\\/]', name)[-1].lower()
        out[key] = (name, off, size)
    return out


def parse_inf(text):
    """MUSIC.INF -> [ {section, PATHDATA, MUSDATA, LOOPDATA[], BPM, ...} ] with engine defaults
    (SongInstance_Init 0x223d60).  DelayTime switches use the engine's (buggy) formula BPM/60*frac*1000."""
    songs, cur = [], None
    for raw in text.splitlines():
        line = raw.split('#', 1)[0].strip()
        if not line:
            continue
        m = re.match(r'\[(.*)\]$', line)
        if m:
            cur = None
            if m.group(1).upper() != 'GLOBAL':
                cur = dict(section=m.group(1), PATHDATA=None, MUSDATA=None, LOOPDATA=[], BPM=120.0,
                           BeatsPerMeasure=4, MeasuresPerBar=2, PhrasesPerBank=4, BeatsPerPhrase=8,
                           PhraseAlign=16, DelayCount=0, DelayTime=100, DelayFeedback=90, DelayLevel=50,
                           PathLevel=100, AsyncLevel=100, ZonePhrases=2)
                songs.append(cur)
            continue
        if cur is None:
            continue
        if '=' in line:
            k, v = [x.strip() for x in line.split('=', 1)]
            v = v.strip('"')
            keys = {x.lower(): x for x in cur}
            k2 = keys.get(k.lower(), k)
            if k2 == 'LOOPDATA':
                cur['LOOPDATA'].append(v)
            elif k2 in ('PATHDATA', 'MUSDATA'):
                cur[k2] = v
            elif k2 == 'BPM':
                cur['BPM'] = float(v)
            else:
                try:
                    cur[k2] = int(float(v))
                except ValueError:
                    cur[k2] = v
        else:
            frac = {'sixteenth': 0.0625, 'eighth': 0.125, 'quarter': 0.25, 'whole': 1.0}.get(line.lower())
            if frac is not None:
                cur['DelayTime'] = int(cur['BPM'] / 60.0 * frac * 1000.0)
                cur['DelaySwitch'] = line
    return songs

# --------------------------------------------------------------------------- MPF
def parse_mpf(d):
    if d[:4] != b'xDFP' or struct.unpack_from('<H', d, 4)[0] != 0x103:
        raise ValueError('not a PFDx v0x103 .mpf')
    ntr, ncol, nev, nact, nrt = d[0xd], d[0xe], d[0xf], d[0x10], d[0x11]
    n = struct.unpack_from('<H', d, 0x12)[0]
    offs = struct.unpack_from('<%dH' % n, d, 0x24)
    nodes = []
    for i, w in enumerate(offs):
        o = w * 4
        seg, = struct.unpack_from('<h', d, o)
        nb = d[o + 0xb]
        br = [dict(lo=struct.unpack_from('<b', d, o + 0xc + 4 * k)[0],
                   hi=struct.unpack_from('<b', d, o + 0xd + 4 * k)[0],
                   dest=struct.unpack_from('<h', d, o + 0xe + 4 * k)[0]) for k in range(nb)]
        kind = 'segment' if seg > 0 else ('start' if seg == 0 else 'end')
        nodes.append(dict(id=i, seg=seg, kind=kind, track=d[o + 2], section=d[o + 3] & 0x7f if d[o + 3] != 0xff else -1,
                          flags=d[o + 3], b4=d[o + 4], nsub=d[o + 5], sync=struct.unpack_from('<b', d, o + 6)[0],
                          sync7=d[o + 7], sync8=d[o + 8], loop=d[o + 9], router=d[o + 10], branches=br))
    last = offs[-1] * 4
    p = last + 0xc + d[last + 0xb] * 4
    evt = d[p:p + ntr * nev * ncol]
    events = [[[evt[(t * nev + e) * ncol + c] for c in range(ncol)] for e in range(nev)] for t in range(ntr)]
    p += (ntr * nev * ncol + 3) & ~3
    actions = []
    for i in range(nact):
        vol, fl, tgt = struct.unpack_from('<BBh', d, p + 4 * i)
        actions.append(dict(id=i, vol=vol if vol < 0x80 else None, flags=fl, target=tgt,
                            immediate=bool(fl & 0x80), transition=bool(fl & 0x03)))
    p += nact * 4
    rofs = struct.unpack_from('<%dI' % (nrt + 1), d, p)
    routers = []
    for k in range(1, nrt + 1):
        ent = [struct.unpack_from('<I', d, 4 * j)[0] for j in range(rofs[k - 1], rofs[k])]
        routers.append([dict(src=(e >> 16) - ((e >> 15) & 0x10000), dst=e & 0xffff) for e in ent])  # src signed
    tofs = struct.unpack_from('<%dI' % ntr, d, rofs[nrt] * 4)
    tracks = []
    for t in range(ntr):
        a = tofs[t] * 4
        b = tofs[t + 1] * 4 if t + 1 < ntr else len(d)
        segs = [dict(seg=k + 1, mus_offset=struct.unpack_from('<I', d, a + 8 * k)[0] * 4,
                     ms=struct.unpack_from('<I', d, a + 8 * k + 4)[0]) for k in range((b - a) // 8)]
        tracks.append(segs)
    return dict(version=0x103, tracks=ntr, columns=ncol, events_n=nev, nodes=nodes, events=events,
                actions=actions, routers=routers, segments=tracks)

# --------------------------------------------------------------------------- runtime model
class PathState:
    """Per-track Pathfinder state needed for successor selection (PF_NextNodeByVar 0x2bf038,
    PF_ResolveNode 0x2bf108, PF_ApplyRouter 0x2bf290)."""
    def __init__(self, mpf):
        self.m = mpf
        self.loop_node = -1      # track+0x0c
        self.loop_cnt = 0        # track+0x05 (u8)
        self.section_start = -1  # track+0x10 (last START marker passed)

    def branch(self, n, var):
        nd = self.m['nodes'][n]
        if self.loop_node == n:
            var = self.loop_cnt & 0x7f
            if nd['loop']:
                self.loop_cnt = (self.loop_cnt - 1) & 0xff
                if self.loop_cnt == 0xff:
                    self.loop_node = -1
        for b in nd['branches']:
            if b['lo'] <= var <= b['hi']:
                return b['dest']
        return -1

    def route(self, cur, n):
        if cur < 0:
            return n
        r = self.m['nodes'][cur]['router']
        if r:
            for e in self.m['routers'][r - 1]:
                if n == e['src']:
                    n = e['dst']
        return n

    def resolve(self, cur, n, var):
        """Follow START/END markers from candidate n (left node = cur) to the next playable node or -1."""
        n = self.route(cur, n)
        guard = 0
        while n >= 0 and self.m['nodes'][n]['seg'] < 1:
            nd = self.m['nodes'][n]
            if nd['seg'] == 0:
                self.section_start = n
            elif nd['loop'] and n != self.loop_node:
                self.loop_cnt = nd['loop']; self.loop_node = n
            n = self.route(cur, self.branch(n, var))
            guard += 1
            if guard > 10000:
                raise RuntimeError('marker loop')
        return self.route(cur, n)   # PF_ResolveNode re-applies the router to the final node

    def next_node(self, cur, var):
        """Successor when segment node 'cur' finishes (PF_AdvanceNode 0x2bfbb8)."""
        return self.resolve(cur, self.branch(cur, var), var)

    def event_action(self, cur, event, track=0):
        """PF_LookupEventAction 0x2bf320: action for an event given the node now playing."""
        col = self.m['nodes'][cur]['flags'] & 0x7f if cur >= 0 else 0
        return self.m['actions'][self.m['events'][track][event][col]]


def default_path(mpf, var=80, start_event=0, max_nodes=100000):
    """Nodes played from event 0 with a constant path variable until the song loops back (or ends).
    Returns (order, loop_index) where order[loop_index:] repeats forever (loop_index None = song ends)."""
    st = PathState(mpf)
    act = st.event_action(-1, start_event)
    n = st.resolve(-1, act['target'], var)
    seen, order = {}, []
    def matters(mk):   # does the END marker's loop counter change its successor?
        return mk >= 0 and len({b['dest'] for b in mpf['nodes'][mk]['branches']}) > 1
    while n >= 0 and len(order) < max_nodes:
        key = (n, st.loop_node, st.loop_cnt if matters(st.loop_node) else 0)
        if key in seen:
            return order, seen[key]
        seen[key] = len(order); order.append(n)
        n = st.next_node(n, var)
    return order, None

# --------------------------------------------------------------------------- MUS / EA-XA
XA = [(0, 0), (240, 0), (460, -208), (392, -220)]


def schl_blocks(d, base):
    """Parse one SCHl stream at d[base:] -> (tags, [(ns, [(hist1, hist2, frames_bytes) per ch])], end)."""
    import sys
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'bnk'))
    from bnkinfo import parse_pt
    if d[base:base + 4] != b'SCHl':
        raise ValueError('no SCHl at %#x' % base)
    hs = struct.unpack_from('<I', d, base + 4)[0]
    L = parse_pt(d, base + 8)
    tags = dict(L[0][0]); tags.update(L[0][1] or {})
    ch = tags.get(0x82, 1)
    p = base + hs
    blocks = []
    while p + 8 <= len(d):
        tag = d[p:p + 4]; size = struct.unpack_from('<I', d, p + 4)[0]
        if size < 8:
            break
        if tag == b'SCDl':
            ns = struct.unpack_from('<I', d, p + 8)[0]
            offs = struct.unpack_from('<%dI' % ch, d, p + 12)
            cl = []
            for c in range(ch):
                a = p + 12 + 4 * ch + offs[c]
                h1, h2 = struct.unpack_from('<hh', d, a)
                nf = (ns + 27) // 28
                cl.append((h1, h2, d[a + 4:a + 4 + 15 * nf]))
            blocks.append((ns, cl))
        p += size
        if tag == b'SCEl':
            break
    return tags, blocks, p


def decode_blocks_np(blocks):
    """Vectorised EA-XA v2 decode of many independent (ns, hist1, hist2, frames) lanes -> list of int16 arrays.
    Raw (0xEE) frames are decoded per sample in python (none occur in the Tricky .mus files)."""
    L = len(blocks)
    if L == 0:
        return []
    T = max(b[0] for b in blocks)
    nf = (T + 27) // 28
    hdr = np.zeros((L, nf), np.uint8); dat = np.zeros((L, nf, 14), np.uint8)
    raw = []
    for i, (ns, h1, h2, fr) in enumerate(blocks):
        k = len(fr) // 15
        a = np.frombuffer(fr[:k * 15], np.uint8).reshape(k, 15)
        hdr[i, :k] = a[:, 0]; dat[i, :k] = a[:, 1:]
        if (a[:, 0] == 0xEE).any():
            raw.append(i)
    nib = np.empty((L, nf, 28), np.int32)
    nib[:, :, 0::2] = dat >> 4; nib[:, :, 1::2] = dat & 15
    nib = np.where(nib >= 8, nib - 16, nib)
    sh = (20 - (hdr & 15).astype(np.int32))[:, :, None]
    base = (nib << sh).reshape(L, nf * 28)[:, :T].astype(np.int64)
    ci = ((hdr >> 4) & 3).astype(np.int64)
    c1 = np.array([c[0] for c in XA], np.int64)[ci]; c2 = np.array([c[1] for c in XA], np.int64)[ci]
    c1 = np.repeat(c1, 28, axis=1)[:, :T]; c2 = np.repeat(c2, 28, axis=1)[:, :T]
    h1 = np.array([b[1] for b in blocks], np.int64); h2 = np.array([b[2] for b in blocks], np.int64)
    out = np.empty((L, T), np.int16)
    for t in range(T):
        s = (base[:, t] + c1[:, t] * h1 + c2[:, t] * h2 + 128) >> 8
        np.clip(s, -32768, 32767, out=s)
        out[:, t] = s
        h2 = h1; h1 = s
    res = [out[i, :blocks[i][0]] for i in range(L)]
    for i in raw:
        res[i] = np.array(eaxa_py(blocks[i][3], blocks[i][0], blocks[i][1], blocks[i][2]), np.int16)
    return res


def eaxa_py(fr, n, h1, h2):
    out, o = [], 0
    while len(out) < n and o < len(fr):
        info = fr[o]
        if info == 0xEE:
            h1, h2 = struct.unpack_from('>hh', fr, o + 1)
            out += list(struct.unpack_from('>28h', fr, o + 5)); h2, h1 = out[-2], out[-1]; o += 61; continue
        c1, c2 = XA[(info >> 4) & 3]; sh = (info & 15) + 8
        for i in range(28):
            b = fr[o + 1 + (i >> 1)]
            nib = (b >> 4) if (i & 1) == 0 else (b & 15)
            s = ((nib << 28) - (1 << 32) if nib & 8 else (nib << 28)) >> sh
            s = (s + c1 * h1 + c2 * h2 + 128) >> 8
            s = -32768 if s < -32768 else 32767 if s > 32767 else s
            out.append(s); h2, h1 = h1, s
        o += 15
    return out[:n]


def decode_segments(mus, offsets, batch_lanes=4096):
    """Decode the SCHl streams at the given .mus byte offsets.  -> list of (rate, int16 array [n, ch])."""
    parsed = [schl_blocks(mus, o) for o in offsets]
    lanes, where = [], []
    for si, (tags, blocks, _) in enumerate(parsed):
        if tags.get(0xA0, 5) != 0x0A:
            raise ValueError('segment %d: codec %#x (only EA-XA 0x0A supported)' % (si, tags.get(0xA0, 5)))
        for bi, (ns, cl) in enumerate(blocks):
            for c, (h1, h2, fr) in enumerate(cl):
                lanes.append((ns, h1, h2, fr)); where.append((si, bi, c))
    pcm = []
    for a in range(0, len(lanes), batch_lanes):
        pcm += decode_blocks_np(lanes[a:a + batch_lanes])
    acc = [[[] for _ in range(p[0].get(0x82, 1))] for p in parsed]
    for (si, bi, c), x in zip(where, pcm):
        acc[si][c].append(x)
    out = []
    for si, (tags, _, _) in enumerate(parsed):
        chans = [np.concatenate(x) if x else np.zeros(0, np.int16) for x in acc[si]]
        out.append((tags.get(0x84, 22050), np.stack(chans, axis=1), tags))
    return out


def write_wav(path, rate, pcm):
    import wave
    pcm = np.ascontiguousarray(pcm, dtype='<i2')
    w = wave.open(path, 'wb'); w.setnchannels(pcm.shape[1]); w.setsampwidth(2); w.setframerate(int(rate))
    w.writeframes(pcm.tobytes()); w.close()
