import type { SearchDone, SearchEvent, SearchPlanned, SearchSpec, SearchTable, ValueSearch } from './api';
import type { TableHits } from './bindings';
import { cellMatches, isNumber } from './search';

type Gen = {
  columns: [string, string][];
  total: number;
  row: (i: number) => (string | null)[];
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const SCAN = 30_000;

function family(type: string): 'text' | 'integer' | 'decimal' | 'other' | 'skip' {
  const t = type.toLowerCase();
  if (/^(text|varchar|bpchar|char|citext|name)/.test(t)) return 'text';
  if (/^(int|smallint|bigint|serial)/.test(t)) return 'integer';
  if (/^(numeric|decimal|float|real|double)/.test(t)) return 'decimal';
  if (/^(bytea|blob)/.test(t)) return 'skip';
  return 'other';
}

function searchable(spec: SearchSpec, type: string): boolean {
  if (!spec.value) return false;
  const number = isNumber(spec.value);
  switch (family(type)) {
    case 'text':
      return spec.kinds.text;
    case 'other':
      return spec.kinds.other;
    case 'integer':
      return spec.kinds.number && number && (spec.mode === 'contains' || !/[.,]/.test(spec.value));
    case 'decimal':
      return spec.kinds.number && number;
    default:
      return false;
  }
}

const columnsOf = (spec: SearchSpec, t: SearchTable) => t.columns.filter((c) => searchable(spec, c.type_name)).map((c) => c.name);

export function searchMock(tables: Record<string, Gen>, chain: (id: string) => Promise<void>) {
  const stops = new Map<string, () => void>();
  const pace = () => (typeof location !== 'undefined' && new URLSearchParams(location.search).has('slow') ? 900 : 70);
  return {
    valueSearchPlan: async (_sid: string, spec: SearchSpec, list: SearchTable[]): Promise<SearchPlanned> => {
      const columns = list.map((t) => columnsOf(spec, t));
      const first = list.findIndex((_, i) => (columns[i]?.length ?? 0) > 0);
      const t = list[first];
      const cols = columns[first] ?? [];
      const q = spec.value.replaceAll("'", "''");
      const test = (c: string) => (spec.mode === 'exact' ? `lower("${c}"::text) = lower('${q}')` : `strpos(lower("${c}"::text), lower('${q}')) > 0`);
      const sample = t
        ? `SELECT count(*)::text, ${cols.map((c) => `count(CASE WHEN ${test(c)} THEN 1 END)::text`).join(', ')} FROM "${t.table.namespace}"."${t.table.name}" WHERE (${cols.map(test).join(' OR ')})`
        : null;
      return { columns, sample };
    },
    valueSearch: async (req: ValueSearch, onEvent: (e: SearchEvent) => void): Promise<SearchDone> => {
      let stopped = false;
      stops.set(req.job_id, () => (stopped = true));
      const started = Date.now();
      const plan = req.tables.map((t) => ({ t, cols: columnsOf(req.spec, t) })).filter((p) => p.cols.length);
      const done: SearchDone = {
        searched: 0,
        of: plan.length,
        with_hits: 0,
        rows: 0,
        failed: 0,
        cancelled: false,
        out_of_time: false,
        elapsed_ms: 0,
      };
      for (const [index, { t, cols }] of plan.entries()) {
        if (stopped) {
          done.cancelled = true;
          break;
        }
        if (Date.now() - started > req.budget_secs * 1000) {
          done.out_of_time = true;
          break;
        }
        onEvent({ kind: 'table', index, of: plan.length, name: t.table.name });
        await sleep(pace());
        done.searched++;
        const gen = tables[t.table.name];
        if (!gen) continue;
        const names = gen.columns.map(([n]) => n);
        const at = cols.map((c) => names.indexOf(c));
        const counts = cols.map(() => 0);
        const shown = [...t.key, ...cols.filter((c) => !t.key.includes(c))];
        const rows: (string | null)[][] = [];
        let total = 0;
        for (let i = 1; i <= Math.min(gen.total, SCAN); i++) {
          const row = gen.row(i);
          let hit = false;
          at.forEach((j, k) => {
            if (cellMatches(req.spec, row[j] ?? null)) {
              counts[k] = (counts[k] ?? 0) + 1;
              hit = true;
            }
          });
          if (!hit) continue;
          total++;
          if (rows.length < req.row_limit) rows.push(shown.map((c) => row[names.indexOf(c)] ?? null));
        }
        if (!total) continue;
        const columns = cols.map((column, k) => ({ column, count: counts[k] ?? 0 })).filter((c) => c.count > 0);
        const keep = [...t.key, ...columns.map((c) => c.column).filter((c) => !t.key.includes(c))];
        const hits: TableHits = {
          namespace: t.table.namespace,
          name: t.table.name,
          total,
          columns,
          shown: keep,
          key: t.key,
          rows: rows.map((r) => keep.map((c) => r[shown.indexOf(c)] ?? null)),
          elapsed_ms: pace(),
        };
        done.with_hits++;
        done.rows = (done.rows ?? 0) + total;
        onEvent({ kind: 'hits', hits });
      }
      stops.delete(req.job_id);
      done.elapsed_ms = Date.now() - started;
      return done;
    },
    jobCancel: async (id: string) => {
      stops.get(id)?.();
      await chain(id);
    },
  };
}
