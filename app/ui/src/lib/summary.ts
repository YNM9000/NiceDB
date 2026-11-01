import { aggregate, type Stats } from './aggregate';
import { api } from './api';
import type { GroupValue_Deserialize as GroupValue, SummaryColumn, SummarySource_Deserialize as SummarySource } from './bindings';

export type { GroupValue, SummaryColumn, SummarySource };

export const GROUP_LIMIT = 5000;

export function statsFromRow(row: readonly (string | null)[], cols: readonly SummaryColumn[]): Stats {
  const rows = Number(row[0] ?? 0);
  let at = 1;
  let count = 0;
  const sums: string[] = [];
  const mins: string[] = [];
  const maxs: string[] = [];
  let distinct: number | null = null;
  for (const c of cols) {
    count += Number(row[at++] ?? 0);
    if (c.numeric) {
      const [sum, min, max] = [row[at++], row[at++], row[at++]];
      if (sum != null) sums.push(sum);
      if (min != null) mins.push(min);
      if (max != null) maxs.push(max);
    } else {
      const d = Number(row[at++] ?? 0);
      distinct = cols.length === 1 ? d : null;
    }
  }
  const cells = rows * cols.length;
  const base: Stats = { cells, nulls: cells - count, count, numeric: false, sum: null, avg: null, min: null, max: null, distinct, scale: 0 };
  if (!count || !cols.every((c) => c.numeric)) return base;
  const total = aggregate([{ numeric: true, values: sums }]);
  const lo = aggregate([{ numeric: true, values: mins }]);
  const hi = aggregate([{ numeric: true, values: maxs }]);
  const sum = total.sum ?? '0';
  return { ...base, numeric: true, sum, avg: Number(sum) / count, min: lo.min, max: hi.max, scale: total.scale };
}

export const regrouped = (agg: GroupValue['func']) => (agg === 'count' ? 'sum' : agg);

export interface Summarizer {
  stats: (cols: SummaryColumn[]) => Promise<Stats>;
  groups: (x: string, values: GroupValue[]) => Promise<(string | null)[][]>;
}

export const summarizer = (sessionId: () => string | null, source: () => SummarySource): Summarizer => {
  const live = () => {
    const id = sessionId();
    if (!id) throw new Error('no session');
    return id;
  };
  return {
    stats: async (columns) => statsFromRow((await api.dataSummarize(live(), source(), { kind: 'stats', columns }))[0] ?? [], columns),
    groups: (x, values) => api.dataSummarize(live(), source(), { kind: 'group', x, values, limit: GROUP_LIMIT + 1 }),
  };
};
