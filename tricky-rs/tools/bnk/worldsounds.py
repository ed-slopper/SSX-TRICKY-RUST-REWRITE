#!/usr/bin/env python3
"""Build the world-sound map for tricky-rs from an extracted AUDIO.BIG and BANKS.INF, and write each
course's placed sounds.

usage: worldsounds.py AUDIO_BIG_OUT BANKS.INF [LEVELS_DIR]
  AUDIO_BIG_OUT  directory written by `levelaudio.py AUDIO.BIG AUDIO_BIG_OUT`
  BANKS.INF      DATA/CONFIG/BANKS.INF
  LEVELS_DIR     e.g. lvl/all: every <track>/ with Instances.json gets <track>/audio/levelaudio.json
                 (placements merged into an existing levelaudio.json, e.g. gari's course music)
Writes AUDIO_BIG_OUT/worldsounds.json.  File names in it are relative to AUDIO_BIG_OUT.
"""
import sys, os, re, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import levelaudio as L

# remake track dir -> BANKS.INF section (the game picks the section whose name appears in the track
# file name; AUDIO.INF / CROWD.INF use the same names)
TRACK_SECTION = {'gari': 'GARI', 'snowdream': 'SNOW', 'elysium': 'ELYSIUM', 'mesablanca': 'MESA',
                 'merqury': 'MERQUERY', 'megaplex': 'MEGAPLEX', 'aloha': 'ALOHA', 'alaska': 'ALASKA',
                 'untracked': 'UNTRACKED', 'pipedream': 'PIPE', 'trick': 'TRICK'}

def parse_inf(path):
    secs, cur = {}, None
    for line in open(path, encoding='latin-1'):
        line = line.split('#', 1)[0].strip()
        if not line: continue
        m = re.match(r'\[(.+)\]$', line)
        if m: cur = secs.setdefault(m.group(1), {}); continue
        m = re.match(r'(\w+)\s*=\s*"?([^"]*)"?', line)
        if m and cur is not None:
            cur.setdefault(m.group(1), []).append(m.group(2))
    return secs

def stem(f): return os.path.splitext(os.path.basename(f.replace('\\', '/')))[0].strip()

def bank_index(root, name):
    """program -> {file, loop_file, meta} for an extracted bank dir (case-insensitive name)."""
    dirs = {d.lower(): d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d))}
    d = dirs.get(stem(name).lower())
    if not d: return None, {}
    meta = json.load(open(os.path.join(root, d, d + '.json')))
    out = {}
    for p, m in meta.items():
        e = dict(file=f'{d}/{d}_{p}.wav', loop=bool(m.get('loop')))
        if e['loop']: e['loop_file'] = f'{d}/{d}_{p}_loop.wav'
        if m.get('mix'): e['mix_file'] = f"{d}/{m['mix']}"
        for k in ('root', 'bend', 'vol', 'detune', 'rate', 'chans', 'pitch_rand', 'vol_rand'):
            if k in m: e[k] = m[k]
        if m.get('layers'): e['layers'] = len(m['layers'])
        out[int(p)] = e
    return d, out

def script_sounds(level_dir, n_inst):
    """op 8 SoundPlay per instance: walk every instance's EffectSlot headers (op 7 switches to the
    target instance, ops 21/26 call Functions in the same instance context)."""
    try:
        lg = json.load(open(os.path.join(level_dir, 'SSFLogic.json')))
    except FileNotFoundError:
        return []
    heads = [h.get('Effects', []) for h in lg.get('EffectHeaders', [])]
    funcs = [f.get('Effects', []) for f in lg.get('Functions', [])]
    ins = json.load(open(os.path.join(level_dir, 'Instances.json')))['Instances']
    keys = ['PersistantEffectSlot', 'CollisionEffectSlot', 'Slot3', 'Slot4', 'EffectTriggerSlot', 'Slot6', 'Slot7']
    found = {}
    def walk(ops, inst, src, slot, depth, seen):
        if depth > 12: return
        for op in ops:
            t = op.get('MainType')
            if t == 8:
                found.setdefault((inst, op['SoundPlay']), dict(instance=inst, program=op['SoundPlay'],
                                 triggered_by=src, slot=slot))
            elif t == 7 and op.get('Instance'):
                ti, h = op['Instance'].get('InstanceIndex', -1), op['Instance'].get('EffectIndex', -1)
                if 0 <= h < len(heads) and (ti, h) not in seen:
                    walk(heads[h], ti if 0 <= ti < n_inst else inst, src, slot, depth + 1, seen | {(ti, h)})
            elif t in (21, 26):
                fi = op.get('FunctionRunIndex', -1)
                if isinstance(fi, int) and 0 <= fi < len(funcs) and ('f', fi) not in seen:
                    walk(funcs[fi], inst, src, slot, depth + 1, seen | {('f', fi)})
    slots = lg.get('EffectSlots', [])
    for i, x in enumerate(ins):
        s = x.get('EffectSlotIndex', -1)
        if not (0 <= s < len(slots)): continue
        for k in keys:
            h = slots[s].get(k, -1)
            if 0 <= h < len(heads):
                walk(heads[h], i, i, k, 0, {(i, h)})
    out = []
    for e in found.values():
        if 0 <= e['instance'] < n_inst:
            e['name'] = ins[e['instance']]['InstanceName']; e['pos'] = ins[e['instance']]['Location']
        out.append(e)
    return sorted(out, key=lambda e: (e['instance'], e['program']))

