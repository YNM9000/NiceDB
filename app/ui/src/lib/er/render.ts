import { bounds, HEAD, ROW, type ErEdge, type ErGraph, type ErTable, type Point, type Rect } from './model';

const COLORS = [
  'bg', 'grid', 'grid-2', 'card', 'head', 'head-ink', 'ink', 'ink-2', 'ink-3', 'rule', 'frame', 'sel', 'hit', 'line', 'line-case', 'line-hi',
  'pk', 'fk', 'area-ink', 'lamp-on', 'lamp-near', 'lamp-off', 'shadow', 'area-1', 'area-2', 'area-3', 'area-4', 'area-5', 'area-6',
] as const;
type ColorName = (typeof COLORS)[number];

const fitted = new Map<string, string>();

export function fitText(ctx: Pick<CanvasRenderingContext2D, 'measureText' | 'font'>, text: string, max: number, px: number): string {
  if (text.length * px * 0.45 <= max * 1.25) return text;
  const key = `${ctx.font}|${Math.round(max)}|${text}`;
  const hit = fitted.get(key);
  if (hit !== undefined) return hit;
  const head = text.slice(0, Math.ceil(max / (px * 0.3)) + 1);
  let out = text;
  if (head.length < text.length || ctx.measureText(head).width > max * 1.25) {
    let lo = 0;
    let hi = head.length;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (ctx.measureText(`${head.slice(0, mid)}…`).width <= max) lo = mid;
      else hi = mid - 1;
    }
    out = `${head.slice(0, lo)}…`;
  }
  if (fitted.size > 4000) fitted.clear();
  fitted.set(key, out);
  return out;
}

export interface Look {
  c: Record<ColorName, string>;
  areas: string[];
  gridKind: 'none' | 'lines' | 'tiles';
  gridStep: number;
  coords: boolean;
  frameW: number;
  selW: number;
  selDouble: boolean;
  lineW: number;
  lineCaseW: number;
  areaAlpha: number;
  wash: 'water' | 'outline' | 'flat' | 'none';
  radius: number;
  caps: boolean;
  lamp: boolean;
  dim: number;
  fontTitle: string;
  fontMono: string;
  fontArea: string;
  areaSize: number;
  areaStyle: string;
  titleWeight: string;
  inset: { top: number; right: number; bottom: number; left: number };
}

export interface View {
  x: number;
  y: number;
  k: number;
}

export interface Marks {
  selected: string | null;
  related: ReadonlySet<string> | null;
  hits: ReadonlySet<string>;
  hidden: ReadonlySet<string>;
}

export const NO_MARKS: Marks = { selected: null, related: null, hits: new Set(), hidden: new Set() };

export const LOD_SOLID = 0.24;
export const LOD_BLOCK = 0.55;
export const LOD_TYPES = 0.68;

export function readLook(host: HTMLElement): Look {
  const cs = getComputedStyle(host);
  const v = (name: string) => cs.getPropertyValue(`--er-${name}`).trim();
  const n = (name: string, fallback: number) => {
    const x = Number.parseFloat(v(name));
    return Number.isFinite(x) ? x : fallback;
  };
  const pick = <T extends string>(name: string, options: readonly [T, ...T[]]): T => options.find((o) => o === v(name)) ?? options[0];
  const probe = document.createElement('span');
  probe.style.display = 'none';
  host.append(probe);
  const c = {} as Record<ColorName, string>;
  for (const name of COLORS) {
    probe.style.color = 'transparent';
    probe.style.color = `var(--er-${name}, transparent)`;
    c[name] = getComputedStyle(probe).color;
  }
  probe.remove();
  return {
    c,
    areas: [c['area-1'], c['area-2'], c['area-3'], c['area-4'], c['area-5'], c['area-6']],
    gridKind: pick('grid-kind', ['none', 'lines', 'tiles']),
    gridStep: n('grid-step', 20),
    coords: v('coords') === '1',
    frameW: n('frame-w', 1),
    selW: n('sel-w', 2),
    selDouble: v('sel-double') === '1',
    lineW: n('line-w', 1),
    lineCaseW: n('line-case-w', 0),
    areaAlpha: n('area-alpha', 0.15),
    wash: pick('wash', ['none', 'water', 'outline', 'flat']),
    radius: n('radius', 0),
    caps: v('caps') === '1',
    lamp: v('lamp') === '1',
    dim: n('dim', 0.28),
    fontTitle: v('font-title') || 'sans-serif',
    fontMono: v('font-mono') || 'monospace',
    fontArea: v('font-area') || 'sans-serif',
    areaSize: n('area-size', 16),
    areaStyle: v('area-style') || 'normal',
    titleWeight: v('title-weight') || '600',
    inset: { top: n('inset-top', 0), right: n('inset-right', 0), bottom: n('inset-bottom', 0), left: n('inset-left', 0) },
  };
}

