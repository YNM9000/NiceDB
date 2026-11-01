export interface Cell {
  r: number;
  c: number;
}

export interface Range {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

export interface Selection {
  ranges: readonly Range[];
  anchor: Cell | null;
  focus: Cell | null;
}

export interface Bounds {
  rows: number;
  cols: number;
}

export interface Mods {
  shift?: boolean;
  toggle?: boolean;
}

export const EMPTY: Selection = { ranges: [], anchor: null, focus: null };

export const span = (a: Cell, b: Cell): Range => ({ r0: Math.min(a.r, b.r), c0: Math.min(a.c, b.c), r1: Math.max(a.r, b.r), c1: Math.max(a.c, b.c) });

export const inside = (x: Range, r: number, c: number) => r >= x.r0 && r <= x.r1 && c >= x.c0 && c <= x.c1;

export const covers = (outer: Range, inner: Range) => inner.r0 >= outer.r0 && inner.r1 <= outer.r1 && inner.c0 >= outer.c0 && inner.c1 <= outer.c1;

export function selected(s: Selection, r: number, c: number): boolean {
  for (const x of s.ranges) if (inside(x, r, c)) return true;
  return false;
}

export function subtract(x: Range, hole: Range): Range[] {
  const r0 = Math.max(x.r0, hole.r0);
  const r1 = Math.min(x.r1, hole.r1);
  const c0 = Math.max(x.c0, hole.c0);
  const c1 = Math.min(x.c1, hole.c1);
  if (r0 > r1 || c0 > c1) return [x];
  const out: Range[] = [];
  if (x.r0 < r0) out.push({ r0: x.r0, c0: x.c0, r1: r0 - 1, c1: x.c1 });
  if (r1 < x.r1) out.push({ r0: r1 + 1, c0: x.c0, r1: x.r1, c1: x.c1 });
  if (x.c0 < c0) out.push({ r0, c0: x.c0, r1, c1: c0 - 1 });
  if (c1 < x.c1) out.push({ r0, c0: c1 + 1, r1, c1: x.c1 });
  return out;
}

function remove(s: Selection, hole: Range, at: Cell): Selection {
  return { ranges: s.ranges.flatMap((x) => subtract(x, hole)), anchor: at, focus: at };
}

function pick(s: Selection, area: Range, anchor: Cell, focus: Cell, m: Mods): Selection {
  if (m.shift && s.anchor) return extendTo(s, focus);
  if (m.toggle) {
    if (s.ranges.some((x) => covers(x, area))) return remove(s, area, focus);
    return { ranges: [...s.ranges, area], anchor, focus };
  }
  return { ranges: [area], anchor, focus };
}

export function extendTo(s: Selection, focus: Cell): Selection {
  const anchor = s.anchor ?? focus;
  return { ranges: [...s.ranges.slice(0, -1), span(anchor, focus)], anchor, focus };
}

export function clickCell(s: Selection, cell: Cell, m: Mods = {}): Selection {
  return pick(s, span(cell, cell), cell, cell, m);
}

export function clickRow(s: Selection, r: number, b: Bounds, m: Mods = {}): Selection {
  if (b.cols === 0) return s;
  const row = { r0: r, c0: 0, r1: r, c1: b.cols - 1 };
  if (m.shift && s.anchor) {
    const a = s.anchor.r;
    return { ranges: [...s.ranges.slice(0, -1), { r0: Math.min(a, r), c0: 0, r1: Math.max(a, r), c1: b.cols - 1 }], anchor: s.anchor, focus: { r, c: s.focus?.c ?? 0 } };
  }
  return pick(s, row, { r, c: 0 }, { r, c: 0 }, m);
}

export function clickColumn(s: Selection, c: number, b: Bounds, m: Mods = {}): Selection {
  if (b.rows === 0) return s;
  const col = { r0: 0, c0: c, r1: b.rows - 1, c1: c };
  if (m.shift && s.anchor) {
    const a = s.anchor.c;
    return { ranges: [...s.ranges.slice(0, -1), { r0: 0, c0: Math.min(a, c), r1: b.rows - 1, c1: Math.max(a, c) }], anchor: s.anchor, focus: { r: s.focus?.r ?? 0, c } };
  }
  return pick(s, col, { r: 0, c }, { r: 0, c }, m);
}

export function dragRows(s: Selection, r: number, b: Bounds): Selection {
  const a = s.anchor?.r ?? r;
  return { ranges: [...s.ranges.slice(0, -1), { r0: Math.min(a, r), c0: 0, r1: Math.max(a, r), c1: b.cols - 1 }], anchor: s.anchor ?? { r, c: 0 }, focus: { r, c: s.focus?.c ?? 0 } };
}

export function dragColumns(s: Selection, c: number, b: Bounds): Selection {
  const a = s.anchor?.c ?? c;
  return { ranges: [...s.ranges.slice(0, -1), { r0: 0, c0: Math.min(a, c), r1: b.rows - 1, c1: Math.max(a, c) }], anchor: s.anchor ?? { r: 0, c }, focus: { r: s.focus?.r ?? 0, c } };
}

export function all(b: Bounds): Selection {
  if (b.rows === 0 || b.cols === 0) return EMPTY;
  return { ranges: [{ r0: 0, c0: 0, r1: b.rows - 1, c1: b.cols - 1 }], anchor: { r: 0, c: 0 }, focus: { r: b.rows - 1, c: b.cols - 1 } };
}

const clamp = (v: number, hi: number) => Math.min(hi, Math.max(0, v));

export function move(s: Selection, dr: number, dc: number, b: Bounds, extend = false): Selection {
  if (b.rows === 0 || b.cols === 0) return s;
  const from = s.focus ?? { r: 0, c: 0 };
  const to = { r: clamp(from.r + dr, b.rows - 1), c: clamp(from.c + dc, b.cols - 1) };
  if (extend && s.anchor) return extendTo(s, to);
  return clickCell(EMPTY, to);
}

export function axes(s: Selection): { rows: number[]; cols: number[] } {
  const rows = new Set<number>();
  const cols = new Set<number>();
  for (const x of s.ranges) {
    for (let r = x.r0; r <= x.r1; r++) rows.add(r);
    for (let c = x.c0; c <= x.c1; c++) cols.add(c);
  }
  return { rows: [...rows].sort((a, b) => a - b), cols: [...cols].sort((a, b) => a - b) };
}

export function count(s: Selection): number {
  if (s.ranges.length === 1) {
    const x = s.ranges[0]!;
    return (x.r1 - x.r0 + 1) * (x.c1 - x.c0 + 1);
  }
  const { rows, cols } = axes(s);
  let n = 0;
  for (const r of rows) for (const c of cols) if (selected(s, r, c)) n++;
  return n;
}

export function project<T>(s: Selection, at: (r: number, c: number) => T): { rows: number[]; cols: number[]; cells: (T | undefined)[][] } {
  if (s.ranges.length === 1) {
    const x = s.ranges[0]!;
    const rows = Array.from({ length: x.r1 - x.r0 + 1 }, (_, i) => x.r0 + i);
    const cols = Array.from({ length: x.c1 - x.c0 + 1 }, (_, i) => x.c0 + i);
    return { rows, cols, cells: rows.map((r) => cols.map((c) => at(r, c))) };
  }
  const { rows, cols } = axes(s);
  return { rows, cols, cells: rows.map((r) => cols.map((c) => (selected(s, r, c) ? at(r, c) : undefined))) };
}

export function wholeRows(s: Selection, b: Bounds): number[] | null {
  if (!s.ranges.length || !s.ranges.every((x) => x.c0 === 0 && x.c1 === b.cols - 1)) return null;
  return axes(s).rows;
}

export function wholeColumns(s: Selection, b: Bounds): number[] | null {
  if (!b.rows || !s.ranges.length || !s.ranges.every((x) => x.r0 === 0 && x.r1 === b.rows - 1)) return null;
  return axes(s).cols;
}

export function rowSpans(s: Selection): [number, number][] {
  const sorted = [...s.ranges].map((x): [number, number] => [x.r0, x.r1]).sort((a, b) => a[0] - b[0]);
  const out: [number, number][] = [];
  for (const [a, b] of sorted) {
    const last = out.at(-1);
    if (last && a <= last[1] + 1) last[1] = Math.max(last[1], b);
    else out.push([a, b]);
  }
  return out;
}

export function byColumn<T>(s: Selection, at: (r: number, c: number) => T): { c: number; values: T[] }[] {
  if (s.ranges.length === 1) {
    const x = s.ranges[0]!;
    const out: { c: number; values: T[] }[] = [];
    for (let c = x.c0; c <= x.c1; c++) {
      const values: T[] = new Array<T>(x.r1 - x.r0 + 1);
      for (let r = x.r0; r <= x.r1; r++) values[r - x.r0] = at(r, c);
      out.push({ c, values });
    }
    return out;
  }
  const { rows, cols } = axes(s);
  return cols.map((c) => ({ c, values: rows.filter((r) => selected(s, r, c)).map((r) => at(r, c)) }));
}