def main(root, banks_inf, levels=None):
    inf = parse_inf(banks_inf)
    tracks, bank_programs, bank_loops, missing = {}, {}, {}, {}
    for sec, kv in inf.items():
        if 'BANK' not in kv: continue
        t = {k.lower(): kv[k][0] for k in ('MAIN', 'BOARD', 'BANK', 'CROWD', 'TRICKY') if k in kv}
        t['swap_count'] = len(kv.get('SWAP', []))
        tracks[sec] = t
        d, idx = bank_index(root, t['bank'])
        t['bank_dir'] = d
        bank_programs[sec] = {str(p): e.get('mix_file', e['file']) for p, e in sorted(idx.items())}
        bank_loops[sec] = {str(p): e['loop_file'] for p, e in sorted(idx.items()) if e['loop']}
        for p in idx: idx[p].pop('file', None)
    crowd_dir, crowd = bank_index(root, 'Crowd.bnk')
    ws = {}
    for i, e in L.world_sound_table().items():
        if e['bank'] == 'SWAP' and 'file' in e and e['file'].endswith('.bnk'):
            d, idx = bank_index(root, e['file'])
            alt = bank_index(root, e['alt_file'])[0] if 'alt_file' in e else None
            if d is None:
                ws[str(i)] = dict(kind='swap', bank=stem(e['file']) + '.bnk', missing=True); missing[i] = e['file']; continue
            ws[str(i)] = dict(kind='swap', bank=stem(e['file']) + '.bnk', **idx[0])
            if alt: ws[str(i)]['alt_file'] = f'{alt}/{alt}_0.wav'
        elif i == 0x66:
            ws[str(i)] = dict(kind='speech', note='random entry from "speech" (chant.inf lists: char chants when the player leads, else 50/50 or general)')
        elif e['bank'] == 'CROWD':
            ws[str(i)] = dict(kind='crowd', bank='Crowd.bnk', program=e['program'], **crowd.get(e['program'], {}))
        else:
            ws[str(i)] = dict(kind='bank', program=e['program'], note='file per track: bank_programs[section][program]')
        if e.get('needs_hit'): ws[str(i)]['needs_hit'] = True
    speech = {'general': [], 'character': {}}
    for d in sorted(os.listdir(root)):
        m = re.match(r'C_(\w+?)_?(\d+)$', d)
        if m and os.path.isdir(os.path.join(root, d)):
            f = f'{d}/{d}_0.wav'
            if m.group(1) == 'Gen': speech['general'].append(f)
            else: speech['character'].setdefault(m.group(1), []).append(f)
    doc = dict(tracks=tracks, track_dirs=TRACK_SECTION, world_sounds=ws, bank_programs=bank_programs,
               bank_loops=bank_loops,
               speech=speech['general'] + [f for v in speech['character'].values() for f in v],
               speech_groups=speech, missing_swap_banks={str(k): v for k, v in missing.items()},
               notes=dict(units='positions in game units (cm), z up; remake = (x, z, -y) * 0.01',
                          bank='kind "bank" = in-game bank slot 2 = BANKS.INF BANK of the track',
                          collision='Instances[].Sounds.CollisonSound -> world_sounds id',
                          op8='SSFLogic op 8 SoundPlay -> bank_programs[section][SoundPlay] directly'))
    json.dump(doc, open(os.path.join(root, 'worldsounds.json'), 'w'), indent=1)
    print('worldsounds.json:', len(ws), 'ids,', len(tracks), 'tracks, missing swap banks', sorted(missing.values()))
    if not levels: return
    for tdir in sorted(os.listdir(levels)):
        ld = os.path.join(levels, tdir)
        if not os.path.isfile(os.path.join(ld, 'Instances.json')): continue
        sec = TRACK_SECTION.get(tdir)
        progs = bank_programs.get(sec, {})
        pl = L.placements(ld)
        n_inst = len(json.load(open(os.path.join(ld, 'Instances.json')))['Instances'])
        def res(sid):
            w = ws.get(str(sid))
            if not w: return None
            if w['kind'] == 'bank': return progs.get(str(w['program']))
            return w.get('loop_file') or w.get('file')
        for e in pl['emitters']:
            e['id'] = e.pop('sound'); e['wav'] = res(e['id']); e.pop('target', None)
        ins = json.load(open(os.path.join(ld, 'Instances.json')))['Instances']
        coll = {}
        for i, x in enumerate(ins):
            s = x.get('Sounds') or {}
            if x.get('IncludeSound') and s.get('CollisonSound'):
                coll[str(i)] = s['CollisonSound']
        ops = script_sounds(ld, n_inst)
        for o in ops: o['wav'] = progs.get(str(o['program']))
        cids = sorted(set(coll.values()))
        out = dict(section=sec, bank=tracks.get(sec, {}).get('bank'), emitters=pl['emitters'],
                   collision=coll,
                   collision_ids={str(c): dict(world_sounds=ws.get(str(c)), wav=res(c)) for c in cids},
                   script_sounds=ops)
        od = os.path.join(ld, 'audio'); os.makedirs(od, exist_ok=True)
        fp = os.path.join(od, 'levelaudio.json')
        doc2 = json.load(open(fp)) if os.path.exists(fp) else dict(source=None, entries=[])
        doc2['placements'] = out
        doc2['worldsounds'] = os.path.relpath(os.path.join(root, 'worldsounds.json'), od)
        json.dump(doc2, open(fp, 'w'), indent=1)
        unres = [e['id'] for e in out['emitters'] if not e['wav']] + [c for c in cids if not res(c)] + \
                [o['program'] for o in ops if not o['wav']]
        print(f'{tdir:11s} {sec:9s} emitters {len(out["emitters"]):3d} collision {len(coll):4d} ids {cids} '
              f'op8 {len(ops):3d} {sorted({o["program"] for o in ops})} unresolved {sorted(set(unres))}')

if __name__ == '__main__':
    if len(sys.argv) not in (3, 4): sys.exit(__doc__)
    main(*sys.argv[1:4])
