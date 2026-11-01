import { m } from '../paraglide/messages';
import { getLocale } from '../paraglide/runtime';
import type { Finding_Serialize, NodeKind, Node_Serialize, PlanReport_Serialize, Plan_Serialize } from './bindings';

export type Report = PlanReport_Serialize;
export type Plan = Plan_Serialize;
export type PlanNode = Node_Serialize;
export type Finding = Finding_Serialize;

const tag = () => (getLocale() === 'it' ? 'it-IT' : 'en-GB');
const fmt = (digits: number) => new Intl.NumberFormat(tag(), { maximumFractionDigits: digits, minimumFractionDigits: digits });

export function ms(v: number | null | undefined): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return '—';
  if (v >= 1000) return `${fmt(2).format(v / 1000)} s`;
  if (v >= 100) return `${fmt(0).format(v)} ms`;
  if (v >= 1) return `${fmt(1).format(v)} ms`;
  return `${fmt(2).format(v)} ms`;
}

export function count(v: number | null | undefined): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return '—';
  return new Intl.NumberFormat(tag(), { maximumFractionDigits: v < 10 ? 1 : 0 }).format(v);
}

export function short(v: number | null | undefined): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return '—';
  if (v < 100_000) return count(v);
  return new Intl.NumberFormat(tag(), { notation: 'compact', maximumFractionDigits: 1 }).format(v);
}

export function rows(v: number | null | undefined, format: (v: number | null | undefined) => string = count): string {
  return v === 1 ? m.plan_rows_one() : m.plan_rows({ count: format(v) });
}

export function pct(share: number): string {
  if (share > 0 && share < 0.01) return '<1%';
  return `${Math.round(share * 100)}%`;
}

export function factor(v: number): string {
  return v >= 100 ? count(Math.round(v)) : fmt(v < 10 ? 1 : 0).format(v);
}

const KIND: Record<NodeKind, () => string> = {
  full_scan: m.plan_kind_full_scan,
  index_scan: m.plan_kind_index_scan,
  index_only_scan: m.plan_kind_index_only_scan,
  bitmap_scan: m.plan_kind_bitmap_scan,
  bitmap_index: m.plan_kind_bitmap_index,
  hash_join: m.plan_kind_hash_join,
  nested_loop: m.plan_kind_nested_loop,
  merge_join: m.plan_kind_merge_join,
  hash: m.plan_kind_hash,
  aggregate: m.plan_kind_aggregate,
  sort: m.plan_kind_sort,
  limit: m.plan_kind_limit,
  gather: m.plan_kind_gather,
  materialize: m.plan_kind_materialize,
  subquery: m.plan_kind_subquery,
  append: m.plan_kind_append,
  window: m.plan_kind_window,
  unique: m.plan_kind_unique,
  modify: m.plan_kind_modify,
  result: m.plan_kind_result,
  other: m.plan_kind_other,
};

export function label(n: PlanNode): string {
  const op = KIND[n.kind]();
  const name = n.kind === 'other' ? n.op : op;
  return n.relation ? m.plan_op_on({ op: name, name: n.relation }) : name;
}

export function native(n: PlanNode): string {
  const target = n.relation ? [n.relation, n.alias && n.alias !== n.relation ? n.alias : null].filter(Boolean).join(' ') : null;
  return [n.op, target, n.index ? `· ${n.index}` : null].filter(Boolean).join(' ');
}

export function subject(n: PlanNode): string {
  if (n.relation) return [n.relation, n.alias && n.alias !== n.relation ? n.alias : null].filter(Boolean).join(' ');
  const cond = n.condition ?? n.filter;
  if (cond) return cond.replace(/^\((.*)\)$/, '$1');
  const key = n.details.find((d) => d.key === 'Sort Key' || d.key === 'Group Key');
  return key?.value ?? n.index ?? '';
}

export type Glyph = 'scan' | 'index' | 'join' | 'aggregate' | 'sort' | 'other';

export function glyph(kind: NodeKind): Glyph {
  if (kind === 'full_scan') return 'scan';
  if (kind === 'index_scan' || kind === 'index_only_scan' || kind === 'bitmap_scan' || kind === 'bitmap_index') return 'index';
  if (kind === 'hash_join' || kind === 'nested_loop' || kind === 'merge_join') return 'join';
  if (kind === 'aggregate' || kind === 'unique' || kind === 'window') return 'aggregate';
  if (kind === 'sort' || kind === 'limit') return 'sort';
  return 'other';
}