export const areaColor = (look: Look, area: number) => look.areas[area % look.areas.length] ?? look.c.rule;

export type EndKind = 'many' | 'one' | 'one-only' | 'zero-one';

export interface EndShape {
  lines: [number, number, number, number][];
  ring: { x: number; y: number; r: number } | null;
}

export function endShape(at: Point, toward: Point, kind: EndKind): EndShape {
  const dir = Math.sign(toward.x - at.x) || 1;
  const x = (d: number) => at.x + dir * d;
  const bar = (d: number): [number, number, number, number] => [x(d), at.y - 5, x(d), at.y + 5];
  switch (kind) {
    case 'many':
      return { lines: [[at.x, at.y - 6, x(11), at.y], [at.x, at.y + 6, x(11), at.y]], ring: null };
    case 'one':
      return { lines: [bar(8)], ring: null };
    case 'one-only':
      return { lines: [bar(7), bar(11)], ring: null };
    case 'zero-one':
      return { lines: [bar(7)], ring: { x: x(15), y: at.y, r: 3.5 } };
  }
}

export function edgeEnds(e: ErEdge): [EndKind, EndKind] {
  return [e.unique ? 'one' : 'many', e.optional ? 'zero-one' : 'one-only'];
}

export function areaBoxes(g: ErGraph, hidden: ReadonlySet<string>): (Rect & { area: number })[] {
  return g.areas.flatMap((a) => {
    const members = a.tables.flatMap((id) => {
      const t = g.byId.get(id);
      return t && !hidden.has(id) ? [t] : [];
    });
    return members.length ? [{ ...bounds(members, 26), area: a.index }] : [];
  });
}

export function zoneName(i: number): string {
  let s = '';
  let n = i;
  do {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
  } while (n >= 0);
  return s;
}

function roundRect(ctx: CanvasRenderingContext2D, r: Rect, radius: number) {
  ctx.beginPath();
  if (radius > 0) ctx.roundRect(r.x, r.y, r.w, r.h, radius);
  else ctx.rect(r.x, r.y, r.w, r.h);
}

function polyline(ctx: CanvasRenderingContext2D, pts: readonly Point[], radius: number) {
  const first = pts[0];
  if (!first) return;
  ctx.moveTo(first.x, first.y);
  for (let i = 1; i < pts.length - 1; i++) {
    const p = pts[i];
    const q = pts[i + 1];
    const o = pts[i - 1];
    if (!p || !q || !o) continue;
    const r = Math.min(radius, Math.abs(p.x - o.x + p.y - o.y) / 2, Math.abs(q.x - p.x + q.y - p.y) / 2);
    ctx.arcTo(p.x, p.y, q.x, q.y, r);
  }
  const last = pts.at(-1);
  if (last) ctx.lineTo(last.x, last.y);
}

function end(ctx: CanvasRenderingContext2D, look: Look, shape: EndShape) {
  ctx.beginPath();
  for (const [x1, y1, x2, y2] of shape.lines) {
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
  }
  ctx.stroke();
  if (!shape.ring) return;
  ctx.beginPath();
  ctx.arc(shape.ring.x, shape.ring.y, shape.ring.r, 0, Math.PI * 2);
  ctx.fillStyle = look.c.bg;
  ctx.fill();
  ctx.stroke();
}

