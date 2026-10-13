//! The replay's Huffman encoder (board row G12c): `Compress_Block` 0x243a60, `Huff_Encode` 0x2438c0 and the
//! functions under them (0x241e48..0x243b88), 1:1 on a byte copy of the game's 0x31ec-byte context, checked in the
//! runner (`tricky-game/tests/huffenc.rs`). The streams it writes are what `compress::huff_decode_block` reads.
//!
//! The context comes from `Compress_Alloc` and is not cleared; the encoder writes every word it reads except the
//! second clue (+0x2dd0), which keeps the old clue word, so `Encoder::new` takes the starting bytes.

/// The context's fields (byte offsets)
pub mod at {
    /// 256 bytes: a byte value already placed (`Huff_WriteTable`)
    pub const USED: usize = 0;
    /// u32 × 0x300: the symbols' counts (0x100), the deltas' (0x100), the run lengths' (0x100, at +0x900)
    pub const COUNT: usize = 0x100;
    pub const RUNS: usize = 0x900;
    /// u32 × 17: codes per length
    pub const LENS: usize = 0xd00;
    /// u32 × 0xfc: the bits and base of a number under 0xfc (`Huff_InitRunCosts`)
    pub const NUM_BITS: usize = 0xd44;
    pub const NUM_BASE: usize = 0x1134;
    /// u32 per node (0x100..): its children
    pub const LEFT: usize = 0x1524;
    pub const RIGHT: usize = 0x1d44;
    /// u32 × 256: each symbol's code length and code
    pub const DEPTH: usize = 0x2564;
    pub const CODE: usize = 0x2964;
    /// u32 × 17: 2^n − 1
    pub const MASK: usize = 0x2d64;
    pub const BITS: usize = 0x2da8;
    pub const ACC: usize = 0x2dac;
    pub const SRC: usize = 0x2db0;
    pub const END: usize = 0x2db4;
    pub const SIZE: usize = 0x2db8;
    pub const SUM: usize = 0x2dbc;
    pub const MAX_LEN: usize = 0x2dc0;
    pub const SYMBOLS: usize = 0x2dc4;
    pub const LIMITED: usize = 0x2dc8;
    /// the escape (clue) symbol and its run of free values; the second run (the near deltas)
    pub const CLUE: usize = 0x2dcc;
    pub const CLUE2: usize = 0x2dd0;
    pub const CLUES: usize = 0x2dd4;
    pub const NEARS: usize = 0x2dd8;
    pub const NEAR_LO: usize = 0x2ddc;
    pub const NEAR_HI: usize = 0x2de0;
    pub const OUT_BYTES: usize = 0x2de4;
    pub const SIZE2: usize = 0x2de8;
    /// u32 × 256: the symbols by code length
    pub const ORDER: usize = 0x2dec;
    pub const LEN: usize = 0x31ec;
}
use at::*;

/// `Delta_Encode` 0x241e48(src, dst, n): each byte less the one before it (in place when dst is src)
pub fn delta_encode(buf: &mut [u8]) {
    let mut last = 0u8;
    for b in buf.iter_mut() {
        let c = *b;
        *b = c.wrapping_sub(last);
        last = c;
    }
}

/// The output (the game's two words: the buffer and the bytes written)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out {
    pub buf: Vec<u8>,
    pub pos: u32,
}

impl Out {
    fn put(&mut self, b: u8) {
        let at = self.pos as usize;
        if self.buf.len() <= at {
            self.buf.resize(at + 1, 0);
        }
        self.buf[at] = b;
        self.pos = self.pos.wrapping_add(1);
    }
}

/// The encoder's context and the bytes it encodes (`src_addr`: the address +0x2db0 / +0x2db4 hold in the game)
pub struct Encoder<'a> {
    pub ctx: Vec<u8>,
    /// The first pass's counts, as `Huff_Analyze` keeps them on its stack for the second
    pub saved: Vec<u32>,
    pub src: &'a [u8],
    pub src_addr: u32,
}

