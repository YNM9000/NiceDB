import type { QuotaCap } from './bindings';
import { m } from '../paraglide/messages';
import { getLocale } from '../paraglide/runtime';
import { bytes, duration } from './tools';

const count = (n: number) => new Intl.NumberFormat(getLocale() === 'it' ? 'it-IT' : 'en-GB', { maximumFractionDigits: 0 }).format(n);

export function quotaWindow(seconds: number): string {
  if (seconds % 86400 === 0) return m.quota_days({ n: seconds / 86400 });
  if (seconds % 3600 === 0) return m.quota_hours({ n: seconds / 3600 });
  if (seconds % 60 === 0) return m.quota_minutes({ n: seconds / 60 });
  return m.quota_seconds({ n: seconds });
}

const LABEL: Record<string, () => string> = {
  max_queries: m.quota_queries,
  max_query_selects: m.quota_selects,
  max_query_inserts: m.quota_inserts,
  max_errors: m.quota_errors,
  max_result_rows: m.quota_result_rows,
  max_result_bytes: m.quota_result_bytes,
  max_read_rows: m.quota_read_rows,
  max_read_bytes: m.quota_read_bytes,
  max_execution_time: m.quota_execution_time,
};

export function quotaCap(c: QuotaCap): string {
  const label = LABEL[c.what]?.() ?? c.what;
  if (c.max === null) return label;
  const value = c.what.endsWith('_bytes') ? bytes(c.max) : c.what === 'max_execution_time' ? duration(c.max * 1000) : count(c.max);
  return `${label} ${value}`;
}
