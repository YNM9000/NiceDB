import type { ConnectionProfile, PlanReport } from './api';
import engines from './mock-plan-engines.json';
import { DEFAULT_SESSION } from './session';

const network = (host: string, port: number) => ({ host, port, database: 'gestionale', user: 'nicedb', tls: 'disable' as const, ca_file: null, ssh: null, proxy: null, client_cert: null });
const base = { folder: null, environment: 'development' as const, color: null, session: DEFAULT_SESSION, template: { kind: 'none' as const }, raw_fields: {} };

export const ENGINE_CONNECTIONS: ConnectionProfile[] = [
  { ...base, id: 'officina-ms', name: 'Officina — SQL Server', target: { driver: 'sqlserver', ...network('10.0.4.12', 1433) } },
  { ...base, id: 'officina-duck', name: 'Officina — DuckDB', target: { driver: 'duckdb', path: '/home/ada/dati/officina.duckdb', read_only: false } },
  { ...base, id: 'officina-crdb', name: 'Officina — CockroachDB', target: { driver: 'cockroach', ...network('10.0.4.20', 26257) } },
];

const PLANS: Record<string, unknown> = engines;

export function enginePlan(driver: string | undefined): PlanReport | null {
  const r = driver ? PLANS[driver] : undefined;
  return r ? (structuredClone(r) as PlanReport) : null;
}
