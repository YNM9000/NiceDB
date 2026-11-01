import { m } from '../paraglide/messages';
import type { RiskFound } from './api';
import { num } from './format';

export const touches = (n: number | null | undefined) => (n === null || n === undefined ? m.confirm_rows_unknown() : n === 1 ? m.confirm_rows_one() : m.confirm_rows_other({ count: num(n) }));

export const counted = (r: RiskFound) => r.risk === 'update_without_where' || r.risk === 'delete_without_where';

export function changeRows(counts: (number | null)[], kinds: string[]): { rows: string; stale: string | null } {
  let total = 0;
  let unknown = false;
  let stale = 0;
  kinds.forEach((kind, i) => {
    if (kind === 'insert') return void total++;
    const n = counts[i] ?? null;
    if (n === null) unknown = true;
    else {
      total += n;
      if (n === 0) stale++;
    }
  });
  return {
    rows: unknown ? m.confirm_rows_unknown() : touches(total),
    stale: stale === 0 ? null : stale === 1 ? m.confirm_stale_one() : m.confirm_stale_other({ count: num(stale) }),
  };
}
