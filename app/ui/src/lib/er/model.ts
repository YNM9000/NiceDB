import type { ForeignKey, Relation_Serialize as Relation, Schema_Serialize as Schema } from '../bindings';
import { Obstacles, routeAround, type End } from './route';

export const HEAD = 28;
export const ROW = 19;
export const FOOT = 7;
export const STUB = 18;
const TITLE_CHAR = 8.2;
const NAME_CHAR = 7.2;
const TYPE_CHAR = 6.6;
const MIN_W = 150;
const MAX_W = 340;
const LANES = [0, 6, -6, 12, -12];

export interface ErTable {
  id: string;
  namespace: string;
  name: string;
  relation: Relation;
  area: number;
  w: number;
  h: number;
  x: number;
  y: number;
}

export interface ErEdge {
  id: string;
  from: string;
  to: string;
  fk: ForeignKey;
  optional: boolean;
  unique: boolean;
  lane: number;
  route: Point[];
  box: Rect;
}

export interface ErArea {
  index: number;
  namespace: string;
  label: string;
  tables: string[];
}

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ErGraph {
  tables: ErTable[];
  byId: Map<string, ErTable>;
  edges: ErEdge[];
  areas: ErArea[];
  namespaces: string[];
}

export const tableId = (namespace: string, name: string) => `${namespace}.${name}`;

export function tableSize(r: Relation): { w: number; h: number } {
  const cols = r.columns.reduce((m, c) => Math.max(m, 48 + c.name.length * NAME_CHAR + c.type_name.length * TYPE_CHAR), 0);
  const title = 96 + r.name.length * TITLE_CHAR;
  return { w: Math.round(Math.min(MAX_W, Math.max(MIN_W, cols, title))), h: HEAD + Math.max(1, r.columns.length) * ROW + FOOT };
}

function covers(index: readonly string[], columns: readonly string[]) {
  return index.length === columns.length && columns.every((c) => index.includes(c));
}

export function buildGraph(schema: Schema): ErGraph {
  const tables: ErTable[] = [];
  for (const ns of schema.namespaces)
    for (const r of ns.tables) tables.push({ id: tableId(ns.name, r.name), namespace: ns.name, name: r.name, relation: r, area: 0, x: 0, y: 0, ...tableSize(r) });
  const byId = new Map(tables.map((t) => [t.id, t]));

  const edges: ErEdge[] = [];
  for (const t of tables)
    for (const fk of t.relation.foreign_keys) {
      const to = tableId(fk.ref_namespace, fk.ref_table);
      if (!byId.has(to)) continue;
      const cols = fk.columns.map((c) => t.relation.columns.find((x) => x.name === c));
      const pk = t.relation.columns.filter((c) => c.primary_key).map((c) => c.name);
      edges.push({
        id: `${t.id}:${fk.name}`,
        from: t.id,
        to,
        fk,
        optional: cols.some((c) => c?.nullable ?? true),
        unique: covers(pk, fk.columns) || t.relation.indexes.some((i) => i.unique && covers(i.columns, fk.columns)),
        lane: 0,
        route: [],
        box: { x: 0, y: 0, w: 0, h: 0 },
      });
    }
  const lanes = new Map<string, number>();
  for (const e of edges) {
    const n = lanes.get(e.to) ?? 0;
    e.lane = LANES[n % LANES.length] ?? 0;
    lanes.set(e.to, n + 1);
  }

  const areas = detectAreas(tables, edges);
  return { tables, byId, edges, areas, namespaces: schema.namespaces.filter((n) => n.tables.length > 0).map((n) => n.name) };
}

