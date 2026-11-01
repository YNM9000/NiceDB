import type { Api } from './api';
import type { Summary_Deserialize as Summary, SummarySource_Deserialize as SummarySource } from './bindings';

type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

const MOCK_ROWS = 200_000;

function source(tables: Record<string, Gen>, src: SummarySource): Gen | null {
  const name = src.kind === 'table' ? src.browse.table.name : (/\bfrom\s+(?:\w+\.)?"?(\w+)"?/i.exec(src.sql)?.[1] ?? '');
  return tables[name] ?? null;
}

function* rowsOf(t: Gen) {
  const n = Math.min(t.total, MOCK_ROWS);
  for (let i = 1; i <= n; i++) yield t.row(i);
}

const fixed = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2));

export function summarize(t: Gen, s: Summary): (string | null)[][] {
  const at = (name: string) => t.columns.findIndex(([c]) => c === name);
  if (s.kind === 'stats') {
    const acc = s.columns.map(() => ({ count: 0, sum: 0, min: Infinity, max: -Infinity, seen: new Set<string>() }));
    let rows = 0;
    for (const r of rowsOf(t)) {
      rows++;
      s.columns.forEach((c, k) => {
        const v = r[at(c.name)] ?? null;
        const a = acc[k];
        if (v === null || !a) return;
        a.count++;
        if (c.numeric) {
          const n = Number(v);
          a.sum += n;
          a.min = Math.min(a.min, n);
          a.max = Math.max(a.max, n);
        } else a.seen.add(v);
      });
    }
    const out: (string | null)[] = [String(rows)];
    s.columns.forEach((c, k) => {
      const a = acc[k];
      if (!a) return;
      out.push(String(a.count));
      if (c.numeric) out.push(a.count ? fixed(a.sum) : null, a.count ? fixed(a.min) : null, a.count ? fixed(a.max) : null);
      else out.push(String(a.seen.size));
    });
    return [out];
  }
  const xi = at(s.x);
  const groups = new Map<string | null, { n: number; sums: number[]; counts: number[]; mins: number[]; maxs: number[] }>();
  for (const r of rowsOf(t)) {
    const x = r[xi] ?? null;
    let g = groups.get(x);
    if (!g) groups.set(x, (g = { n: 0, sums: s.values.map(() => 0), counts: s.values.map(() => 0), mins: s.values.map(() => Infinity), maxs: s.values.map(() => -Infinity) }));
    g.n++;
    s.values.forEach((v, k) => {
      const raw = v.column ? (r[at(v.column)] ?? null) : null;
      if (v.column && raw === null) return;
      const n = Number(raw);
      g.counts[k] = (g.counts[k] ?? 0) + 1;
      g.sums[k] = (g.sums[k] ?? 0) + n;
      g.mins[k] = Math.min(g.mins[k] ?? Infinity, n);
      g.maxs[k] = Math.max(g.maxs[k] ?? -Infinity, n);
    });
  }
  return [...groups]
    .slice(0, s.limit)
    .map(([x, g]) => [
      x,
      ...s.values.map((v, k) => {
        const c = g.counts[k] ?? 0;
        if (v.func === 'count') return String(v.column ? c : g.n);
        if (!c) return null;
        const f = { sum: g.sums[k] ?? 0, avg: (g.sums[k] ?? 0) / c, min: g.mins[k] ?? 0, max: g.maxs[k] ?? 0 }[v.func];
        return fixed(f);
      }),
    ]);
}

const COUNTS: [string, number][] = [
  ['gestionale', 5],
  ['gestionale_archivio', 2],
  ['postgres', 0],
  ['template0', 0],
  ['template1', 0],
];

export function extras6Mock(tables: Record<string, Gen>, params: URLSearchParams): Pick<Api, 'dataSummarize' | 'databaseCounts'> {
  return {
    databaseCounts: async () => (params.has('counts') ? (await new Promise((r) => setTimeout(r, 80)), COUNTS) : []),
    dataSummarize: async (_sid, src, s) => {
      await new Promise((r) => setTimeout(r, 120));
      const t = source(tables, src);
      if (!t) throw { code: 'unsortable', message: 'not_select' };
      return summarize(t, s);
    },
  };
}
