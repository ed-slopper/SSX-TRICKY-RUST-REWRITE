#!/usr/bin/env python3
"""SSX Tricky (PS2) EA "Pathfinder" song -> per-node wavs + graph.json for the Rust player.

usage: mpf2json.py SONG.mpf SONG.mus OUT_DIR [--bpm N | --inf MUSIC.INF] [--var 80] [--preview]

Writes
  OUT_DIR/node_NNN.wav   one per playable node (NNN = mpf node id, seg > 0), 16-bit stereo, rate from the
                         SCHl header (36000 Hz; slaybreak 35999).  Markers (seg 0/-1) carry no audio.
  OUT_DIR/graph.json     everything the player needs (see "graph.json" below)
  OUT_DIR/preview.wav    (--preview) the default path from event 0 at var 80, intro + one loop pass

The format, the runtime rules and the Rust spec are in ../music/README.md and ./SPEC.md; the parser,
runtime model (PathState) and EA-XA decoder are shared with ../music/pathfinder.py.

graph.json
  rate, channels, bpm (if known), beat_samples, lookahead_ms (500: PF_OpenTrack from SongInstance_Init)
  nodes[]: id, kind (segment|start|end), seg, section (event-table column), main (START marker flag 0x80),
           wav, samples, ms, beats, branches [{lo,hi,dest}] (FIRST range with lo<=v<=hi wins, signed,
           inclusive; ranges overlap at the edges so e.g. v=28 in (0,28),(28,66) takes the first),
           router (0 = none, else 1-based index into routers), loop (END markers: pass counter),
           next: resolved successor for a fresh loop state, as [{lo,hi,node}] over v = 0..127,
           next_stateful: true if the walk to the successor crosses an END marker with loop>0 (then
           the player must run the real resolve() with its loop counter instead of the table)
  routers[k-1] = [{src,dst}]
  actions[] {id, vol (null = keep), flags, target, immediate (0x80), transition (flags&3),
             stop (target -1 & flags 0x02), restore (target START marker lacks flag 0x80)}
  events[e] = {action (column 0), per_column [action ids], entry_node (resolved from no node)}
  start_event 0, finish_event 10 (race), default_path {var, nodes, loop_at}, finish_path
"""
import sys, os, json
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'music'))
import pathfinder as pf          # noqa: E402
import numpy as np               # noqa: E402


def next_table(m, n):
    """Successor of segment node n for every v in 0..127 with a fresh PathState; and whether the walk
    touched a looping END marker (successor then depends on the run-time loop counter)."""
    out, stateful = [], False
    for v in range(128):
        st = pf.PathState(m)
        x = st.next_node(n, v)
        if st.loop_node >= 0:
            stateful = True
        if out and out[-1]['node'] == x and out[-1]['hi'] == v - 1:
            out[-1]['hi'] = v
        else:
            out.append(dict(lo=v, hi=v, node=x))
    return out, stateful


def bpm_from_inf(inf_path, mpf_path):
    key = os.path.basename(mpf_path).lower().replace('data_audio_', '')
    for s in pf.parse_inf(open(inf_path, encoding='latin-1').read()):
        if (s['PATHDATA'] or '').lower() == key:
            return s['BPM'], s
    return None, None


