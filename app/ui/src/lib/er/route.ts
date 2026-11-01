import type { Point, Rect } from './model';

export const MARGIN = 10;
const BEND = 36;
const CELL = 256;
const FRAME = 2 * MARGIN;
const TRIES = 10;

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface End {
  side: Point;
  dir: 1 | -1;
  stub: number;
  owner: number;
}

export class Obstacles {
  readonly boxes: Box[];
  private buckets = new Map<number, number[]>();

  constructor(rects: readonly Rect[]) {
    this.boxes = rects.map((r) => ({ x0: r.x - MARGIN, y0: r.y - MARGIN, x1: r.x + r.w + MARGIN, y1: r.y + r.h + MARGIN }));
    this.boxes.forEach((b, i) => this.cells(b, (k) => this.bucket(k).push(i)));
  }

  private bucket(k: number): number[] {
    let list = this.buckets.get(k);
    if (!list) this.buckets.set(k, (list = []));
    return list;
  }

  private cells(b: Box, fn: (k: number) => void) {
    const [cx0, cy0, cx1, cy1] = [Math.floor(b.x0 / CELL), Math.floor(b.y0 / CELL), Math.floor(b.x1 / CELL), Math.floor(b.y1 / CELL)];
    for (let cy = cy0; cy <= cy1; cy++) for (let cx = cx0; cx <= cx1; cx++) fn(cy * 1_000_003 + cx);
  }

  near(q: Box): number[] {
    const out = new Set<number>();
    this.cells(q, (k) => {
      for (const i of this.buckets.get(k) ?? []) {
        const b = this.boxes[i];
        if (b && q.x0 < b.x1 && b.x0 < q.x1 && q.y0 < b.y1 && b.y0 < q.y1) out.add(i);
      }
    });
    return [...out];
  }

  crossed(a: Point, b: Point): number[] {
    const q = { x0: Math.min(a.x, b.x), y0: Math.min(a.y, b.y), x1: Math.max(a.x, b.x), y1: Math.max(a.y, b.y) };
    return this.near({ x0: q.x0 - 1, y0: q.y0 - 1, x1: q.x1 + 1, y1: q.y1 + 1 }).filter((i) => {
      const r = this.boxes[i];
      return !!r && q.x0 < r.x1 && r.x0 < q.x1 && q.y0 < r.y1 && r.y0 < q.y1 && (q.y0 === q.y1 ? r.y0 < q.y0 && q.y0 < r.y1 : r.x0 < q.x0 && q.x0 < r.x1);
    });
  }

  inside(p: Point, i: number): boolean {
    const b = this.boxes[i];
    return !!b && b.x0 < p.x && p.x < b.x1 && b.y0 < p.y && p.y < b.y1;
  }
}

let gScore = new Float64Array(0);
let stamp = new Uint32Array(0);
let parent = new Int32Array(0);
let epoch = 0;

function buffers(n: number) {
  if (gScore.length < n) {
    gScore = new Float64Array(n);
    stamp = new Uint32Array(n);
    parent = new Int32Array(n);
    epoch = 0;
  }
  epoch++;
  if (epoch === 0xffffffff) {
    stamp.fill(0);
    epoch = 1;
  }
}

class Heap {
  private ids: number[] = [];
  private keys: number[] = [];

  get size() {
    return this.ids.length;
  }

  push(id: number, key: number) {
    const { ids, keys } = this;
    let i = ids.length;
    ids.push(id);
    keys.push(key);
    while (i > 0) {
      const p = (i - 1) >> 1;
      if ((keys[p] ?? 0) <= key) break;
      ids[i] = ids[p] ?? 0;
      keys[i] = keys[p] ?? 0;
      i = p;
    }
    ids[i] = id;
    keys[i] = key;
  }

  pop(): number {
    const { ids, keys } = this;
    const top = ids[0] ?? -1;
    const id = ids.pop() ?? 0;
    const key = keys.pop() ?? 0;
    const n = ids.length;
    if (n === 0) return top;
    let i = 0;
    for (;;) {
      const l = 2 * i + 1;
      if (l >= n) break;
      const r = l + 1;
      const c = r < n && (keys[r] ?? 0) < (keys[l] ?? 0) ? r : l;
      if ((keys[c] ?? 0) >= key) break;
      ids[i] = ids[c] ?? 0;
      keys[i] = keys[c] ?? 0;
      i = c;
    }
    ids[i] = id;
    keys[i] = key;
    return top;
  }
}