function wash(ctx: CanvasRenderingContext2D, look: Look, box: Rect & { area: number }, label: string, k: number) {
  const color = areaColor(look, box.area);
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  if (look.wash === 'water') {
    const seed = box.area * 7.3;
    for (let i = 0; i < 3; i++) {
      const j = Math.round(Math.sin(seed + i * 2.1) * 9);
      ctx.globalAlpha = look.areaAlpha * (i === 0 ? 1 : 0.5);
      roundRect(ctx, { x: box.x + j, y: box.y - j / 2, w: box.w - j, h: box.h + j }, 18 + i * 6);
      ctx.fill();
    }
    ctx.globalAlpha = Math.min(1, look.areaAlpha * 2);
    ctx.lineWidth = 1;
    roundRect(ctx, box, 18);
    ctx.stroke();
  } else if (look.wash === 'flat') {
    ctx.globalAlpha = look.areaAlpha;
    roundRect(ctx, box, look.radius * 2);
    ctx.fill();
    ctx.globalAlpha = Math.min(1, look.areaAlpha * 3);
    ctx.lineWidth = 1;
    ctx.stroke();
  } else if (look.wash === 'outline') {
    ctx.setLineDash([12, 4, 2, 4]);
    ctx.lineWidth = 0.8;
    ctx.strokeStyle = look.c['ink-3'];
    roundRect(ctx, box, 0);
    ctx.stroke();
    ctx.setLineDash([]);
  }
  ctx.globalAlpha = 1;
  if (!label) return;
  ctx.fillStyle = look.c['area-ink'];
  const size = Math.min(Math.max(look.areaSize, 13 / k), (box.w * 0.8) / (label.length * 0.55));
  if (size * k < 6) return;
  ctx.font = `${look.areaStyle} ${size}px ${look.fontArea}`;
  const text = look.caps ? label.toUpperCase() : label;
  if (look.wash === 'outline') {
    ctx.textBaseline = 'bottom';
    ctx.textAlign = 'left';
    ctx.fillText(text, box.x + 2, box.y - 4);
  } else {
    ctx.textBaseline = 'alphabetic';
    ctx.textAlign = 'right';
    ctx.fillText(text, box.x + box.w - 12, box.y + box.h - 9);
  }
}

function grid(ctx: CanvasRenderingContext2D, look: Look, view: View, w: number, h: number) {
  if (look.gridKind === 'none') return;
  let step = look.gridStep;
  while (step * view.k < 9) step *= look.gridKind === 'tiles' ? 4 : 5;
  const x0 = Math.floor(-view.x / view.k / step) * step;
  const y0 = Math.floor(-view.y / view.k / step) * step;
  const x1 = (w - view.x) / view.k;
  const y1 = (h - view.y) / view.k;
  ctx.lineWidth = 1 / view.k;
  if (look.gridKind === 'tiles') {
    const off = 1 / view.k;
    for (const [color, d] of [[look.c.grid, 0], [look.c['grid-2'], off]] as const) {
      ctx.strokeStyle = color;
      ctx.beginPath();
      for (let x = x0; x <= x1; x += step) {
        ctx.moveTo(x + d, y0);
        ctx.lineTo(x + d, y1);
      }
      for (let y = y0; y <= y1; y += step) {
        ctx.moveTo(x0, y + d);
        ctx.lineTo(x1, y + d);
      }
      ctx.stroke();
    }
    return;
  }
  for (const major of [false, true]) {
    ctx.strokeStyle = major ? look.c['grid-2'] : look.c.grid;
    ctx.beginPath();
    for (let x = x0; x <= x1; x += step) {
      if ((Math.round(x / step) % 5 === 0) !== major) continue;
      ctx.moveTo(x, y0);
      ctx.lineTo(x, y1);
    }
    for (let y = y0; y <= y1; y += step) {
      if ((Math.round(y / step) % 5 === 0) !== major) continue;
      ctx.moveTo(x0, y);
      ctx.lineTo(x1, y);
    }
    ctx.stroke();
  }
}

