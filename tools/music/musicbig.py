#!/usr/bin/env python3
"""Batch-convert all SSX Tricky Pathfinder songs.
usage: musicbig.py SRC MUSIC.INF OUTDIR [--var N] [--preview] [--only key,key]

SRC = MUSIC.BIG (or MUSIC.BIG.00; .01, .02 ... parts are concatenated automatically) or a directory
holding the loose .mpf / .mus / .bnk files (case-insensitive names).
For every [section] of MUSIC.INF (songkey = PATHDATA basename without extension, e.g. 'smartbomb'):
  OUTDIR/<songkey>/song.json + node_###.wav     via mus2wav (if the .mus exists; else graph-only song.json)
  OUTDIR/<songkey>/loops/loops_<prog>.wav ...   the LOOPDATA bank(s) via tools/bnk/bnk2wav.py (prefix
                                                 'loops' for the first bank, 'loops<k>' for more)
  OUTDIR/songs.json  [{key, section, params from MUSIC.INF (engine defaults filled in), mpf, mus, loops:
                       {bank, prefix, programs[] (used program numbers), phrases{phrase: [beat progs]}},
                       rate, default_path_s, loop_s}]
"""
import sys, os, json, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'bnk'))
import pathfinder as pf
import mpf2json, mus2wav
import bnk2wav
from bnkinfo import bank as parse_bank


class Store:
    def __init__(self, src):
        self.dir = None
        if os.path.isdir(src):
            self.dir = {f.lower(): os.path.join(src, f) for f in os.listdir(src)}
        else:
            self.src = pf.Source(src)
            self.big = pf.read_big(self.src)

    def get(self, name):
        if not name:
            return None
        key = name.replace('\\', '/').split('/')[-1].lower()
        if self.dir is not None:
            p = self.dir.get(key)
            return open(p, 'rb').read() if p else None
        e = self.big.get(key)
        return self.src.read(e[1], e[2]) if e else None


def main():
    args = [x for x in sys.argv[1:] if not x.startswith('--')]
    var, only = 80, None
    if '--var' in sys.argv:
        var = int(sys.argv[sys.argv.index('--var') + 1]); args.remove(str(var))
    if '--only' in sys.argv:
        only = set(sys.argv[sys.argv.index('--only') + 1].split(',')); args = [a for a in args if set(a.split(',')) != only]
    src, inf, out = args[:3]
    store = Store(src)
    songs = pf.parse_inf(open(inf, 'r', encoding='latin-1').read())
    os.makedirs(out, exist_ok=True)
    index = []
    for s in songs:
        key = os.path.splitext(s['PATHDATA'] or s['section'])[0].lower()
        if only and key not in only:
            continue
        dst = os.path.join(out, key)
        os.makedirs(dst, exist_ok=True)
        ent = dict(key=key, **s)
        mpf = store.get(s['PATHDATA']); mus = store.get(s['MUSDATA'])
        ent['mpf_found'] = mpf is not None; ent['mus_found'] = mus is not None
        if mpf is not None and mus is not None:
            j = mus2wav.convert(mpf, mus, dst, var, '--preview' in sys.argv, key)
        elif mpf is not None:
            j = mpf2json.build(pf.parse_mpf(mpf), var)
            json.dump(j, open(os.path.join(dst, 'song.json'), 'w'), indent=1)
        else:
            j = None
        if j:
            ent['nodes'] = len(j['nodes']); ent['segments'] = len(j['segments'][0])
            ent['rate'] = j.get('rate'); ent['default_path_s'] = j['default_path']['ms'] / 1000.0
            ent['loop_s'] = j['default_path']['loop_ms'] / 1000.0
            if 'boundary_jump_ratio_max' in j['default_path']:
                ent['boundary_jump_ratio_max'] = j['default_path']['boundary_jump_ratio_max']
        ent['loops'] = []
        for k, b in enumerate(s['LOOPDATA']):
            data = store.get(b)
            if data is None:
                ent['loops'].append(dict(bank=b, found=False)); continue
            prefix = 'loops' if k == 0 else 'loops%d' % k
            with tempfile.TemporaryDirectory() as td:
                p = os.path.join(td, 'bank.bnk'); open(p, 'wb').write(data)
                bnk2wav.main(p, os.path.join(dst, 'loops'), prefix)
                progs = sorted(parse_bank(p)[3])
            al = s['PhraseAlign']
            ph = {}
            for pr in progs:
                ph.setdefault(pr // al, []).append(pr % al)
            ent['loops'].append(dict(bank=b, found=True, prefix=prefix, dir='loops', programs=progs,
                                     phrases={str(k2): v for k2, v in sorted(ph.items())}))
        index.append(ent)
        print('%-12s %-32s nodes %-4s default %.1fs loops %s' % (key, s['section'], ent.get('nodes'),
              ent.get('default_path_s', 0), [len(l.get('programs', [])) for l in ent['loops']]))
    json.dump(index, open(os.path.join(out, 'songs.json'), 'w'), indent=1)


if __name__ == '__main__':
    main()