export function weights(plan: Plan): number[] {
  if (plan.analyzed) return plan.nodes.map((n) => Math.max(0, n.self_ms ?? 0));
  return plan.nodes.map((n) => {
    const kids = n.children.reduce((s, c) => s + (plan.nodes[c]?.total_cost ?? 0), 0);
    return Math.max(0, (n.total_cost ?? 0) - kids);
  });
}

export function shares(plan: Plan): number[] {
  const w = weights(plan);
  const total = w.reduce((a, b) => a + b, 0);
  return w.map((x) => (total > 0 ? x / total : 0));
}

export function hottest(plan: Plan): number | null {
  if (plan.slowest !== null && plan.slowest !== undefined) return plan.slowest;
  const w = weights(plan);
  let best = -1;
  w.forEach((x, i) => {
    if (x > 0 && (best < 0 || x > (w[best] ?? 0))) best = i;
  });
  return best < 0 ? null : best;
}

export interface Segment {
  id: number | null;
  label: string;
  value: number;
  share: number;
  hot: boolean;
}

export function segments(plan: Plan, keep = 3): Segment[] {
  const w = weights(plan);
  const total = w.reduce((a, b) => a + b, 0);
  if (total <= 0) return [];
  const hot = hottest(plan);
  const order = w
    .map((value, id) => ({ id, value }))
    .filter((x) => x.value > 0)
    .sort((a, b) => b.value - a.value);
  const top = order.slice(0, keep).filter((x, i) => i === 0 || x.value / total >= 0.02);
  const out: Segment[] = top.map((x) => {
    const n = plan.nodes[x.id];
    return { id: x.id, label: n ? label(n) : '', value: x.value, share: x.value / total, hot: x.id === hot };
  });
  const rest = total - top.reduce((s, x) => s + x.value, 0);
  if (rest > 0) out.push({ id: null, label: m.plan_other(), value: rest, share: rest / total, hot: false });
  return out;
}

export interface Box {
  id: number;
  n: number;
  x: number;
  y: number;
  col: number;
  hot: boolean;
  worst: boolean;
  share: number;
}

export interface Edge {
  from: number;
  to: number | null;
  d: string;
  width: number;
  hot: boolean;
}

export interface Layout {
  boxes: Box[];
  edges: Edge[];
  width: number;
  height: number;
  exit: { x: number; y: number };
  cloud: string | null;
}

export const BOX = { w: 164, h: 66, gapX: 34, gapY: 16, pad: 14, exit: 96 };

export function thickness(rows: number | null | undefined): number {
  const r = Math.max(0, rows ?? 0);
  return Math.round(Math.min(8, 1.5 + Math.log10(r + 1) * 1.05) * 10) / 10;
}

export function cloud(x0: number, y0: number, x1: number, y1: number, r: number): string {
  const pts: [number, number, number][] = [];
  const side = (ax: number, ay: number, bx: number, by: number) => {
    const len = Math.hypot(bx - ax, by - ay);
    const steps = Math.max(2, Math.round(len / (r * 1.7)));
    for (let s = 1; s <= steps; s++) pts.push([ax + ((bx - ax) * s) / steps, ay + ((by - ay) * s) / steps, len / steps]);
  };
  side(x0, y0, x1, y0);
  side(x1, y0, x1, y1);
  side(x1, y1, x0, y1);
  side(x0, y1, x0, y0);
  return pts.reduce((d, [x, y, l]) => `${d}A${(l * 0.62).toFixed(1)} ${(l * 0.62).toFixed(1)} 0 0 1 ${x.toFixed(1)} ${y.toFixed(1)}`, `M${x0} ${y0}`);
}

export function drawnChildren(plan: Plan): Map<number, number[]> {
  const folds = (n: PlanNode | undefined) => !!n && n.kind === 'hash' && n.children.length === 1 && n.id !== 0;
  const visible = (id: number, depth: number): number[] => {
    const n = plan.nodes[id];
    if (!n) return [];
    return folds(n) && depth < 400 ? n.children.flatMap((c) => visible(c, depth + 1)) : [id];
  };
  const kids = new Map<number, number[]>();
  for (const n of plan.nodes) if (!folds(n)) kids.set(n.id, n.children.flatMap((c) => visible(c, 0)));
  return kids;
}