export const ZONE = 400;
export const BAND = 16;

export function coords(ctx: CanvasRenderingContext2D, look: Look, view: View, w: number, h: number, origin: Point) {
  if (!look.coords) return;
  ctx.fillStyle = look.c.bg;
  ctx.fillRect(0, 0, w, BAND);
  ctx.fillRect(0, 0, BAND, h);
  ctx.strokeStyle = look.c.ink;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(BAND + 0.5, BAND + 0.5);
  ctx.lineTo(w, BAND + 0.5);
  ctx.moveTo(BAND + 0.5, BAND + 0.5);
  ctx.lineTo(BAND + 0.5, h);
  ctx.stroke();
  ctx.fillStyle = look.c['ink-2'];
  ctx.font = `9.5px ${look.fontMono}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  let zone = ZONE;
  while (zone * view.k < 60) zone *= 2;
  const z = zone * view.k;
  ctx.beginPath();
  for (let i = Math.floor((-view.x / view.k - origin.x) / zone); ; i++) {
    const sx = (origin.x + i * zone) * view.k + view.x;
    if (sx > w) break;
    ctx.moveTo(Math.round(sx) + 0.5, BAND - 5);
    ctx.lineTo(Math.round(sx) + 0.5, BAND);
    if (i >= 0 && sx + z / 2 > BAND) ctx.fillText(zoneName(i), sx + z / 2, BAND / 2);
  }
  for (let i = Math.floor((-view.y / view.k - origin.y) / zone); ; i++) {
    const sy = (origin.y + i * zone) * view.k + view.y;
    if (sy > h) break;
    ctx.moveTo(BAND - 5, Math.round(sy) + 0.5);
    ctx.lineTo(BAND, Math.round(sy) + 0.5);
    if (i >= 0 && sy + z / 2 > BAND) ctx.fillText(String(i + 1), BAND / 2, sy + z / 2);
  }
  ctx.stroke();
}

function lamp(ctx: CanvasRenderingContext2D, look: Look, x: number, y: number, color: string | null) {
  ctx.beginPath();
  ctx.arc(x, y, 4.5, 0, Math.PI * 2);
  ctx.fillStyle = color ?? look.c['lamp-off'];
  if (color) {
    ctx.shadowColor = color;
    ctx.shadowBlur = 7;
  }
  ctx.fill();
  ctx.shadowBlur = 0;
  ctx.lineWidth = 1;
  ctx.strokeStyle = look.c.frame;
  ctx.stroke();
}

let numbers: { lang: string; small: Intl.NumberFormat; big: Intl.NumberFormat } | null = null;

export function compact(n: number): string {
  const lang = typeof document === 'undefined' ? 'it' : document.documentElement.lang || 'it';
  if (numbers?.lang !== lang)
    numbers = { lang, small: new Intl.NumberFormat(lang), big: new Intl.NumberFormat(lang, { notation: 'compact', maximumFractionDigits: 1 }) };
  return n < 10000 ? numbers.small.format(Math.round(n)) : numbers.big.format(n);
}

function table(ctx: CanvasRenderingContext2D, look: Look, t: ErTable, k: number, marks: Marks, fkCols: ReadonlySet<string>) {
  const selected = marks.selected === t.id;
  const near = !selected && (marks.related?.has(t.id) ?? false);
  const dim = marks.related !== null && !marks.related.has(t.id);
  const hit = marks.hits.has(t.id);
  ctx.globalAlpha = dim ? look.dim : 1;
  const box = { x: t.x, y: t.y, w: t.w, h: t.h };
  const block = k < LOD_BLOCK;
  const solid = k < LOD_SOLID;

  if (!dim && !solid) {
    ctx.fillStyle = look.c.shadow;
    roundRect(ctx, { ...box, x: box.x + 1.5, y: box.y + 2.5 }, look.radius);
    ctx.fill();
  }
  ctx.fillStyle = look.c.card;
  roundRect(ctx, box, look.radius);
  ctx.fill();

  ctx.save();
  roundRect(ctx, box, look.radius);
  ctx.clip();
  ctx.fillStyle = hit ? look.c.hit : look.c.head;
  ctx.fillRect(t.x, t.y, t.w, solid ? t.h : HEAD);
  if (!solid) {
    ctx.strokeStyle = look.c.rule;
    ctx.lineWidth = look.frameW;
    ctx.beginPath();
    ctx.moveTo(t.x, t.y + HEAD);
    ctx.lineTo(t.x + t.w, t.y + HEAD);
    ctx.stroke();
  }
  ctx.restore();

  ctx.lineWidth = selected ? look.selW : look.frameW;
  ctx.strokeStyle = selected ? look.c.sel : look.c.frame;
  roundRect(ctx, box, look.radius);
  ctx.stroke();
  if (selected && look.selDouble) {
    ctx.lineWidth = 1;
    roundRect(ctx, { x: t.x - 4, y: t.y - 4, w: t.w + 8, h: t.h + 8 }, look.radius + 3);
    ctx.stroke();
  }

  const title = look.caps ? t.name.toUpperCase() : t.name;
  const pad = look.lamp ? 24 : 10;
  if (look.lamp) lamp(ctx, look, t.x + 12, t.y + (block ? Math.min(t.h / 2, 24) : HEAD / 2), selected ? look.c['lamp-on'] : near || hit ? look.c['lamp-near'] : null);
  ctx.fillStyle = look.c['head-ink'];
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  if (solid) {
    const size = Math.min(34, Math.max(13, 10 / k));
    if (size * k >= 4) {
      ctx.font = `${look.titleWeight} ${size}px ${look.fontTitle}`;
      ctx.fillText(fitText(ctx, title, t.w - pad - 6, size), t.x + pad, t.y + Math.min(t.h / 2, size * 1.1), t.w - pad - 6);
    }
    ctx.globalAlpha = 1;
    return;
  }
  if (block) {
    const size = Math.min(22, Math.max(13, 9 / k));
    ctx.font = `${look.titleWeight} ${size}px ${look.fontTitle}`;
    ctx.fillText(fitText(ctx, title, t.w - pad - 8, size), t.x + pad, t.y + HEAD / 2 + 1, t.w - pad - 8);
    const base = ctx.globalAlpha;
    ctx.globalAlpha = base * 0.45;
    t.relation.columns.forEach((c, i) => {
      ctx.fillStyle = c.primary_key ? look.c.pk : look.c['ink-3'];
      ctx.fillRect(t.x + 12, t.y + HEAD + i * ROW + ROW / 2 - 2, Math.min(t.w - 24, 10 + c.name.length * 6), 4);
    });
    ctx.globalAlpha = 1;
    return;
  }

  ctx.font = `${look.titleWeight} 13px ${look.fontTitle}`;
  ctx.fillText(fitText(ctx, title, t.w - pad - 56, 13), t.x + pad, t.y + HEAD / 2 + 0.5, t.w - pad - 56);
  if (t.relation.estimated_rows !== null) {
    ctx.fillStyle = look.c['ink-3'];
    if (look.c.head !== look.c.card) ctx.fillStyle = look.c['head-ink'];
    ctx.globalAlpha = (dim ? look.dim : 1) * (look.c.head !== look.c.card ? 0.7 : 1);
    ctx.font = `10.5px ${look.fontMono}`;
    ctx.textAlign = 'right';
    ctx.fillText(compact(t.relation.estimated_rows), t.x + t.w - 9, t.y + HEAD / 2 + 0.5);
    ctx.globalAlpha = dim ? look.dim : 1;
  }

  const types = k >= LOD_TYPES;
  t.relation.columns.forEach((c, i) => {
    const y = t.y + HEAD + i * ROW + ROW / 2 + 0.5;
    const fk = fkCols.has(c.name);
    if (c.primary_key || fk) {
      ctx.fillStyle = c.primary_key ? look.c.pk : look.c.fk;
      ctx.font = `600 8.5px ${look.fontMono}`;
      ctx.textAlign = 'left';
      ctx.fillText(c.primary_key ? 'PK' : 'FK', t.x + 7, y);
    }
    ctx.fillStyle = c.primary_key ? look.c.ink : look.c['ink-2'];
    ctx.font = `${c.primary_key ? '500 ' : ''}12px ${look.fontMono}`;
    ctx.textAlign = 'left';
    const nameW = Math.max(24, types ? t.w - 43 - c.type_name.length * 6.6 : t.w - 34);
    ctx.fillText(fitText(ctx, c.name, nameW, 12), t.x + 26, y, nameW);
    if (c.primary_key) ctx.fillRect(t.x + 26, y + 6.5, Math.min(c.name.length * 7.2, nameW), 0.9);
    if (types) {
      ctx.fillStyle = look.c['ink-3'];
      ctx.font = `11px ${look.fontMono}`;
      ctx.textAlign = 'right';
      ctx.fillText(c.type_name, t.x + t.w - 9, y);
    }
  });
  ctx.globalAlpha = 1;
}

export interface Scene {
  graph: ErGraph;
  areaLabels: string[];
  fkColumns: Map<string, Set<string>>;
}

export function scene(graph: ErGraph, areaLabels: string[]): Scene {
  const fkColumns = new Map<string, Set<string>>();
  for (const t of graph.tables) fkColumns.set(t.id, new Set(t.relation.foreign_keys.flatMap((f) => f.columns)));
  return { graph, areaLabels, fkColumns };
}

const EMPTY = new Set<string>();

export function sheet(ctx: CanvasRenderingContext2D, look: Look, view: View, w: number, h: number) {
  ctx.save();
  ctx.fillStyle = look.c.bg;
  ctx.fillRect(0, 0, w, h);
  ctx.translate(view.x, view.y);
  ctx.scale(view.k, view.k);
  grid(ctx, look, view, w, h);
  ctx.restore();
}

export function draw(ctx: CanvasRenderingContext2D, s: Scene, look: Look, view: View, w: number, h: number, marks: Marks, opts: { lod?: number } = {}) {
  ctx.save();
  ctx.fillStyle = look.c.bg;
  ctx.fillRect(0, 0, w, h);
  ctx.translate(view.x, view.y);
  ctx.scale(view.k, view.k);
  const k = opts.lod ?? view.k;
  grid(ctx, look, view, w, h);

  const vx0 = -view.x / view.k - 40;
  const vy0 = -view.y / view.k - 40;
  const vx1 = (w - view.x) / view.k + 40;
  const vy1 = (h - view.y) / view.k + 40;
  const inView = (r: Rect) => r.x < vx1 && r.x + r.w > vx0 && r.y < vy1 && r.y + r.h > vy0;

  if (look.wash !== 'none') {
    for (const box of areaBoxes(s.graph, marks.hidden)) if (inView(box)) wash(ctx, look, box, s.areaLabels[box.area] ?? '', k);
  }

  const g = s.graph;
  const hot = (e: ErEdge) => marks.selected !== null && (e.from === marks.selected || e.to === marks.selected);
  const cold: ErEdge[] = [];
  const warm: ErEdge[] = [];
  for (const e of g.edges) {
    if (marks.hidden.has(e.from) || marks.hidden.has(e.to) || e.route.length < 2 || !inView(e.box)) continue;
    (hot(e) ? warm : cold).push(e);
  }
  const radius = look.radius + 4;
  const strokeEdges = (list: ErEdge[], color: string, width: number) => {
    if (list.length === 0) return;
    if (look.lineCaseW > 0) {
      ctx.strokeStyle = look.c['line-case'];
      ctx.lineWidth = width + look.lineCaseW * 2;
      ctx.beginPath();
      for (const e of list) polyline(ctx, e.route, radius);
      ctx.stroke();
    }
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.beginPath();
    for (const e of list) polyline(ctx, e.route, radius);
    ctx.stroke();
    if (k < LOD_BLOCK) return;
    ctx.strokeStyle = look.lineCaseW > 0 ? look.c['line-case'] : color;
    ctx.lineWidth = look.lineCaseW > 0 ? 1.6 : Math.max(1, width);
    for (const e of list) {
      const [a, b, c, d] = [e.route[0], e.route[1], e.route.at(-2), e.route.at(-1)];
      const [childEnd, parentEnd] = edgeEnds(e);
      if (a && b) end(ctx, look, endShape(a, b, childEnd));
      if (c && d) end(ctx, look, endShape(d, c, parentEnd));
    }
  };
  ctx.lineCap = 'butt';
  ctx.lineJoin = 'round';
  if (marks.related) ctx.globalAlpha = look.dim;
  strokeEdges(cold, look.c.line, look.lineW);
  ctx.globalAlpha = 1;
  strokeEdges(warm, look.c['line-hi'], look.lineW + 0.8);

  for (const t of g.tables) {
    if (marks.hidden.has(t.id) || !inView(t)) continue;
    table(ctx, look, t, k, marks, s.fkColumns.get(t.id) ?? EMPTY);
  }
  ctx.restore();
}

export interface MiniFrame {
  scale: number;
  ox: number;
  oy: number;
}

export function miniFrame(s: Scene, hidden: ReadonlySet<string>, w: number, h: number): MiniFrame {
  const all = bounds(s.graph.tables.filter((t) => !hidden.has(t.id)), 40);
  const scale = Math.min(w / Math.max(all.w, 1), h / Math.max(all.h, 1));
  return { scale, ox: (w - all.w * scale) / 2 - all.x * scale, oy: (h - all.h * scale) / 2 - all.y * scale };
}

export function drawMinimap(ctx: CanvasRenderingContext2D, s: Scene, look: Look, w: number, h: number, marks: Marks, f: MiniFrame) {
  ctx.clearRect(0, 0, w, h);
  ctx.save();
  ctx.translate(f.ox, f.oy);
  ctx.scale(f.scale, f.scale);
  for (const t of s.graph.tables) {
    if (marks.hidden.has(t.id)) continue;
    ctx.globalAlpha = marks.related && !marks.related.has(t.id) ? 0.35 : 0.9;
    ctx.fillStyle = t.id === marks.selected ? look.c.sel : areaColor(look, t.area);
    ctx.fillRect(t.x, t.y, t.w, t.h);
  }
  ctx.restore();
}

export function drawViewport(ctx: CanvasRenderingContext2D, look: Look, f: MiniFrame, viewport: Rect) {
  ctx.strokeStyle = look.c.ink;
  ctx.lineWidth = 1.5;
  ctx.strokeRect(Math.round(viewport.x * f.scale + f.ox) + 0.5, Math.round(viewport.y * f.scale + f.oy) + 0.5, Math.max(4, viewport.w * f.scale), Math.max(4, viewport.h * f.scale));
}

export function fitView(box: Rect, w: number, h: number, margin = 40, maxK = 1.1): View {
  const k = Math.min(maxK, (w - 2 * margin) / Math.max(box.w, 1), (h - 2 * margin) / Math.max(box.h, 1));
  return { k, x: (w - box.w * k) / 2 - box.x * k, y: (h - box.h * k) / 2 - box.y * k };
}

export function hitTable(g: ErGraph, hidden: ReadonlySet<string>, p: Point): ErTable | null {
  for (let i = g.tables.length - 1; i >= 0; i--) {
    const t = g.tables[i];
    if (t && !hidden.has(t.id) && p.x >= t.x && p.x <= t.x + t.w && p.y >= t.y && p.y <= t.y + t.h) return t;
  }
  return null;
}