def build(mpf_b, mus_b, outdir, bpm=None, var=80, preview=False):
    os.makedirs(outdir, exist_ok=True)
    m = pf.parse_mpf(mpf_b)
    segnodes = [n for n in m['nodes'] if n['seg'] > 0]
    for n in segnodes:
        s = m['segments'][n['track']][n['seg'] - 1]
        n['ms'] = s['ms']; n['mus_offset'] = s['mus_offset']
    dec = pf.decode_segments(mus_b, [n['mus_offset'] for n in segnodes])
    audio, rate, chans = {}, None, None
    clip = 0; peak = 0; total = 0
    for n, (r, a, tags) in zip(segnodes, dec):
        fn = 'node_%03d.wav' % n['id']
        pf.write_wav(os.path.join(outdir, fn), r, a)
        audio[n['id']] = a
        rate = rate or int(r); chans = chans or int(a.shape[1])
        n['wav'] = fn; n['samples'] = int(a.shape[0]); n['rate'] = int(r)
        if 0x85 in tags and tags[0x85] != a.shape[0]:
            raise ValueError('node %d: header says %d samples, decoded %d' % (n['id'], tags[0x85], a.shape[0]))
        clip += int((np.abs(a.astype(np.int32)) >= 32767).sum()); peak = max(peak, int(np.abs(a.astype(np.int32)).max()))
        total += a.size
    beat = rate * 60.0 / bpm if bpm else None
    nodes = []
    for n in m['nodes']:
        o = dict(id=n['id'], kind=n['kind'], seg=n['seg'], section=n['flags'] & 0x7f if n['flags'] != 0xff else -1,
                 main=bool(n['flags'] & 0x80) and n['seg'] == 0, branches=n['branches'], router=n['router'],
                 loop=n['loop'], raw=dict(b4=n['b4'], nsub=n['nsub'], sync=n['sync'], s7=n['sync7'], s8=n['sync8']))
        if n['seg'] > 0:
            o.update(wav=n['wav'], samples=n['samples'], ms=n['ms'], mus_offset=n['mus_offset'])
            if beat:
                o['beats'] = round(n['samples'] / beat, 3)
            o['next'], o['next_stateful'] = next_table(m, n['id'])
        nodes.append(o)
    actions = []
    for a in m['actions']:
        t = a['target']
        actions.append(dict(id=a['id'], vol=a['vol'], flags=a['flags'], target=t, immediate=a['immediate'],
                            transition=a['transition'], stop=t < 0 and bool(a['flags'] & 2),
                            restore=t >= 0 and not (m['nodes'][t]['flags'] & 0x80)))
    events = []
    for e in range(m['events_n']):
        cols = m['events'][0][e]
        a = m['actions'][cols[0]]
        ent = pf.PathState(m).resolve(-1, a['target'], var) if a['target'] >= 0 else -1
        events.append(dict(event=e, action=cols[0], per_column=cols, entry_node=ent))
    order, loop_at = pf.default_path(m, var)
    fin, st = [], pf.PathState(m)
    fe = min(10, m['events_n'] - 1)
    a = m['actions'][m['events'][0][fe][0]]
    x = st.resolve(-1, a['target'], var) if a['target'] >= 0 else -1
    while x >= 0 and x not in fin:
        fin.append(x); x = st.next_node(x, var)
    # ---- checks: header lengths vs ms, beats, joins along the default path
    ms_err = max(abs(n['samples'] * 1000.0 / rate - n['ms']) for n in segnodes)
    seq = order + ([order[loop_at]] if loop_at is not None else [])
    jumps = []
    for p, q in zip(seq, seq[1:]):
        xa, ya = audio[p].astype(np.int32), audio[q].astype(np.int32)
        ctx = np.abs(np.diff(np.concatenate([xa[-4096:], ya[:4096]]), axis=0)).max(axis=1)
        ctx = np.delete(ctx, min(4096, len(xa)) - 1)
        jumps.append(float(np.abs(ya[0] - xa[-1]).max()) / (float(np.percentile(ctx, 99.9)) + 1.0))
    checks = dict(peak=peak, clipped_samples=clip, clipped_fraction=clip / max(total, 1),
                  max_len_vs_header_ms=round(ms_err, 3), join_jump_ratio_max=max(jumps) if jumps else 0,
                  join_jump_ratio_median=float(np.median(jumps)) if jumps else 0)
    if beat:
        bs = [n['samples'] / beat for n in segnodes]
        checks['beats_hist'] = {str(k): v for k, v in sorted(
            __import__('collections').Counter(int(round(b)) for b in bs).items())}
        checks['beats_max_dev'] = round(max(abs(b - round(b)) for b in bs), 4)
    g = dict(format='ssx-tricky-pathfinder/1', rate=rate, channels=chans, bpm=bpm,
             beat_samples=beat, lookahead_ms=500, nodes=nodes, routers=m['routers'], actions=actions,
             events=events, start_event=0, finish_event=fe,
             default_path=dict(var=var, nodes=order, loop_at=loop_at,
                               samples=int(sum(audio[i].shape[0] for i in order))),
             finish_path=dict(event=fe, nodes=fin, ends=x < 0,
                              samples=int(sum(audio[i].shape[0] for i in fin))),
             checks=checks)
    json.dump(g, open(os.path.join(outdir, 'graph.json'), 'w'), indent=1)
    if preview:
        pf.write_wav(os.path.join(outdir, 'preview.wav'), rate, np.concatenate([audio[i] for i in order]))
    return g


def main():
    av = sys.argv[1:]
    opt = {}
    for k in ('--bpm', '--inf', '--var'):
        if k in av:
            i = av.index(k); opt[k] = av[i + 1]; del av[i:i + 2]
    preview = '--preview' in av
    av = [a for a in av if a != '--preview']
    if len(av) != 3:
        sys.exit(__doc__)
    bpm = float(opt['--bpm']) if '--bpm' in opt else None
    if bpm is None and '--inf' in opt:
        bpm, _ = bpm_from_inf(opt['--inf'], av[0])
    g = build(open(av[0], 'rb').read(), open(av[1], 'rb').read(), av[2], bpm, int(opt.get('--var', 80)), preview)
    dp = g['default_path']; c = g['checks']
    print('%s: %d nodes (%d with audio) @ %d Hz; default path %d nodes %.1f s, loop from #%s; finish %d nodes'
          % (av[0], len(g['nodes']), sum(1 for n in g['nodes'] if n['kind'] == 'segment'), g['rate'],
             len(dp['nodes']), dp['samples'] / g['rate'], dp['loop_at'], len(g['finish_path']['nodes'])))
    print('checks:', json.dumps(c))


if __name__ == '__main__':
    main()
