#!/usr/bin/env python3
"""Decode an SSX Tricky Pathfinder song (.mpf graph + .mus audio) into one wav per playable node.
usage: mus2wav.py SONG.mpf SONG.mus OUTDIR [--var N] [--preview]

Writes OUTDIR/node_###.wav (### = mpf node id; only nodes with seg > 0 carry audio; 16-bit stereo,
36000 Hz (slaybreak 35999) as stored in the SCHl headers) and OUTDIR/song.json = the mpf2json dump plus
per node: wav, samples, rate; and 'next' = resolved successor table [{lo,hi,node}] for a fresh
PathState (markers and routers already followed; -1 = song stops).  --preview also writes
OUTDIR/default_path.wav (the nodes of default_path concatenated, intro + one pass of the loop).
"""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pathfinder as pf
import mpf2json
import numpy as np


def next_table(m, n):
    """Resolved successor of node n for every path value 0..127 (fresh loop state), as ranges."""
    out = []
    for v in range(128):
        st = pf.PathState(m)
        x = st.next_node(n, v)
        if out and out[-1]['node'] == x and out[-1]['hi'] == v - 1:
            out[-1]['hi'] = v
        else:
            out.append(dict(lo=v, hi=v, node=x))
    return out


def convert(mpf_bytes, mus_bytes, outdir, var=80, preview=False, name=None):
    os.makedirs(outdir, exist_ok=True)
    m = pf.parse_mpf(mpf_bytes)
    j = mpf2json.build(m, var)
    segnodes = [n for n in j['nodes'] if n['seg'] > 0]
    pcm = pf.decode_segments(mus_bytes, [n['mus_offset'] for n in segnodes])
    audio = {}
    for n, (rate, a, tags) in zip(segnodes, pcm):
        fn = 'node_%03d.wav' % n['id']
        pf.write_wav(os.path.join(outdir, fn), rate, a)
        n['wav'] = fn; n['samples'] = int(a.shape[0]); n['rate'] = int(rate); n['channels'] = int(a.shape[1])
        audio[n['id']] = a
    for n in j['nodes']:
        if n['seg'] > 0:
            n['next'] = next_table(m, n['id'])
    dp = j['default_path']
    dp['samples'] = int(sum(audio[i].shape[0] for i in dp['nodes']))
    # boundary continuity along the default path (and its loop-back join)
    seq = dp['nodes'] + ([dp['nodes'][dp['loop_at']]] if dp['loop_at'] is not None else [])
    jumps = []   # |step across the join| / 99.9th percentile of |step| in the 4096 samples either side
    for a, b in zip(seq, seq[1:]):
        x, y = audio[a].astype(np.int32), audio[b].astype(np.int32)
        ctx = np.abs(np.diff(np.concatenate([x[-4096:], y[:4096]]), axis=0)).max(axis=1)
        ctx = np.delete(ctx, min(4096, len(x)) - 1)
        jumps.append(float(np.abs(y[0] - x[-1]).max()) / (float(np.percentile(ctx, 99.9)) + 1.0))
    dp['boundary_jump_ratio_max'] = max(jumps) if jumps else 0
    dp['boundary_jump_ratio_median'] = float(np.median(jumps)) if jumps else 0
    if preview:
        pf.write_wav(os.path.join(outdir, 'default_path.wav'), segnodes[0]['rate'] if segnodes else 36000,
                     np.concatenate([audio[i] for i in dp['nodes']]))
    j['name'] = name
    j['rate'] = segnodes[0]['rate'] if segnodes else None
    j['total_segment_samples'] = int(sum(a.shape[0] for a in audio.values()))
    json.dump(j, open(os.path.join(outdir, 'song.json'), 'w'), indent=1)
    return j


def main():
    args = [x for x in sys.argv[1:] if not x.startswith('--')]
    var = 80
    if '--var' in sys.argv:
        var = int(sys.argv[sys.argv.index('--var') + 1]); args.remove(str(var))
    j = convert(open(args[0], 'rb').read(), open(args[1], 'rb').read(), args[2], var, '--preview' in sys.argv,
                os.path.splitext(os.path.basename(args[0]))[0])
    dp = j['default_path']
    print('%s: %d nodes, %d wavs; default path %d nodes %.2f s (loop from #%s, %.2f s); max boundary jump %.2fx'
          % (args[0], len(j['nodes']), sum(1 for n in j['nodes'] if n['seg'] > 0), len(dp['nodes']),
             dp['samples'] / j['rate'], dp['loop_at'], dp['loop_ms'] / 1000, dp['boundary_jump_ratio_max']))


if __name__ == '__main__':
    main()