const DX = [1, 0, -1, 0];
const DY = [0, 1, 0, -1];

function sortedUnique(v: number[]): number[] {
  v.sort((a, b) => a - b);
  return v.filter((x, i) => i === 0 || x !== v[i - 1]);
}

function lower(v: readonly number[], x: number): number {
  let [lo, hi] = [0, v.length];
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if ((v[mid] ?? 0) < x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

function search(boxes: readonly Box[], a: Point, dirA: number, b: Point, dirB: number): { pts: Point[]; cost: number } | null {
  const xsRaw = [a.x, b.x];
  const ysRaw = [a.y, b.y];
  let [fx0, fy0, fx1, fy1] = [Math.min(a.x, b.x), Math.min(a.y, b.y), Math.max(a.x, b.x), Math.max(a.y, b.y)];
  for (const r of boxes) {
    xsRaw.push(r.x0, r.x1);
    ysRaw.push(r.y0, r.y1);
    fx0 = Math.min(fx0, r.x0);
    fy0 = Math.min(fy0, r.y0);
    fx1 = Math.max(fx1, r.x1);
    fy1 = Math.max(fy1, r.y1);
  }
  xsRaw.push(fx0 - FRAME, fx1 + FRAME);
  ysRaw.push(fy0 - FRAME, fy1 + FRAME);
  const xs = sortedUnique(xsRaw);
  const ys = sortedUnique(ysRaw);
  const nx = xs.length;
  const ny = ys.length;
  const cells = nx * ny;
  const node = new Uint8Array(cells);
  const hBlock = new Uint8Array(cells);
  const vBlock = new Uint8Array(cells);
  for (const r of boxes) {
    const i0 = lower(xs, r.x0);
    const i1 = lower(xs, r.x1);
    const j0 = lower(ys, r.y0);
    const j1 = lower(ys, r.y1);
    for (let j = j0; j <= j1; j++)
      for (let i = i0; i <= i1; i++) {
        const k = j * nx + i;
        const innerY = j > j0 && j < j1;
        const innerX = i > i0 && i < i1;
        if (innerX && innerY) node[k] = 1;
        if (innerY && i < i1) hBlock[k] = 1;
        if (innerX && j < j1) vBlock[k] = 1;
      }
  }
  const start = lower(ys, a.y) * nx + lower(xs, a.x);
  const goal = lower(ys, b.y) * nx + lower(xs, b.x);
  const terminal = cells * 4;
  buffers(terminal + 1);
  const heap = new Heap();
  const h = (k: number) => Math.abs((xs[k % nx] ?? 0) - b.x) + Math.abs((ys[Math.floor(k / nx)] ?? 0) - b.y);
  const relax = (s: number, g: number, from: number, key: number) => {
    if (stamp[s] === epoch && (gScore[s] ?? Infinity) <= g) return;
    stamp[s] = epoch;
    gScore[s] = g;
    parent[s] = from;
    heap.push(s, key);
  };
  relax(start * 4 + dirA, 0, -1, h(start));
  while (heap.size) {
    const s = heap.pop();
    if (s === terminal) break;
    const g = gScore[s] ?? Infinity;
    const k = s >> 2;
    const d = s & 3;
    if (k === goal) {
      const cost = g + (d === dirB ? 0 : BEND);
      relax(terminal, cost, s, cost);
      continue;
    }
    const i = k % nx;
    const j = (k - i) / nx;
    for (let nd = 0; nd < 4; nd++) {
      if (nd === (d ^ 2)) continue;
      const ni = i + (DX[nd] ?? 0);
      const nj = j + (DY[nd] ?? 0);
      if (ni < 0 || nj < 0 || ni >= nx || nj >= ny) continue;
      const nk = nj * nx + ni;
      if (node[nk] && nk !== goal) continue;
      if (nd === 0 && hBlock[k]) continue;
      if (nd === 2 && hBlock[nk]) continue;
      if (nd === 1 && vBlock[k]) continue;
      if (nd === 3 && vBlock[nk]) continue;
      const len = Math.abs((xs[ni] ?? 0) - (xs[i] ?? 0)) + Math.abs((ys[nj] ?? 0) - (ys[j] ?? 0));
      const ng = g + len + (nd === d ? 0 : BEND);
      relax(nk * 4 + nd, ng, s, ng + h(nk));
    }
  }
  if (stamp[terminal] !== epoch) return null;
  const pts: Point[] = [];
  for (let s = parent[terminal] ?? -1; s >= 0; s = parent[s] ?? -1) {
    const k = s >> 2;
    pts.push({ x: xs[k % nx] ?? 0, y: ys[Math.floor(k / nx)] ?? 0 });
    if (s === start * 4 + dirA) break;
  }
  return { pts: pts.reverse(), cost: gScore[terminal] ?? Infinity };
}

export function simplify(pts: readonly Point[]): Point[] {
  const out: Point[] = [];
  for (const p of pts) {
    const last = out.at(-1);
    if (last && last.x === p.x && last.y === p.y) continue;
    const prev = out.at(-2);
    if (prev && last && ((prev.x === last.x && last.x === p.x) || (prev.y === last.y && last.y === p.y))) out[out.length - 1] = p;
    else out.push(p);
  }
  return out;
}

const dirIndex = (dir: 1 | -1) => (dir === 1 ? 0 : 2);

function stubPoint(obs: Obstacles, e: End, ignored: Set<number>): Point {
  for (const len of [e.stub, Math.min(e.stub, MARGIN * 1.8)]) {
    const p = { x: e.side.x + e.dir * len, y: e.side.y };
    const hit = obs.crossed(e.side, p).filter((i) => i !== e.owner);
    if (hit.length === 0 && !obs.near({ x0: p.x - 1, y0: p.y - 1, x1: p.x + 1, y1: p.y + 1 }).some((i) => i !== e.owner && obs.inside(p, i))) return p;
  }
  const p = { x: e.side.x + e.dir * Math.min(e.stub, MARGIN * 1.8), y: e.side.y };
  for (const i of obs.crossed(e.side, p)) if (i !== e.owner) ignored.add(i);
  for (const i of obs.near({ x0: p.x - 1, y0: p.y - 1, x1: p.x + 1, y1: p.y + 1 })) if (obs.inside(p, i)) ignored.add(i);
  return p;
}

export function routeAround(obs: Obstacles, from: End, to: End): { pts: Point[]; cost: number } | null {
  const ignored = new Set<number>();
  const a = stubPoint(obs, from, ignored);
  const b = stubPoint(obs, to, ignored);
  const pad = 3 * MARGIN;
  const set = new Set(obs.near({ x0: Math.min(a.x, b.x) - pad, y0: Math.min(a.y, b.y) - pad, x1: Math.max(a.x, b.x) + pad, y1: Math.max(a.y, b.y) + pad }));
  set.add(from.owner);
  set.add(to.owner);
  for (const i of ignored) set.delete(i);
  for (let attempt = 0; attempt <= TRIES; attempt++) {
    const all = attempt === TRIES;
    const boxes = (all ? obs.boxes.filter((_, i) => !ignored.has(i)) : [...set].flatMap((i) => obs.boxes[i] ?? [])).filter(
      (r) => !(r.x0 < a.x && a.x < r.x1 && r.y0 < a.y && a.y < r.y1) && !(r.x0 < b.x && b.x < r.x1 && r.y0 < b.y && b.y < r.y1),
    );
    const found = search(boxes, a, dirIndex(from.dir), b, dirIndex(-to.dir as 1 | -1));
    if (!found) return null;
    const pts = simplify([from.side, ...found.pts, to.side]);
    let fresh = false;
    for (let s = 0; s < pts.length - 1; s++) {
      const [p, q] = [pts[s], pts[s + 1]];
      if (!p || !q) continue;
      const own = s === 0 ? from.owner : s === pts.length - 2 ? to.owner : -1;
      for (const i of obs.crossed(p, q))
        if (i !== own && !set.has(i) && !ignored.has(i)) {
          set.add(i);
          fresh = true;
        }
    }
    if (!fresh) return { pts, cost: found.cost };
  }
  return null;
}