export function detectAreas(tables: ErTable[], edges: ErEdge[]): ErArea[] {
  const neighbours = new Map<string, string[]>(tables.map((t) => [t.id, []]));
  for (const e of edges) {
    if (e.from === e.to) continue;
    neighbours.get(e.from)?.push(e.to);
    neighbours.get(e.to)?.push(e.from);
  }
  const degree = (id: string) => neighbours.get(id)?.length ?? 0;
  const order = [...tables].sort((a, b) => degree(b.id) - degree(a.id) || a.id.localeCompare(b.id));
  const label = new Map(tables.map((t) => [t.id, t.id]));
  for (let round = 0; round < 24; round++) {
    let changed = false;
    for (const t of order) {
      const counts = new Map<string, number>();
      for (const n of neighbours.get(t.id) ?? []) {
        const nt = label.get(n);
        if (nt && n.startsWith(`${t.namespace}.`)) counts.set(nt, (counts.get(nt) ?? 0) + 1);
      }
      let best = label.get(t.id) ?? t.id;
      let bestCount = counts.get(best) ?? 0;
      for (const [l, c] of counts)
        if (c > bestCount || (c === bestCount && l < best)) {
          best = l;
          bestCount = c;
        }
      if (best !== label.get(t.id)) {
        label.set(t.id, best);
        changed = true;
      }
    }
    if (!changed) break;
  }

  const groups = new Map<string, ErTable[]>();
  for (const t of tables) {
    const key = degree(t.id) === 0 ? `${t.namespace}\u0000` : (label.get(t.id) ?? t.id);
    const list = groups.get(key);
    if (list) list.push(t);
    else groups.set(key, [t]);
  }
  const areas = [...groups.entries()]
    .map(([key, members]) => {
      const hub = [...members].sort((a, b) => degree(b.id) - degree(a.id) || a.name.localeCompare(b.name))[0];
      return { key, members, namespace: members[0]?.namespace ?? '', label: key.endsWith('\u0000') ? '' : (hub?.name ?? '') };
    })
    .sort((a, b) => a.namespace.localeCompare(b.namespace) || b.members.length - a.members.length || a.label.localeCompare(b.label));
  return areas.map((a, index) => {
    for (const t of a.members) t.area = index;
    return { index, namespace: a.namespace, label: a.label, tables: a.members.map((t) => t.id) };
  });
}

export function rowY(t: ErTable, column: string): number {
  const i = t.relation.columns.findIndex((c) => c.name === column);
  return i < 0 ? t.y + HEAD / 2 : t.y + HEAD + i * ROW + ROW / 2;
}

export function simpleRoute(child: ErTable, parent: ErTable, fk: ForeignKey, lane = 0): Point[] {
  const y1 = rowY(child, fk.columns[0] ?? '');
  const y2 = rowY(parent, fk.ref_columns[0] ?? '');
  if (child === parent) {
    const x = child.x + child.w;
    return [{ x, y: y1 }, { x: x + STUB * 1.5, y: y1 }, { x: x + STUB * 1.5, y: y2 }, { x, y: y2 }];
  }
  const gap = 2 * STUB;
  if (parent.x + parent.w + gap <= child.x || child.x + child.w + gap <= parent.x) {
    const childLeft = parent.x < child.x;
    const x1 = childLeft ? child.x : child.x + child.w;
    const x2 = childLeft ? parent.x + parent.w : parent.x;
    const mid = Math.round((x1 + x2) / 2) + lane;
    return [{ x: x1, y: y1 }, { x: mid, y: y1 }, { x: mid, y: y2 }, { x: x2, y: y2 }];
  }
  const right = Math.max(child.x + child.w, parent.x + parent.w) + STUB + Math.abs(lane);
  const left = Math.min(child.x, parent.x) - STUB - Math.abs(lane);
  const useRight = Math.abs(child.x + child.w - (parent.x + parent.w)) <= Math.abs(child.x - parent.x);
  const x = useRight ? right : left;
  const cx = useRight ? child.x + child.w : child.x;
  const px = useRight ? parent.x + parent.w : parent.x;
  return [{ x: cx, y: y1 }, { x, y: y1 }, { x, y: y2 }, { x: px, y: y2 }];
}

const end = (t: ErTable, owner: number, y: number, dir: 1 | -1, stub: number): End => ({ side: { x: dir === 1 ? t.x + t.w : t.x, y }, dir, stub, owner });

const parentStub = (lane: number) => STUB + 2 * Math.abs(lane) + (lane < 0 ? 6 : 0);

export function route(child: ErTable, parent: ErTable, fk: ForeignKey, lane = 0, obs?: Obstacles, index?: Map<string, number>): Point[] {
  if (child === parent || !obs || !index) return simpleRoute(child, parent, fk, lane);
  const ci = index.get(child.id) ?? -1;
  const pi = index.get(parent.id) ?? -1;
  const y1 = rowY(child, fk.columns[0] ?? '');
  const y2 = rowY(parent, fk.ref_columns[0] ?? '');
  const ps = parentStub(lane);
  const gap = 2 * STUB;
  const sides: [1 | -1, 1 | -1][] =
    parent.x + parent.w + gap <= child.x
      ? [[-1, 1]]
      : child.x + child.w + gap <= parent.x
        ? [[1, -1]]
        : [
            [1, 1],
            [-1, -1],
          ];
  let best: { pts: Point[]; cost: number } | null = null;
  for (const [cd, pd] of sides) {
    const r = routeAround(obs, end(child, ci, y1, cd, STUB), end(parent, pi, y2, pd, ps));
    if (r && (!best || r.cost < best.cost)) best = r;
  }
  return best?.pts ?? simpleRoute(child, parent, fk, lane);
}