export function layout(plan: Plan): Layout {
  const kids = drawnChildren(plan);
  if (!plan.nodes.length) return { boxes: [], edges: [], width: 0, height: 0, exit: { x: 0, y: 0 }, cloud: null };
  const share = shares(plan);
  const hot = hottest(plan);
  const trunk: number[] = [0];
  for (let id = 0, k = kids.get(0) ?? []; k.length === 1 && trunk.length < 400; k = kids.get(id) ?? []) {
    id = k[0] ?? 0;
    trunk.push(id);
  }
  const branch = trunk.pop() ?? 0;
  const col = new Map<number, number>();
  const row = new Map<number, number>();
  let leaves = 0;
  const place = (id: number, depth: number): number => {
    const k = depth > 400 ? [] : (kids.get(id) ?? []);
    if (!k.length) {
      col.set(id, 0);
      row.set(id, leaves);
      return leaves++;
    }
    const rows = k.map((c) => place(c, depth + 1));
    col.set(id, Math.max(...k.map((c) => col.get(c) ?? 0)) + 1);
    const r = ((rows[0] ?? 0) + (rows[rows.length - 1] ?? 0)) / 2;
    row.set(id, r);
    return r;
  };
  place(branch, 0);
  const trunkCol = (col.get(branch) ?? 0) + 1;
  const bottom = Math.max(row.get(branch) ?? 0, trunk.length - 1);
  trunk.forEach((id, i) => {
    col.set(id, trunkCol);
    row.set(id, bottom - (trunk.length - 1 - i));
  });
  const stepX = BOX.w + BOX.gapX;
  const stepY = BOX.h + BOX.gapY;
  const boxes: Box[] = [...row.keys()]
    .sort((a, b) => a - b)
    .map((id) => ({
      id,
      n: id + 1,
      x: BOX.pad + (col.get(id) ?? 0) * stepX,
      y: BOX.pad + (row.get(id) ?? 0) * stepY,
      col: col.get(id) ?? 0,
      hot: id === hot,
      worst: id === plan.worst_estimate,
      share: share[id] ?? 0,
    }));
  const at = new Map(boxes.map((b) => [b.id, b]));
  const rowsOf = (id: number) => {
    const n = plan.nodes[id];
    return thickness(n ? (n.rows ?? n.est_rows) : 0);
  };
  const edges: Edge[] = [];
  for (const b of boxes) {
    for (const c of kids.get(b.id) ?? []) {
      const child = at.get(c);
      if (!child) continue;
      let d: string;
      if (child.col === b.col) {
        const x = b.x + BOX.w / 2;
        d = `M${x} ${child.y}V${b.y + BOX.h}`;
      } else {
        const x0 = child.x + BOX.w;
        const y0 = child.y + BOX.h / 2;
        const y1 = b.y + BOX.h / 2;
        const mid = b.x - BOX.gapX / 2;
        d = y0 === y1 ? `M${x0} ${y0}H${b.x}` : `M${x0} ${y0}H${mid}V${y1}H${b.x}`;
      }
      edges.push({ from: c, to: b.id, d, width: rowsOf(c), hot: c === hot });
    }
  }
  const root = at.get(0);
  const exit = { x: BOX.pad + trunkCol * stepX + BOX.w + 26, y: root ? root.y + BOX.h / 2 : BOX.pad };
  if (root) edges.push({ from: 0, to: null, d: `M${root.x + BOX.w} ${exit.y}H${exit.x}`, width: rowsOf(0), hot: hot === 0 });
  const rows = Math.max(leaves, bottom + 1);
  const hotBox = hot !== null ? at.get(hot) : undefined;
  return {
    boxes,
    edges,
    width: exit.x + BOX.exit + BOX.pad,
    height: BOX.pad * 2 + rows * stepY - BOX.gapY,
    exit,
    cloud: hotBox ? cloud(hotBox.x - 8, hotBox.y - 8, hotBox.x + BOX.w + 8, hotBox.y + BOX.h + 8, 7) : null,
  };
}

