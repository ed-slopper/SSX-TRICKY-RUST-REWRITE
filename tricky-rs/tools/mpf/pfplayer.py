#!/usr/bin/env python3
"""Reference Pathfinder player (the algorithm the Rust remake should implement), driven by graph.json.

usage: pfplayer.py OUT_DIR_OF_mpf2json SCRIPT OUT.wav
  SCRIPT: comma list of  t:event  (post event at t seconds)  or  t:v=N  (set path variable), e.g.
          "0:0,0:v=80,40:1,55:2,90:v=20,120:10"
Prints the node order with start times.  Model of PF_Update/PF_AdvanceNode/PF_ApplyEvent
(SLUS_203.26 0x2c0488 / 0x2bfbb8 / 0x2bf508) for a single track opened with 500 ms lookahead.
"""
import sys, os, json, wave
import numpy as np


class Graph:
    def __init__(self, d):
        self.g = json.load(open(os.path.join(d, 'graph.json')))
        self.nodes = self.g['nodes']
        self.dir = d
        self._pcm = {}

    def pcm(self, n):
        if n not in self._pcm:
            w = wave.open(os.path.join(self.dir, self.nodes[n]['wav']))
            self._pcm[n] = np.frombuffer(w.readframes(w.getnframes()), '<i2').reshape(-1, w.getnchannels())
        return self._pcm[n]


class Player:
    def __init__(self, graph):
        self.G = graph
        self.var = 0
        self.loop_node, self.loop_cnt = -1, 0
        self.queue = []          # event queue [(prio, seq, event)]
        self.seq = 0
        self.play = []           # queued audio: [node, pos]  (play[0] is sounding)
        self.last = -1           # last queued node = lib's track+0x16 (used for event column + routers)
        self.la = int(graph.g['rate'] * graph.g['lookahead_ms'] / 1000)
        self.log = []
        self.t = 0

    # ---------------------------------------------------------------- graph walk (PF_NextNodeByVar etc.)
    def branch(self, n, v):
        nd = self.G.nodes[n]
        if self.loop_node == n:
            v = self.loop_cnt & 0x7f
            if nd['loop']:
                self.loop_cnt = (self.loop_cnt - 1) & 0xff
                if self.loop_cnt == 0xff:
                    self.loop_node = -1
        for b in nd['branches']:
            if b['lo'] <= v <= b['hi']:
                return b['dest']
        return -1

    def route(self, cur, x):
        if cur >= 0 and self.G.nodes[cur]['router']:
            for e in self.G.g['routers'][self.G.nodes[cur]['router'] - 1]:
                if x == e['src']:
                    x = e['dst']
        return x

    def resolve(self, cur, x):
        x = self.route(cur, x)
        while x >= 0 and self.G.nodes[x]['seg'] < 1:
            nd = self.G.nodes[x]
            if nd['seg'] == -1 and nd['loop'] and x != self.loop_node:
                self.loop_cnt, self.loop_node = nd['loop'], x
            x = self.route(cur, self.branch(x, self.var))
        return self.route(cur, x)

    # ---------------------------------------------------------------- inputs
    def post(self, e, prio=0):
        self.queue.append((-prio, self.seq, e)); self.seq += 1; self.queue.sort()

    def set_var(self, v):
        self.var = max(0, min(127, int(v)))

    def action(self, e):
        col = self.G.nodes[self.last]['section'] if self.last >= 0 else 0
        ev = self.G.g['events'][e]
        return self.G.g['actions'][ev['per_column'][max(col, 0)]]

    # ---------------------------------------------------------------- per audio block
    def _enqueue(self, n):
        self.last = n
        if n >= 0:
            self.play.append([n, 0])
            self.log.append((n, self.t + sum(len(self.G.pcm(p)) - q for p, q in self.play[:-1])))

    def _remaining(self):
        return sum(len(self.G.pcm(n)) - p for n, p in self.play)

    def render(self, nframes):
        # 1. immediate event at the queue head: cut now (flush queued audio)
        if self.queue:
            e = self.queue[0][2]
            a = self.action(e)
            if a['immediate']:
                self.queue.pop(0)
                if a['transition'] and a['target'] >= 0:
                    n = self.resolve(self.last, a['target'])
                    self.play = []; self._enqueue(n)
                elif a['stop']:
                    self.play = []; self.last = -1
        # 2. top up: decide the successor when <= lookahead of queued audio is left
        while self.last >= 0 and self._remaining() <= self.la:
            nxt = None
            if self.queue:
                e = self.queue.pop(0)[2]
                a = self.action(e)
                if a['transition'] and a['target'] >= 0:
                    nxt = self.resolve(self.last, a['target'])
                elif a['stop']:
                    nxt = -1
                # no-op action: consumed, fall through to the normal successor
            if nxt is None:
                nxt = self.resolve(self.last, self.branch(self.last, self.var))
            self._enqueue(nxt)
        # 3. mix (gapless concatenation)
        out = np.zeros((nframes, 2), np.int16); o = 0
        while o < nframes and self.play:
            n, p = self.play[0]; a = self.G.pcm(n)
            k = min(nframes - o, len(a) - p)
            out[o:o + k] = a[p:p + k]; o += k; self.play[0][1] += k
            if self.play[0][1] >= len(a):
                self.play.pop(0)
        self.t += nframes
        return out


def main():
    d, script, outp = sys.argv[1:4]
    G = Graph(d); P = Player(G); rate = G.g['rate']
    cmds = []
    for c in script.split(','):
        t, x = c.split(':'); cmds.append((float(t), x))
    end = max(t for t, _ in cmds) + 60
    blk = rate // 60; out = []; i = 0
    while P.t < end * rate:
        while i < len(cmds) and cmds[i][0] * rate <= P.t:
            x = cmds[i][1]
            if x.startswith('v='):
                P.set_var(int(x[2:]))
            else:
                P.post(int(x), 1 if x == '0' else 0)
            i += 1
        out.append(P.render(blk))
        if not P.play and P.last < 0 and i >= len(cmds) and P.t > 0:
            break
    pcm = np.concatenate(out)
    w = wave.open(outp, 'wb'); w.setnchannels(2); w.setsampwidth(2); w.setframerate(rate); w.writeframes(pcm.tobytes()); w.close()
    for n, t in P.log:
        print('%7.2fs node %3d sec %d' % (t / rate, n, G.nodes[n]['section']))
    print('total %.1f s' % (len(pcm) / rate))


if __name__ == '__main__':
    main()
