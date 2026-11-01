import { DRIVERS, SUPPORTED, type Driver } from './drivers';
import { wordmarkGeometry } from './wordmark';

export const STAGE = { w: 1440, h: 900 } as const;
export const COL_W = [96, 168, 112, 184, 88, 152, 120, 120] as const;
export const FIELD = { x: 200, y: 190, cols: COL_W.length, rows: 13, w: 1040, rh: 40 } as const;
export const COL_X = COL_W.reduce<number[]>((xs, w) => [...xs, (xs.at(-1) ?? FIELD.x) + w], [FIELD.x]);
export const WORD_SIZE = 104;
export const DIGIT_H = 16;

export const BEAT = {
  lead: 0,
  rows: 140,
  cols: 300,
  cells: 620,
  links: 980,
  monogram: 1180,
  brace: 1640,
  fold: 1800,
  land: 2150,
  still: 2270,
  resolve: 2470,
  word: 2540,
  engines: 2700,
} as const;

export function random(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface Digit {
  strip: string;
  n: number;
  t: number;
}

export type Cell = { c: number; r: number; d: number } & (
  | { kind: 'num'; digits: (Digit | string)[] }
  | { kind: 'glyph'; glyph: number }
  | { kind: 'engine'; engine: Driver }
);

export interface Link {
  path: string;
  d: number;
  t: number;
  ends: [number, number, number, number];
}

export const ENGINES: readonly Driver[] = SUPPORTED.map((id) => DRIVERS.find((d) => d.id === id)).filter((d): d is Driver => d !== undefined);

export const GLYPHS = 8;

function number(rand: () => number): (Digit | string)[] {
  const len = 2 + Math.floor(rand() * 6);
  const chars: string[] = [];
  for (let i = 0; i < len; i++) chars.push(String(Math.floor(rand() * 10)));
  if (len > 4 && rand() < 0.5) chars.splice(len - 3, 0, '.');
  else if (len > 2 && rand() < 0.3) chars.splice(len - 2, 0, ',');
  return chars.map((ch) => {
    if (!/\d/.test(ch)) return ch;
    const n = 9 + Math.floor(rand() * 9);
    const spin = Array.from({ length: n }, () => String(Math.floor(rand() * 10)));
    return { strip: [...spin, ch].join('\n'), n, t: 900 + Math.floor(rand() * 600) };
  });
}

export function cellCenter(c: number, r: number) {
  return { x: (COL_X[c] ?? 0) + (COL_W[c] ?? 0) / 2, y: FIELD.y + r * FIELD.rh + FIELD.rh / 2 };
}

export function cellRight(c: number) {
  return COL_X[c + 1] ?? 0;
}

export function scene(seed = 11) {
  const rand = random(seed);
  const free = Array.from({ length: FIELD.cols * FIELD.rows }, (_, i) => i);
  const take = () => {
    const i = Math.floor(rand() * free.length);
    const [v] = free.splice(i, 1);
    return { c: (v ?? 0) % FIELD.cols, r: Math.floor((v ?? 0) / FIELD.cols) };
  };
  const cells: Cell[] = [];
  const glyphCells: { c: number; r: number }[] = [];
  for (let g = 0; g < GLYPHS; g++) {
    const at = take();
    glyphCells.push(at);
    cells.push({ ...at, kind: 'glyph', glyph: g, d: BEAT.cells + Math.floor(rand() * 480) });
  }
  ENGINES.forEach((engine) => {
    const at = take();
    glyphCells.push(at);
    cells.push({ ...at, kind: 'engine', engine, d: BEAT.cells + 60 + Math.floor(rand() * 480) });
  });
  for (let i = 0; i < 26; i++) cells.push({ ...take(), kind: 'num', digits: number(rand), d: BEAT.cells - 80 + Math.floor(rand() * 640) });

  const used = new Set(cells.map((c) => c.r * FIELD.cols + c.c));
  const empty = (c: number, r: number) => !used.has(r * FIELD.cols + c);
  const links: Link[] = [];
  const taken = new Set<number>();
  for (let i = 0; i < glyphCells.length && links.length < 5; i++)
    for (let j = 0; j < glyphCells.length && links.length < 5; j++) {
      const l = glyphCells[i];
      const r = glyphCells[j];
      if (!l || !r || taken.has(i) || taken.has(j) || r.c - l.c < 2 || l.r === r.r) continue;
      for (let m = l.c + 1; m <= r.c; m++) {
        let clear = true;
        for (let c = l.c + 1; c < m; c++) clear &&= empty(c, l.r);
        for (let c = m; c < r.c; c++) clear &&= empty(c, r.r);
        if (!clear) continue;
        const p = cellCenter(l.c, l.r);
        const q = cellCenter(r.c, r.r);
        const mx = COL_X[m] ?? 0;
        const x0 = p.x + 16;
        const x1 = q.x - 16;
        const s = Math.sign(q.y - p.y);
        const rad = 8;
        const path = `M ${x0} ${p.y} H ${mx - rad} Q ${mx} ${p.y} ${mx} ${p.y + s * rad} V ${q.y - s * rad} Q ${mx} ${q.y} ${mx + rad} ${q.y} H ${x1}`;
        links.push({ path, d: BEAT.links + links.length * 80, t: 520, ends: [x0, p.y, x1, q.y] });
        taken.add(i).add(j);
        for (let c = l.c + 1; c < m; c++) used.add(l.r * FIELD.cols + c);
        for (let c = m; c < r.c; c++) used.add(r.r * FIELD.cols + c);
        break;
      }
    }
  return { cells, links };
}

export function choreography() {
  const g = wordmarkGeometry(WORD_SIZE, true);
  const left = Math.round(STAGE.w / 2 - g.width / 2);
  const top = 368;
  const fieldMid = FIELD.x + FIELD.w / 2;
  const totals = g.rules.map((r, i) => ({
    x: left + r.x,
    y: top + r.y,
    w: r.w,
    h: r.h,
    dx: fieldMid - (left + r.x + r.w / 2),
    sx: FIELD.w / r.w,
    d: BEAT.land + i * 30,
  }));
  const rowY = Array.from({ length: FIELD.rows + 1 }, (_, j) => FIELD.y + j * FIELD.rh);
  const mid = FIELD.rows / 2;
  const rows = rowY.map((y, j) => {
    const target = (j <= mid ? totals[0] : totals[1]) ?? { y };
    const away = Math.abs(j - mid);
    return {
      y,
      dy: target.y - y,
      from: j % 2 ? 1 : -1,
      enter: BEAT.rows + Math.round(away * 42),
      out: BEAT.fold + Math.round((mid - away) * 12),
    };
  });
  const cols = COL_X.map((x, i) => ({
    x,
    from: i % 2 ? 1 : -1,
    enter: BEAT.cols + i * 46,
    out: BEAT.fold + Math.round(Math.abs(i - FIELD.cols / 2) * 16),
  }));
  return { word: { left, top, size: WORD_SIZE }, totals, rows, cols, originY: totals[0]?.y ?? STAGE.h / 2 };
}


export const MONOGRAM = (() => {
  const path: [number, number][] = [[0, 3], [0, 2], [0, 1], [0, 0], [1, 1], [2, 2], [3, 3], [3, 2], [3, 1], [3, 0]];
  return path.map(([c, r], i) => ({
    x: STAGE.w / 2 - 2 * FIELD.rh + c * FIELD.rh + 4,
    y: FIELD.y + (5 + r) * FIELD.rh + 4,
    w: FIELD.rh - 8,
    h: FIELD.rh - 8,
    diag: c === 1 || c === 2,
    d: BEAT.monogram + i * 34,
  }));
})();