export function obstacles(g: ErGraph): { obs: Obstacles; index: Map<string, number> } {
  return { obs: new Obstacles(g.tables), index: new Map(g.tables.map((t, i) => [t.id, i])) };
}

function setRoute(e: ErEdge, pts: Point[]) {
  e.route = pts;
  e.box = bounds(pts.map((q) => ({ x: q.x, y: q.y, w: 0, h: 0 })), 16);
}

export function routeAll(g: ErGraph, only?: ReadonlySet<string>) {
  const { obs, index } = obstacles(g);
  for (const e of g.edges) {
    if (only && !only.has(e.from) && !only.has(e.to)) continue;
    const c = g.byId.get(e.from);
    const p = g.byId.get(e.to);
    if (c && p) setRoute(e, route(c, p, e.fk, e.lane, obs, index));
  }
}

export function routeNear(g: ErGraph, id: string) {
  const { obs, index } = obstacles(g);
  const at = index.get(id) ?? -1;
  const behind = (e: ErEdge) => e.route.some((p, i) => {
    const q = e.route[i + 1];
    return !!q && obs.crossed(p, q).includes(at);
  });
  for (const e of g.edges) {
    if (e.from !== id && e.to !== id && !behind(e)) continue;
    const c = g.byId.get(e.from);
    const p = g.byId.get(e.to);
    if (c && p) setRoute(e, route(c, p, e.fk, e.lane, obs, index));
  }
}

export function areaLabels(g: ErGraph, loose: string): string[] {
  const linked = new Map<string, number>();
  for (const a of g.areas) if (a.label) linked.set(a.namespace, (linked.get(a.namespace) ?? 0) + 1);
  return g.areas.map((a) => {
    if (!a.label) return loose;
    return linked.get(a.namespace) === 1 ? a.namespace : a.label;
  });
}

export function incoming(g: ErGraph, id: string): ErEdge[] {
  return g.edges.filter((e) => e.to === id);
}

export function outgoing(g: ErGraph, id: string): ErEdge[] {
  return g.edges.filter((e) => e.from === id);
}

export function indexed(r: Relation, columns: readonly string[]): boolean {
  const leads = (cols: readonly string[]) => columns.length > 0 && covers(cols.slice(0, columns.length), columns);
  return leads(r.columns.filter((c) => c.primary_key).map((c) => c.name)) || r.indexes.some((i) => leads(i.columns));
}

export function bounds(rects: Iterable<Rect>, pad = 0): Rect {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const r of rects) {
    x0 = Math.min(x0, r.x);
    y0 = Math.min(y0, r.y);
    x1 = Math.max(x1, r.x + r.w);
    y1 = Math.max(y1, r.y + r.h);
  }
  if (x0 === Infinity) return { x: 0, y: 0, w: 0, h: 0 };
  return { x: x0 - pad, y: y0 - pad, w: x1 - x0 + 2 * pad, h: y1 - y0 + 2 * pad };
}

export function neighbours(g: ErGraph, id: string): Set<string> {
  const out = new Set<string>([id]);
  for (const e of g.edges) {
    if (e.from === id) out.add(e.to);
    if (e.to === id) out.add(e.from);
  }
  return out;
}

export function search(g: ErGraph, query: string, limit = 8): ErTable[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const score = (t: ErTable) => {
    const n = t.name.toLowerCase();
    if (n === q) return 0;
    if (n.startsWith(q)) return 1;
    if (n.includes(q)) return 2;
    if (t.relation.columns.some((c) => c.name.toLowerCase().includes(q))) return 3;
    return 9;
  };
  return g.tables
    .map((t) => ({ t, s: score(t) }))
    .filter((x) => x.s < 9)
    .sort((a, b) => a.s - b.s || a.t.name.length - b.t.name.length || a.t.id.localeCompare(b.t.id))
    .slice(0, limit)
    .map((x) => x.t);
}
