import type { AuditEntry } from './api';
import { m } from '../paraglide/messages';

const iso = (ms: number) => new Date(ms).toISOString();

const cell = (v: string) => {
  const safe = /^[=+@\t\r]/.test(v) ? `'${v}` : v;
  return /[",\n\r]/.test(safe) ? `"${safe.replaceAll('"', '""')}"` : safe;
};

export function auditCsv(entries: readonly AuditEntry[]): string {
  const head = ['at', 'who', 'database', 'rows', 'error', 'sql'];
  const lines = entries.map((e) => [iso(e.at ?? 0), e.who, e.database, e.rows === null ? '' : String(e.rows), e.error ?? '', e.sql].map(cell).join(','));
  return `${[head.join(','), ...lines].join('\r\n')}\r\n`;
}

export function auditJson(entries: readonly AuditEntry[]): string {
  return `${JSON.stringify(
    entries.map((e) => ({ at: iso(e.at ?? 0), who: e.who, database: e.database, rows: e.rows, error: e.error, sql: e.sql })),
    null,
    2,
  )}\n`;
}

export function auditFile(connection: string, kind: 'csv' | 'json', now = new Date()): string {
  const name = connection.normalize('NFKD').replace(/[^\w-]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || m.file_connection();
  return `${m.file_audit({ connection: name, date: now.toISOString().slice(0, 10) })}.${kind}`;
}