impl<'a> Encoder<'a> {
    /// A context with these starting bytes (0x31ec; what `Compress_Alloc` handed out)
    pub fn new(ctx: Vec<u8>, src: &'a [u8], src_addr: u32) -> Encoder<'a> {
        assert_eq!(ctx.len(), LEN);
        Encoder { ctx, saved: vec![], src, src_addr }
    }

    pub fn g(&self, off: usize) -> u32 {
        assert!(off + 4 <= LEN, "the game reads past its context (+{off:#x})");
        u32::from_le_bytes(self.ctx[off..off + 4].try_into().unwrap())
    }
    pub fn s(&mut self, off: usize, v: u32) {
        assert!(off + 4 <= LEN, "the game writes past its context (+{off:#x})");
        self.ctx[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn cnt(&self, i: u32) -> u32 {
        self.g(COUNT + 4 * i as usize)
    }
    fn set_cnt(&mut self, i: u32, v: u32) {
        self.s(COUNT + 4 * i as usize, v)
    }
    fn depth(&self, i: u32) -> u32 {
        self.g(DEPTH + 4 * i as usize)
    }
    fn set_depth(&mut self, i: u32, v: u32) {
        self.s(DEPTH + 4 * i as usize, v)
    }
    fn code(&self, i: u32) -> u32 {
        self.g(CODE + 4 * i as usize)
    }
    fn byte(&self, p: u32) -> u32 {
        self.src[p.wrapping_sub(self.src_addr) as usize] as u32
    }

    /// `Huff_WriteBits` 0x241e88(out, value, n): more than 16 bits as the high part first; the bits gather from
    /// bit 23 of +0x2dac down and each whole byte goes out (+0x2de4 counts them)
    pub fn write_bits(&mut self, out: &mut Out, v: u32, mut n: u32) {
        while n > 16 {
            self.write_bits(out, v >> 16, n - 16);
            n = 16;
        }
        let bits = self.g(BITS).wrapping_add(n);
        self.s(BITS, bits);
        let add = (v & self.g(MASK + 4 * n as usize)).wrapping_shl(24u32.wrapping_sub(bits) & 31);
        self.s(ACC, self.g(ACC).wrapping_add(add));
        let mut bits = bits;
        while bits > 7 {
            out.put((self.g(ACC) >> 16) as u8);
            bits = self.g(BITS).wrapping_sub(8);
            self.s(ACC, self.g(ACC) << 8);
            self.s(OUT_BYTES, self.g(OUT_BYTES).wrapping_add(1));
            self.s(BITS, bits);
        }
    }

    /// `Huff_AssignDepths` 0x241f78(node, depth): inner nodes (0x100..) one deeper for both children
    pub fn assign_depths(&mut self, mut node: u32, mut d: u32) {
        while node >= 0x100 {
            d = d.wrapping_add(1);
            let l = self.g(LEFT + 4 * node as usize);
            self.assign_depths(l, d);
            node = self.g(RIGHT + 4 * node as usize);
        }
        self.set_depth(node, d);
    }

    /// `Huff_BuildTree` 0x241ff8: every depth 99, then the used symbols' tree (the two lightest joined each time,
    /// the last entry moved into the lighter's slot); +0x2dc4 the used count
    pub fn build_tree(&mut self) {
        let mut w = vec![0u32; 0x102];
        let mut syms = vec![None::<u32>; 0x102];
        let mut n = 1usize;
        for u in 0..0x100u32 {
            self.set_depth(u, 99);
            let c = self.cnt(u);
            if c != 0 {
                w[n] = c;
                syms[n] = Some(u);
                n += 1;
            }
        }
        let mut top = n - 1;
        self.s(SYMBOLS, top as u32);
        let sym = |s: &Vec<Option<u32>>, i: usize| s[i].expect("Huff_BuildTree: the game reads an unset stack word");
        if n < 3 {
            self.assign_depths(sym(&syms, top), 1);
            return;
        }
        let mut node = 0x100u32;
        let mut k = n;
        loop {
            k -= 2;
            let (mut lo_w, mut lo_i, mut hi_w, mut hi_i) = (w[k], k, w[top], top);
            if w[top] < w[k] {
                (lo_w, lo_i, hi_w, hi_i) = (w[top], top, w[k], k);
            }
            if k != 0 {
                loop {
                    k -= 1;
                    while hi_w < w[k] {
                        k -= 1;
                    }
                    if k == 0 {
                        break;
                    }
                    let v = w[k];
                    hi_i = k;
                    if v <= lo_w {
                        hi_i = lo_i;
                        hi_w = lo_w;
                        lo_i = k;
                        lo_w = v;
                    } else {
                        hi_w = v;
                    }
                }
            }
            let (l, r) = (sym(&syms, hi_i), sym(&syms, lo_i));
            w[hi_i] = hi_w.wrapping_add(lo_w);
            syms[hi_i] = Some(node);
            self.s(LEFT + 4 * node as usize, l);
            w[lo_i] = w[top];
            self.s(RIGHT + 4 * node as usize, r);
            syms[lo_i] = syms[top];
            if top < 3 {
                break;
            }
            k = top;
            top -= 1;
            node += 1;
        }
        self.assign_depths(node, 0);
    }

    /// `Huff_RunCost` 0x242210(run, k): the fewest bits for a run with the clue's run symbols 1..k (or the clue
    /// and its number)
    pub fn run_cost(&self, n: u32, k: u32) -> i32 {
        let clue = self.g(CLUE);
        if k == 0 {
            if n == 0 {
                0
            } else if n < 0xfc {
                (self.depth(clue) as i32).wrapping_add((self.g(NUM_BITS + 4 * n as usize) as i32).wrapping_mul(2)).wrapping_add(3)
            } else {
                0x14
            }
        } else {
            let mut c = self.run_cost(n, k - 1);
            if self.cnt(clue.wrapping_add(k)) != 0 {
                let (q, r) = ((n as i32).wrapping_div(k as i32), (n as i32).wrapping_rem(k as i32));
                let alt = self
                    .run_cost(r as u32, k - 1)
                    .wrapping_add((self.depth(clue.wrapping_add(k)) as i32).wrapping_mul(q));
                if alt < c {
                    c = alt;
                }
            }
            c
        }
    }

    /// `Huff_WriteVarNum` 0x242320: a 1 after b zeros, then n less its base in b + 2 bits
    pub fn write_var_num(&mut self, out: &mut Out, n: u32) {
        let (b, base) = if n < 0xfc {
            (self.g(NUM_BITS + 4 * n as usize), self.g(NUM_BASE + 4 * n as usize))
        } else {
            const T: [(u32, u32, u32); 12] = [
                (0x1fc, 6, 0xfc),
                (0x3fc, 7, 0x1fc),
                (0x7fc, 8, 0x3fc),
                (0xffc, 9, 0x7fc),
                (0x1ffc, 10, 0xffc),
                (0x3ffc, 11, 0x1ffc),
                (0x7ffc, 12, 0x3ffc),
                (0xfffc, 13, 0x7ffc),
                (0x1fffc, 14, 0xfffc),
                (0x3fffc, 15, 0x1fffc),
                (0x80000, 16, 0x3fffc),
                (0x100000, 17, 0x80000),
            ];
            T.iter().find(|t| n < t.0).map_or((18, 0x100000), |t| (t.1, t.2))
        };
        self.write_bits(out, 1, b + 1);
        self.write_bits(out, n.wrapping_sub(base), b + 2);
    }

    /// `Huff_WriteClueSymbol` 0x2424d0: the clue, 0, then a 0 bit and the byte
    pub fn write_clue_symbol(&mut self, out: &mut Out, v: u32) {
        let c = self.g(CLUE);
        self.write_bits(out, self.code(c), self.depth(c));
        self.write_var_num(out, 0);
        self.write_bits(out, v, 9);
    }

    /// `Huff_WriteSymbol` 0x242548: a symbol's code; the clue's own byte value escaped
    pub fn write_symbol(&mut self, out: &mut Out, s: u32) {
        if s == self.g(CLUE) {
            self.write_clue_symbol(out, s);
            return;
        }
        self.write_bits(out, self.code(s), self.depth(s));
    }

    /// `Huff_InitRunCosts` 0x242578: the numbers under 0xfc: 0..3 none, then 1..5 bits from 4, 0xc, 0x1c, 0x3c,
    /// 0x7c
    pub fn init_run_costs(&mut self) {
        for i in 0..0xfcu32 {
            let (b, base) = match i {
                0..4 => (0, 0),
                4..0xc => (1, 4),
                0xc..0x1c => (2, 0xc),
                0x1c..0x3c => (3, 0x1c),
                0x3c..0x7c => (4, 0x3c),
                _ => (5, 0x7c),
            };
            self.s(NUM_BITS + 4 * i as usize, b);
            self.s(NUM_BASE + 4 * i as usize, base);
        }
    }

    /// A repeat run's cheapest form (`Huff_Analyze`'s second pass and `Huff_WriteTable` share it): the run's
    /// length in the clue's run symbols (`plain`, 32000 when they cannot make it) and as the clue and a number
    /// (`clue_cost`), against `lit`, the byte written `run` times; `counts` are the counts the choice reads
    fn run_choice(&self, counts: &dyn Fn(u32) -> u32, first: u32, run: u32, clues: u32) -> (bool, u32, u32, u32) {
        let lit = run.wrapping_mul(self.depth(first)) as i32 as i64 as u64;
        let mut plain = 32000u64;
        let mut clue_cost = 32000u64;
        let worse;
        if clues == 0 {
            worse = 32000 < lit;
        } else {
            let c = self.g(CLUE);
            if counts(c) != 0 {
                clue_cost = 0x14;
                if run < 0xfc {
                    clue_cost = (self.depth(c).wrapping_add(self.g(NUM_BITS + 4 * run as usize).wrapping_mul(2)).wrapping_add(3)) as i32 as i64 as u64;
                }
            }
            let mut w = clue_cost < lit;
            if clues > 1 {
                let mut p = 0u32;
                let mut r = run;
                let mut j = clues;
                loop {
                    j -= 1;
                    if j == 0 {
                        break;
                    }
                    if counts(c.wrapping_add(j)) != 0 {
                        let q = (r as i32).wrapping_div(j as i32);
                        p = p.wrapping_add((q.wrapping_mul(self.depth(c.wrapping_add(j)) as i32)) as u32);
                        r = r.wrapping_sub(q.wrapping_mul(j as i32) as u32);
                    }
                }
                plain = p as i32 as i64 as u64;
                w = clue_cost < lit;
                if r != 0 {
                    plain = 32000;
                }
            }
            worse = w;
        }
        (worse || plain < lit, plain as u32, clue_cost as u32, lit as u32)
    }

    /// `Huff_Analyze` 0x2426f8(flags, longest code): counts the symbols, deltas and repeat runs; picks the clue (the
    /// longest run of unused byte values, its following values standing for repeat runs) and the near deltas;
    /// builds the tree; drops run symbols that do not pay; counts again as `Huff_WriteTable` will write; builds the
    /// tree again, lengthens codes past the longest; the codes in canonical order
    pub fn analyze(&mut self, flags: u32, max_len: u32) {
        let nf = !flags;
        let eof = flags & 0x20;
        let limit = max_len < 99;
        let mut c0 = 0u32;
        for i in 0..0x300 {
            self.set_cnt(i, 0);
        }
        let (start, end) = (self.g(SRC), self.g(END));
        // the first count
        let mut sum = 0u32;
        let mut prev = 0x100u32;
        let mut p = start;
        while p < end {
            let mut cur = self.byte(p);
            let mut nxt = p.wrapping_add(1);
            sum = sum.wrapping_add(cur);
            if cur == prev {
                let lim = if end < p.wrapping_add(0x7531) { end } else { p.wrapping_add(0x7531) };
                let mut run = 0u32;
                let mut last = cur;
                if nxt < lim {
                    loop {
                        last = self.byte(nxt);
                        run += 1;
                        nxt = nxt.wrapping_add(1);
                        sum = sum.wrapping_add(last);
                        if last != prev || lim <= nxt {
                            break;
                        }
                    }
                }
                cur = last;
                if run < 0xff {
                    self.set_cnt(run + 0x200, self.cnt(run + 0x200).wrapping_add(1));
                } else {
                    self.s(RUNS, self.g(RUNS).wrapping_add(1));
                }
            }
            self.set_cnt(cur, self.cnt(cur).wrapping_add(1));
            let d = (cur.wrapping_sub(prev) & 0xff) + 0x100;
            self.set_cnt(d, self.cnt(d).wrapping_add(1));
            p = nxt;
            prev = cur;
        }
        self.s(SUM, sum);
        if self.g(RUNS) == 0 {
            self.s(RUNS, 1);
        }
        // the clue: the longest run of unused values (the second longest the near deltas')
        self.s(CLUES, 0);
        self.s(NEARS, 0);
        let mut rare = 0u32;
        let mut u = 0u32;
        loop {
            let mut zr = 0u32;
            if self.cnt(u) < self.cnt(rare) {
                rare = u;
            }
            let mut v = u;
            if self.cnt(u) == 0 {
                loop {
                    v += 1;
                    zr += 1;
                    if self.cnt(v) != 0 || v >= 0x100 {
                        break;
                    }
                }
            }
            if zr >= self.g(NEARS) {
                self.s(CLUE2, u);
                self.s(NEARS, zr);
                if self.g(CLUES) <= zr {
                    self.s(NEARS, self.g(CLUES));
                    self.s(CLUE2, self.g(CLUE));
                    self.s(CLUE, u);
                    self.s(CLUES, zr);
                }
            }
            u = v + 1;
            if u > 0xff {
                break;
            }
        }
        if eof != 0 && self.g(CLUES) == 0 {
            self.s(CLUE, rare);
            self.s(CLUES, 1);
        }
        if nf & 2 != 0 {
            if self.g(CLUES) > 1 {
                self.s(CLUES, 1);
            }
            if nf & 1 != 0 {
                self.s(CLUES, 0);
            }
        }
        let mut nn = 0u32;
        if nf & 4 == 0 {
            nn = self.g(NEARS);
            let nc;
            if nn < 0xb {
                nc = self.g(CLUES);
            } else {
                let (a, b) = (self.g(CLUES), self.g(CLUE));
                self.s(CLUES, nn);
                self.s(CLUE, self.g(CLUE2));
                self.s(CLUE2, b);
                self.s(NEARS, a);
                nc = self.g(CLUES);
                nn = a;
            }
            let q = nn >> 2;
            if nc << 2 < nn {
                nn -= q;
                self.s(CLUES, q);
                self.s(NEARS, nn);
                self.s(CLUE, self.g(CLUE2).wrapping_add(nn));
            }
        } else {
            self.s(NEARS, 0);
        }
        let nc;
        if nn == 0 {
            nc = self.g(CLUES);
        } else {
            c0 = (self.g(SIZE2) as i32 / 0x19) as u32;
            let half = (nn as i32) / 2;
            let hi = (nn as i32).wrapping_sub(half);
            self.s(NEAR_LO, half.wrapping_neg() as u32);
            self.s(NEAR_HI, hi as u32);
            let mut base = self.g(CLUE2);
            if hi != 0 {
                let mut k = 1u32;
                loop {
                    let c = self.cnt(k + 0x100);
                    if c0 < c {
                        self.set_cnt(base.wrapping_add((k - 1) * 2), c);
                        base = self.g(CLUE2);
                    }
                    k += 1;
                    if k > self.g(NEAR_HI) {
                        break;
                    }
                }
            }
            let mut lo = self.g(NEAR_LO);
            if lo != 0 {
                let (mut k, mut j, mut p) = (1u32, 0u32, 0x8fcusize);
                loop {
                    let c = self.g(p);
                    if c0 < c {
                        self.set_cnt(base.wrapping_add(j).wrapping_add(1), c);
                        base = self.g(CLUE2);
                        lo = self.g(NEAR_LO);
                    }
                    k += 1;
                    j += 2;
                    p -= 4;
                    if k > lo.wrapping_neg() {
                        break;
                    }
                }
            }
            let nn2 = self.g(NEARS);
            let mut last = 0u32;
            let (mut nc0, clue) = (self.g(CLUES), self.g(CLUE));
            if nn2 != 0 {
                let mut k = 0u32;
                loop {
                    if self.cnt(base.wrapping_add(k)) != 0 {
                        last = k;
                    }
                    k += 1;
                    if k >= nn2 {
                        break;
                    }
                }
            }
            let off = nn2.wrapping_sub(last).wrapping_sub(1);
            self.s(NEARS, nn2.wrapping_sub(off));
            if clue == base.wrapping_add(nn2) {
                nc0 = nc0.wrapping_add(off);
                self.s(CLUE, clue.wrapping_sub(off));
                self.s(CLUES, nc0);
            }
            let h = -((self.g(NEARS) as i32) / 2);
            self.s(NEAR_LO, h as u32);
            self.s(NEAR_HI, self.g(NEARS).wrapping_add(h as u32));
            nc = nc0;
        }
        if nc != 0 {
            let mut k = 0u32;
            loop {
                let i = self.g(CLUE).wrapping_add(k);
                let v = self.g(RUNS + 4 * k as usize);
                k += 1;
                self.set_cnt(i, v);
                if k >= self.g(CLUES) {
                    break;
                }
            }
        }
        self.build_tree();
        // the run symbols that do not pay
        let mut nc = self.g(CLUES);
        if nc >= 2 {
            let mut k = 1u32;
            loop {
                let lim = (k - 1).min(8);
                let i = self.g(CLUE).wrapping_add(k);
                if self.cnt(i) != 0 {
                    let cost = self.run_cost(k, lim) as u32;
                    let i = self.g(CLUE).wrapping_add(k);
                    let d = self.depth(i);
                    if d < cost && k >> 1 <= self.cnt(i).wrapping_mul(cost - d) {
                        // keeps it
                    } else {
                        self.set_cnt(i, 0);
                    }
                    nc = self.g(CLUES);
                }
                k += 1;
                if nc <= k {
                    break;
                }
            }
        }
        // the second count, as the table is written
        // the counts kept on the stack, and after them the 256 words the game zeroes beside them (the near
        // deltas' symbols past 255 read those)
        let mut saved = vec![0u32; 0x200];
        for i in 0..0x100u32 {
            saved[i as usize] = self.cnt(i);
            self.set_cnt(i, 0);
            self.set_cnt(i + 0x100, 0);
            self.set_cnt(i + 0x200, 0);
        }
        self.saved = saved.clone();
        let sv = |i: u32| -> u32 {
            saved.get(i as usize).copied().expect("Huff_Analyze: the game reads past its saved counts")
        };
        let mut prev = 0x100u32;
        let mut p = start;
        while p < end {
            let mut cur = self.byte(p);
            let mut nxt = p.wrapping_add(1);
            if cur == prev {
                let lim = if end < p.wrapping_add(0x7531) { end } else { p.wrapping_add(0x7531) };
                let mut run = 0u32;
                let first = cur;
                let clues = self.g(CLUES);
                if nxt < lim {
                    loop {
                        cur = self.byte(nxt);
                        run += 1;
                        nxt = nxt.wrapping_add(1);
                        if cur != prev || lim <= nxt {
                            break;
                        }
                    }
                }
                let (pay, plain, clue_cost, _) = self.run_choice(&sv, first, run, clues);
                if pay {
                    if clue_cost < plain {
                        let c = self.g(CLUE);
                        self.set_cnt(c, self.cnt(c).wrapping_add(1));
                    } else {
                        let mut j = clues.wrapping_sub(1);
                        if j != 0 {
                            let mut r = run;
                            loop {
                                let c = self.g(CLUE);
                                if sv(c.wrapping_add(j)) != 0 {
                                    let q = (r as i32).wrapping_div(j as i32) as u32;
                                    self.set_cnt(c.wrapping_add(j), self.cnt(c.wrapping_add(j)).wrapping_add(q));
                                    r = r.wrapping_sub(q.wrapping_mul(j));
                                }
                                j -= 1;
                                if j == 0 {
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    self.set_cnt(first, self.cnt(first).wrapping_add(run));
                }
            }
            let d = cur.wrapping_sub(prev);
            let mut sym = cur;
            if self.g(NEARS) != 0 {
                let mut alt = 0u32;
                if (d as i32) <= self.g(NEAR_HI) as i32 && self.g(NEAR_LO) as i32 <= d as i32 {
                    let mut s = (d.wrapping_sub(1)).wrapping_mul(2).wrapping_add(self.g(CLUE2));
                    if cur < prev {
                        s = (prev - cur - 1).wrapping_mul(2).wrapping_add(self.g(CLUE2)).wrapping_add(1);
                    }
                    if c0 < sv(s) {
                        alt = (sv(cur) < 4) as u32 + (self.depth(s) < self.depth(cur)) as u32;
                        if self.depth(s) == self.depth(cur) {
                            alt += (self.cnt(cur) < self.cnt(s)) as u32;
                        }
                    }
                    if alt != 0 {
                        sym = s;
                    }
                }
            }
            self.set_cnt(sym, self.cnt(sym).wrapping_add(1));
            p = nxt;
            prev = cur;
        }
        if eof != 0 {
            let c = self.g(CLUE);
            self.set_cnt(c, self.cnt(c).wrapping_add(1));
        }
        self.build_tree();
        self.s(LIMITED, 0);
        // codes past the longest: the last two found at the deepest level (the game's registers start with
        // leftovers; with two or more symbols at the deepest level, as a tree has, they never show) up to one past
        // the deepest code still under the longest, which moves down with them; the other deepest one up one
        let (mut deep, mut second) = (0u32, 0u32);
        while limit {
            let mut maxd = 0u32;
            for i in 0..0x100u32 {
                if self.cnt(i) != 0 {
                    let d = self.depth(i);
                    if d >= maxd {
                        maxd = d;
                        second = deep;
                        deep = i;
                    }
                }
            }
            if maxd <= max_len {
                break;
            }
            let mut u = 0u32;
            if self.cnt(0) == 0 || max_len <= self.depth(0) {
                loop {
                    u += 1;
                    if u > 0xff {
                        break;
                    }
                    if self.cnt(u) != 0 && self.depth(u) < max_len {
                        break;
                    }
                }
            }
            let mut pick = u;
            if u < 0x100 {
                for v in u..0x100 {
                    if self.cnt(v) != 0 && self.depth(v) < max_len && self.depth(pick) < self.depth(v) {
                        pick = v;
                    }
                }
            }
            let t = self.depth(pick).wrapping_add(1);
            self.set_depth(pick, t);
            self.set_depth(deep, t);
            self.set_depth(second, self.depth(second).wrapping_sub(1));
            self.s(LIMITED, max_len);
        }
        if nf & 8 != 0 {
            for i in 0..0x100 {
                self.set_depth(i, 8);
            }
        }
        for l in 0..0x11 {
            self.s(LENS + 4 * l, 0);
        }
        for i in 0..0x100u32 {
            let d = self.depth(i);
            if d < 0x11 {
                self.s(LENS + 4 * d as usize, self.g(LENS + 4 * d as usize).wrapping_add(1));
            }
        }
        let mut longest = 0u32;
        let mut off = ORDER;
        for l in 1..0x11u32 {
            if self.g(LENS + 4 * l as usize) != 0 {
                for i in 0..0x100u32 {
                    if self.depth(i) == l {
                        self.s(off, i);
                        off += 4;
                    }
                    longest = l;
                }
            }
        }
        self.s(MAX_LEN, longest);
        let (mut code, mut len) = (0u32, 0u32);
        let n = self.g(SYMBOLS);
        let mut i = 0u32;
        while i < n {
            let sym = self.g(ORDER + 4 * i as usize);
            i += 1;
            while len < self.depth(sym) {
                len += 1;
                code <<= 1;
            }
            self.s(CODE + 4 * sym as usize, code);
            code = code.wrapping_add(1);
        }
    }

    /// `Huff_WriteTable` 0x243368: the clue, the codes per length, each code's byte as the free values stepped
    /// over, then the bytes: repeat runs as run symbols or the clue and a number, near deltas, else the codes; the
    /// end (the clue, 0, then 1 and seven zeros)
    pub fn write_table(&mut self, out: &mut Out) {
        self.write_bits(out, self.g(CLUE), 8);
        let m = self.g(MAX_LEN);
        let mut l = 1u32;
        while m != 0 {
            l += 1;
            self.write_var_num(out, self.g(LENS + 4 * (l - 1) as usize));
            if l > m {
                break;
            }
        }
        for i in 0..0x100 {
            self.ctx[USED + i] = 0;
        }
        let mut last = 0xffu32;
        let n = self.g(SYMBOLS);
        let mut i = 0u32;
        while i < n {
            i += 1;
            let mut skip = -1i32;
            loop {
                last = (last + 1) & 0xff;
                if self.ctx[USED + last as usize] == 0 {
                    skip += 1;
                }
                if self.g(ORDER + 4 * (i - 1) as usize) == last {
                    break;
                }
            }
            self.ctx[USED + last as usize] = 1;
            self.write_var_num(out, skip as u32);
        }
        if self.g(CLUES) == 0 {
            self.s(CLUE, 32000);
        }
        let end = self.g(END);
        let mut p = self.g(SRC);
        if p < end {
            let mut cur = self.byte(p);
            let mut prev = 0x100u32;
            loop {
                let mut sym = cur;
                let mut nxt = p.wrapping_add(1);
                if sym == prev {
                    let lim = if end < p.wrapping_add(0x7531) { end } else { p.wrapping_add(0x7531) };
                    let mut run = 0u32;
                    if nxt < lim {
                        loop {
                            sym = self.byte(nxt);
                            run += 1;
                            nxt = nxt.wrapping_add(1);
                            if sym != prev || lim <= nxt {
                                break;
                            }
                        }
                    }
                    let clues = self.g(CLUES);
                    let (pay, plain, clue_cost, _) = self.run_choice(&|i| self.cnt(i), cur, run, clues);
                    if pay {
                        if plain <= clue_cost {
                            let mut j = clues.wrapping_sub(1);
                            let mut r = run;
                            if j != 0 {
                                loop {
                                    let c = self.g(CLUE);
                                    if self.cnt(c.wrapping_add(j)) != 0 {
                                        let q = (r as i32).wrapping_div(j as i32);
                                        r = r.wrapping_sub(q.wrapping_mul(j as i32) as u32);
                                        for _ in 0..q {
                                            let c = self.g(CLUE);
                                            self.write_symbol(out, c.wrapping_add(j));
                                        }
                                    }
                                    if j - 1 == 0 {
                                        break;
                                    }
                                    j -= 1;
                                }
                            }
                        } else {
                            let c = self.g(CLUE);
                            self.write_bits(out, self.code(c), self.depth(c));
                            self.write_var_num(out, run);
                        }
                    } else {
                        for _ in 0..run {
                            self.write_symbol(out, prev);
                        }
                    }
                }
                p = nxt;
                let mut near = false;
                if self.g(NEARS) != 0 {
                    let d = sym.wrapping_sub(prev) as i32;
                    if d <= self.g(NEAR_HI) as i32 && self.g(NEAR_LO) as i32 <= d {
                        let mut s = (d as u32).wrapping_sub(1).wrapping_mul(2).wrapping_add(self.g(CLUE2));
                        if sym < prev {
                            s = (prev - sym - 1).wrapping_mul(2).wrapping_add(self.g(CLUE2)).wrapping_add(1);
                        }
                        if self.depth(s) < self.depth(sym) {
                            near = true;
                            self.write_bits(out, self.code(s), self.depth(s));
                        }
                    }
                }
                if !near {
                    self.write_symbol(out, sym);
                }
                if end <= p {
                    break;
                }
                cur = self.byte(p);
                prev = sym;
            }
        }
        let c = self.g(CLUE);
        self.write_bits(out, self.code(c), self.depth(c));
        self.write_var_num(out, 0);
        self.write_bits(out, 2, 2);
        self.write_bits(out, 0, 7);
    }

    /// `Huff_Encode` 0x2438c0(the source and its size, out, the unpacked size, the delta mode): the masks, the
    /// number table, the analysis (flags 0x39, codes up to 15 bits), the header (0x30fb / 0x32fb / 0x34fb and the
    /// size; 0x31fb.. with the unpacked size first when it differs; 0 for another mode), the table and the bytes
    pub fn encode(&mut self, out: &mut Out, unpacked: u32, mode: i64) -> u32 {
        self.s(BITS, 0);
        self.s(ACC, 0);
        self.s(MASK, 0);
        for i in 1..0x11 {
            self.s(MASK + 4 * i, self.g(MASK + 4 * (i - 1)).wrapping_mul(2).wrapping_add(1));
        }
        self.init_run_costs();
        let size = self.src.len() as u32;
        self.s(SIZE, size);
        self.s(SIZE2, size);
        out.pos = 0;
        self.s(BITS, 0);
        self.s(ACC, 0);
        self.s(OUT_BYTES, 0);
        self.s(END, self.src_addr.wrapping_add(size));
        self.s(SRC, self.src_addr);
        self.analyze(0x39, 0xf);
        let ty = |plain: u32| match mode {
            0 => plain,
            1 => plain + 0x200,
            2 => plain + 0x400,
            _ => 0,
        };
        if unpacked == size {
            self.write_bits(out, ty(0x30fb), 0x10);
            self.write_bits(out, size, 0x18);
        } else {
            self.write_bits(out, ty(0x31fb), 0x10);
            self.write_bits(out, unpacked, 0x18);
            self.write_bits(out, size, 0x18);
        }
        self.write_table(out);
        out.pos
    }
}

/// `Compress_Block` 0x243a60(src, size, dst, options): the delta mode (`*options`, 0 without) 1 delta-codes
/// the bytes once, 2 twice (a copy; decoded by 0x32fb / 0x34fb's sums); then `Huff_Encode` on a fresh context
/// (`ctx`: what `Compress_Alloc` gave). The bytes written.
pub fn compress_block(src: &[u8], src_addr: u32, out: &mut Out, mode: Option<i32>, ctx: Vec<u8>) -> u32 {
    let mode = mode.unwrap_or(0);
    let mut copy;
    let data: &[u8] = match mode {
        1 => {
            copy = src.to_vec();
            delta_encode(&mut copy);
            &copy
        }
        2 => {
            copy = src.to_vec();
            delta_encode(&mut copy);
            delta_encode(&mut copy);
            &copy
        }
        _ => src,
    };
    let mut e = Encoder::new(ctx, data, src_addr);
    e.encode(out, data.len() as u32, mode as i64)
}