export function clip(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, Math.max(1, max - 1))}…` : text;
}

export interface NodeDelta {
  before: number | null;
  after: number | null;
  ratio: number | null;
}

export interface Comparison {
  before: number | null;
  after: number | null;
  ratio: number | null;
  nodes: Map<number, NodeDelta>;
}

function signature(plan: Plan, n: PlanNode): string {
  const path: string[] = [];
  let p = n.parent;
  let guard = 0;
  while (p !== null && p !== undefined && guard++ < 400) {
    const parent = plan.nodes[p];
    if (!parent) break;
    path.push(parent.kind);
    p = parent.parent;
  }
  return path.join('<');
}

const ratio = (before: number | null, after: number | null) => (before !== null && after !== null && after > 0 && before > 0 ? before / after : null);

export function compare(previous: Plan, current: Plan): Comparison {
  const wp = weights(previous);
  const wc = weights(current);
  const free = new Set(previous.nodes.map((n) => n.id));
  const score = (a: PlanNode, b: PlanNode) => {
    if (a.relation && a.relation === b.relation) return a.kind === b.kind ? 6 : 5;
    if (a.relation || b.relation) return 0;
    if (a.kind !== b.kind) return 0;
    return signature(current, a) === signature(previous, b) ? 4 : a.depth === b.depth ? 3 : 2;
  };
  const nodes = new Map<number, NodeDelta>();
  for (const pass of [6, 5, 4, 3, 2]) {
    for (const a of current.nodes) {
      if (nodes.has(a.id)) continue;
      const b = previous.nodes.find((x) => free.has(x.id) && score(a, x) === pass);
      if (!b) continue;
      free.delete(b.id);
      const before = wp[b.id] ?? null;
      const after = wc[a.id] ?? null;
      nodes.set(a.id, { before, after, ratio: ratio(before, after) });
    }
  }
  const total = (p: Plan, w: number[]) => p.total_ms ?? (p.analyzed ? w.reduce((s, x) => s + x, 0) : (p.nodes[0]?.total_cost ?? null));
  const before = total(previous, wp);
  const after = total(current, wc);
  return { before, after, ratio: ratio(before, after), nodes };
}

export function buffersRead(plan: Plan): number | null {
  const b = plan.nodes[0]?.buffers;
  if (!b) return null;
  return (b.hit ?? 0) + (b.read ?? 0);
}

export interface Text {
  title: string;
  body: string;
}

const SPILL: Record<string, () => string> = {
  sort: m.plan_spill_sort,
  hash: m.plan_spill_hash,
  aggregate: m.plan_spill_aggregate,
};

export function findingText(f: Finding): Text {
  const table = f.table ?? '?';
  const column = f.column ?? '?';
  const rows = count(f.rows);
  const kept = count(f.kept);
  switch (f.rule) {
    case 'seq_scan':
      return {
        title: m.plan_f_seq_scan_title({ table }),
        body: f.column ? m.plan_f_seq_scan_body({ table, column, rows, kept }) : m.plan_f_seq_scan_body_nocol({ table, rows, kept }),
      };
    case 'fk_without_index':
      return { title: m.plan_f_fk_title({ table, column }), body: m.plan_f_fk_body({ table, column, ref: f.wrapper ?? '?', rows, kept }) };
    case 'row_estimate':
      return {
        title: m.plan_f_estimate_title({ factor: factor(f.factor ?? 0) }),
        body: f.table ? m.plan_f_estimate_body_table({ est: kept, rows, table }) : m.plan_f_estimate_body({ est: kept, rows }),
      };
    case 'spill': {
      const what = (SPILL[f.wrapper ?? 'sort'] ?? m.plan_spill_sort)();
      const kb = f.kb ?? null;
      return {
        title: m.plan_f_spill_title({ what }),
        body: kb !== null ? m.plan_f_spill_body({ size: kb >= 1024 ? `${count(kb / 1024)} MB` : `${count(kb)} kB` }) : m.plan_f_spill_body_nosize(),
      };
    }
    case 'nested_loop':
      return { title: m.plan_f_loop_title({ loops: count(f.factor) }), body: m.plan_f_loop_body({ table, loops: count(f.factor), rows }) };
    case 'function_on_indexed_column':
      return { title: m.plan_f_function_title({ fn: f.wrapper ?? '?', column }), body: m.plan_f_function_body({ fn: f.wrapper ?? '?', column, table, rows }) };
    case 'implicit_cast':
      return { title: m.plan_f_cast_title({ column }), body: m.plan_f_cast_body({ table, column, type: f.wrapper ?? '?' }) };
    case 'select_star_sort':
      return { title: m.plan_f_star_title(), body: m.plan_f_star_body({ rows, width: count(f.factor), table }) };
    case 'missing_limit':
      return { title: m.plan_f_limit_title(), body: m.plan_f_limit_body({ rows }) };
    case 'docs_examined':
      return { title: m.plan_f_docs_title({ table, factor: count(f.factor) }), body: m.plan_f_docs_body({ table, index: f.wrapper ?? '?', column, rows, kept }) };
    case 'in_memory_sort':
      return {
        title: m.plan_f_memsort_title({ column }),
        body: f.kb !== null && f.kb !== undefined ? m.plan_f_memsort_body({ rows, table, size: f.kb >= 1024 ? `${count(f.kb / 1024)} MB` : `${count(f.kb)} kB` }) : m.plan_f_memsort_body_nosize({ rows, table }),
      };
  }
}
