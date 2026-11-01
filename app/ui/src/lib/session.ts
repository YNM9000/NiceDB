import { m } from '../paraglide/messages';
import type { CoreError, ConnectionProfile } from './api';
import type { DriverId } from './drivers';
import { num } from './format';

export type SessionOptions = ConnectionProfile['session'];

export const DEFAULT_SESSION: SessionOptions = { init: '', keepalive_secs: 0, reconnect: true, idle_minutes: 0, row_limit: null };

export const KEEPALIVE_CHOICES = [0, 30, 60, 120, 300, 600] as const;
export const IDLE_CHOICES = [0, 15, 30, 60, 120, 480] as const;

export const INIT_EXAMPLE: Record<DriverId, string> = {
  postgres: "SET search_path TO sales, public;\nSET TIME ZONE 'UTC';",
  cockroach: "SET search_path TO sales, public;\nSET TIME ZONE 'UTC';",
  mysql: "SET NAMES utf8mb4;\nSET time_zone = '+00:00';",
  sqlserver: 'SET LANGUAGE us_english;\nSET DATEFIRST 1;',
  sqlite: 'PRAGMA foreign_keys = ON;\nPRAGMA busy_timeout = 5000;',
  libsql: 'PRAGMA foreign_keys = ON;',
  redis: 'CLIENT SETNAME nicedb',
  duckdb: "SET threads = 4;\nSET TimeZone = 'UTC';",
  mongodb: '{ "ping": 1 }',
  clickhouse: "SET max_threads = 4;\nSET session_timezone = 'UTC';",
  cassandra: 'CONSISTENCY LOCAL_QUORUM;',
  dynamodb: '',
  elasticsearch: 'GET /_cluster/health',
  firebird: "SET TIME ZONE 'UTC';",
  db2: 'SET SCHEMA APP;',
  hana: 'SET SCHEMA APP;',
  odbc: '',
  oracle: "ALTER SESSION SET CURRENT_SCHEMA = APP;\nALTER SESSION SET TIME_ZONE = 'UTC';",
};

export function wholeNumber(text: string, min: number, max: number): number | null | undefined {
  const t = text.trim().replace(/[.\s]/g, '');
  if (!t) return null;
  if (!/^\d+$/.test(t)) return undefined;
  const n = Number(t);
  return n >= min && n <= max ? n : undefined;
}

export function statementCount(init: string): number {
  return init
    .replace(/--[^\n]*/g, '')
    .split(';')
    .filter((s) => s.trim()).length;
}

export function sessionSummary(s: SessionOptions, network: boolean): string {
  const parts: string[] = [];
  const n = statementCount(s.init);
  if (n) parts.push(m.session_sum_init({ count: n }));
  if (network && s.keepalive_secs) parts.push(m.session_sum_keepalive({ seconds: s.keepalive_secs }));
  if (network && !s.reconnect) parts.push(m.session_sum_no_reconnect());
  if (s.idle_minutes) parts.push(m.session_sum_idle({ minutes: s.idle_minutes }));
  if (s.row_limit) parts.push(m.session_sum_limit({ count: num(s.row_limit) }));
  return parts.length ? parts.join(' · ') : m.session_sum_default();
}

export function initFailure(e: Extract<CoreError, { code: 'init' }>['detail']): string {
  const message = 'message' in e.error ? e.error.message : e.error.kind;
  return m.err_init({ index: e.index + 1, sql: e.sql, message });
}

export type Health = 'ok' | 'lost' | 'closed' | 'reconnecting';

export interface Dropped {
  index: number;
  write: boolean;
  transactionLost: boolean;
  restored: boolean;
  error: string | null;
}

export function droppedNotice(d: Dropped): string {
  const head = d.restored ? m.session_dropped_restored({ index: d.index + 1 }) : m.session_dropped_lost({ index: d.index + 1 });
  const tail = d.write ? m.session_dropped_write() : m.session_dropped_read();
  const tx = d.transactionLost ? ` ${m.session_tx_lost()}` : '';
  return `${head} ${tail}${tx}`;
}
