#!/usr/bin/env python3
"""Pack musicbig.py's output for the game: per song one .ogg of every segment in segment order
(sample-exact offsets in graph.json), the Pathfinder graph in a compact form, and the loop bank.

usage: pack.py MUSIC_OUT_DIR PACK_DIR
  MUSIC_OUT_DIR  what musicbig.py wrote (<key>/song.json, node_###.wav, loops/)
  PACK_DIR       gets <key>/song.ogg, <key>/graph.json, <key>/loops/*.wav + loops.json, songs.json
"""
import json, os, shutil, subprocess, sys, wave


def pcm(path):
    with wave.open(path, 'rb') as w:
        return w.getnchannels(), w.getframerate(), w.readframes(w.getnframes())


def main():
    src, dst = sys.argv[1], sys.argv[2]
    os.makedirs(dst, exist_ok=True)
    songs = json.load(open(os.path.join(src, 'songs.json')))
    out_songs = []
    for s in songs:
        key = s.get('key') or os.path.splitext(s.get('PATHDATA', ''))[0]
        sj = os.path.join(src, key, 'song.json')
        if not os.path.exists(sj):
            continue
        d = json.load(open(sj))
        nodes = d['nodes']
        by_seg = {n['seg']: n for n in nodes if n['seg'] > 0 and n.get('wav')}
        segs = sorted(by_seg)
        if not segs:
            continue
        od = os.path.join(dst, key)
        os.makedirs(od, exist_ok=True)
        raw = bytearray()
        starts, lens = {}, {}
        ch = rate = None
        for sg in segs:
            c, r, b = pcm(os.path.join(src, key, by_seg[sg]['wav']))
            ch, rate = c, r
            starts[sg] = len(raw) // (2 * c)
            lens[sg] = len(b) // (2 * c)
            raw += b
        rawp = os.path.join(od, 'all.raw')
        open(rawp, 'wb').write(raw)
        subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 's16le', '-ar', str(rate), '-ac', str(ch), '-i', rawp,
                        '-c:a', 'libvorbis', '-q:a', '5', os.path.join(od, 'song.ogg')], check=True)
        os.remove(rawp)
        g = {
            'name': d.get('name'), 'rate': rate, 'channels': ch, 'total': len(raw) // (2 * ch),
            'segments': {str(k): [starts[k], lens[k]] for k in segs},
            'nodes': {str(n['id']): {'seg': n['seg'], 'col': n['flags'] & 0x7f, 'loop': n['loop'], 'router': n['router'],
                                     'br': [[b['lo'], b['hi'], b['dest']] for b in n['branches']]} for n in nodes},
            'events': d['events'][0],
            'actions': [[a['flags'], a['target']] for a in d['actions']],
            'routers': [[[r['src'], r['dst']] for r in rt] for rt in d['routers']],
        }
        json.dump(g, open(os.path.join(od, 'graph.json'), 'w'), separators=(',', ':'))
        ld = os.path.join(src, key, 'loops')
        if os.path.isdir(ld):
            os.makedirs(os.path.join(od, 'loops'), exist_ok=True)
            for f in os.listdir(ld):
                if f.endswith('.json') or (f.endswith('.wav') and '_l' not in f and not f.endswith('_mix.wav') and not f.endswith('_loop.wav')):
                    shutil.copy(os.path.join(ld, f), os.path.join(od, 'loops', f))
        s2 = dict(s); s2['key'] = key
        out_songs.append(s2)
        print(f'{key:12s} {len(segs)} segments {g["total"] / rate:.1f} s')
    json.dump(out_songs, open(os.path.join(dst, 'songs.json'), 'w'), indent=1)


if __name__ == '__main__':
    main()
