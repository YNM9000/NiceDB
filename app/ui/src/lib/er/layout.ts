import type { ElkNode } from 'elkjs/lib/elk-api';
import type { ErLayout } from '../bindings';
import { bounds, type ErGraph } from './model';

const AREA_PAD = 36;
const GAP = 48;

export function toElk(g: ErGraph, aspect = 1.6): ElkNode {
  const inArea = new Map(g.tables.map((t) => [t.id, t.area]));
  const edgesOf = new Map<number, { id: string; sources: string[]; targets: string[] }[]>();
  for (const e of g.edges) {
    const area = inArea.get(e.from);
    if (e.from === e.to || area === undefined || area !== inArea.get(e.to)) continue;
    const list = edgesOf.get(area);
    const edge = { id: e.id, sources: [e.to], targets: [e.from] };
    if (list) list.push(edge);
    else edgesOf.set(area, [edge]);
  }
  return {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'rectpacking',
      'elk.spacing.nodeNode': String(GAP * 1.5),
      'elk.aspectRatio': String(aspect),
      'elk.padding': '[top=0,left=0,bottom=0,right=0]',
    },
    children: g.areas.map((a) => ({
      id: `area:${a.index}`,
      layoutOptions: {
        'elk.algorithm': a.label ? 'layered' : 'rectpacking',
        'elk.direction': 'RIGHT',
        'elk.aspectRatio': String(Math.max(0.8, aspect * 0.9)),
        'elk.edgeRouting': 'ORTHOGONAL',
        'elk.padding': `[top=${AREA_PAD},left=${AREA_PAD},bottom=${AREA_PAD},right=${AREA_PAD}]`,
        'elk.spacing.nodeNode': String(GAP / 2),
        'elk.layered.spacing.nodeNodeBetweenLayers': String(GAP * 1.6),
        'elk.layered.wrapping.strategy': 'MULTI_EDGE',
        'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
        'elk.separateConnectedComponents': 'true',
      },
      children: a.tables.flatMap((id) => {
        const t = g.byId.get(id);
        return t ? [{ id, width: t.w, height: t.h }] : [];
      }),
      edges: edgesOf.get(a.index) ?? [],
    })),
  };
}

export function applyElk(g: ErGraph, result: ElkNode) {
  for (const area of result.children ?? [])
    for (const n of area.children ?? []) {
      const t = g.byId.get(n.id);
      if (!t) continue;
      t.x = Math.round((area.x ?? 0) + (n.x ?? 0));
      t.y = Math.round((area.y ?? 0) + (n.y ?? 0));
    }
}

export function toSaved(g: ErGraph): ErLayout {
  return { tables: g.tables.map((t) => ({ namespace: t.namespace, table: t.name, x: t.x, y: t.y })) };
}

export function applySaved(g: ErGraph, saved: ErLayout): boolean {
  const known = new Map(saved.tables.map((p) => [`${p.namespace}.${p.table}`, p]));
  const placed = g.tables.filter((t) => {
    const p = known.get(t.id);
    if (!p || p.x === null || p.y === null) return false;
    t.x = p.x;
    t.y = p.y;
    return true;
  });
  if (placed.length === 0) return false;
  const done = new Set(placed);
  const box = bounds(placed);
  let x = box.x;
  let y = box.y + box.h + GAP * 2;
  let rowH = 0;
  for (const t of g.tables) {
    if (done.has(t)) continue;
    if (x > box.x && x + t.w > box.x + Math.max(box.w, 1200)) {
      x = box.x;
      y += rowH + GAP;
      rowH = 0;
    }
    t.x = x;
    t.y = y;
    x += t.w + GAP;
    rowH = Math.max(rowH, t.h);
  }
  return true;
}
