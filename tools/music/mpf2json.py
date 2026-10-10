#!/usr/bin/env python3
"""Dump an SSX Tricky Pathfinder graph (.mpf) to json.
usage: mpf2json.py SONG.mpf OUT.json [--var N]

Format: see pathfinder.py docstring.  The json carries:
  nodes[]   id, kind (segment|start|end), seg (1-based segment, 0 start marker, -1 end marker), section,
            branches [{lo,hi,dest}] (first lo<=var<=hi wins), router (0/none or 1-based router id),
            loop (END-marker loop counter, 255 = repeat), nsub/sync (beat-sync data, unused by Tricky
            because the song track is opened with 500 ms lookahead), ms (segment length), mus_offset
  routers[] [{src,dst}]  applied to the successor of a node whose 'router' is set
  events[track][event][column] -> action index; actions[] {vol, flags, target, immediate}
  event_entry[e]  per column: resolved first playable node the event jumps to (-1 = stop / none)
  segments[track][] {seg, mus_offset (bytes), ms}
  default_path    nodes played from event 0 at path var --var (default 80, the race-start value),
                  'loop_at' = index where the order starts repeating
"""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pathfinder as pf


def build(mpf, var=80):
    m = mpf
    segs = {s['seg']: s for s in m['segments'][0]}
    for n in m['nodes']:
        if n['seg'] > 0:
            s = m['segments'][n['track']][n['seg'] - 1]
            n['ms'] = s['ms']; n['mus_offset'] = s['mus_offset']
    entry = []
    for e in range(m['events_n']):
        row = []
        for c in range(m['columns']):
            a = m['actions'][m['events'][0][e][c]]
            st = pf.PathState(m)
            tgt = st.resolve(-1, a['target'], var) if a['target'] >= 0 else -1
            row.append(dict(action=a['id'], target=a['target'], node=tgt, immediate=a['immediate'],
                            stop=a['target'] < 0 and bool(a['flags'] & 0x02), noop=not (a['flags'] & 0x03)))
        entry.append(row)
    order, loop_at = pf.default_path(m, var)
    out = dict(m)
    out['event_entry'] = entry
    # what event 10 (race finish) plays: its entry node followed to the end of the song
    fin = []
    st = pf.PathState(m)
    a = m['actions'][m['events'][0][min(10, m['events_n'] - 1)][0]]
    n = st.resolve(-1, a['target'], var) if a['target'] >= 0 else -1
    while n >= 0 and n not in fin:
        fin.append(n); n = st.next_node(n, var)
    out['finish_path'] = dict(event=min(10, m['events_n'] - 1), nodes=fin, ends=n < 0,
                              ms=sum(m['nodes'][i]['ms'] for i in fin))
    out['default_path'] = dict(var=var, nodes=order, loop_at=loop_at,
                               ms=sum(m['nodes'][i]['ms'] for i in order),
                               loop_ms=sum(m['nodes'][i]['ms'] for i in order[loop_at:]) if loop_at is not None else 0)
    return out


def main():
    a = [x for x in sys.argv[1:] if not x.startswith('--')]
    var = 80
    if '--var' in sys.argv:
        var = int(sys.argv[sys.argv.index('--var') + 1]); a = [x for x in a if x != str(var)]
    m = pf.parse_mpf(open(a[0], 'rb').read())
    j = build(m, var)
    json.dump(j, open(a[1], 'w'), indent=1)
    print(a[0], len(m['nodes']), 'nodes', len(m['segments'][0]), 'segments; default path',
          len(j['default_path']['nodes']), 'nodes', j['default_path']['ms'] / 1000.0, 's, loops at',
          j['default_path']['loop_at'])


if __name__ == '__main__':
    main()
