import { CHOICES, DRIVERS, isFlavor, splitChoice, type ChoiceId, type Driver } from './drivers';

export type EngineGroup = 'relational' | 'document' | 'keyvalue' | 'widecolumn' | 'search' | 'analytical' | 'enterprise' | 'odbc';

export const ENGINE_GROUPS: readonly EngineGroup[] = ['relational', 'document', 'keyvalue', 'widecolumn', 'search', 'analytical', 'enterprise', 'odbc'];

const ENTERPRISE: readonly string[] = ['sqlserver', 'oracle', 'firebird', 'db2', 'hana'];

const ALIASES: Partial<Record<ChoiceId, string>> = {
  postgres: 'postgres pg psql',
  mysql: 'mariadb',
  sqlserver: 'mssql tsql',
  oracle: 'plsql',
  cockroach: 'crdb',
  libsql: 'turso',
  mongodb: 'mongo',
  redis: 'valkey',
  cassandra: 'scylla scylladb cql',
  dynamodb: 'aws partiql',
  elasticsearch: 'opensearch elastic',
  db2: 'ibm luw',
  hana: 'sap',
  odbc: 'dsn informix teradata sybase progress',
};

export interface EngineRow {
  id: ChoiceId;
  driver: Driver;
  group: EngineGroup;
  child: boolean;
}

export const groupOf = (d: Driver): EngineGroup => (d.id === 'odbc' ? 'odbc' : ENTERPRISE.includes(d.id) ? 'enterprise' : d.base ? 'relational' : d.kind);

export function engineRows(choices: readonly ChoiceId[] = CHOICES): EngineRow[] {
  const rows = choices.flatMap((id) => {
    const driver = DRIVERS.find((d) => d.id === id);
    return driver ? [{ id, driver, group: groupOf(driver), child: isFlavor(id) }] : [];
  });
  return ENGINE_GROUPS.flatMap((g) => rows.filter((r) => r.group === g));
}

const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

export function engineMatches(row: EngineRow, query: string, words: (row: EngineRow) => string): boolean {
  const want = fold(query).split(/\s+/).filter(Boolean);
  if (!want.length) return true;
  const base = row.driver.base ? (DRIVERS.find((d) => d.id === row.driver.base)?.name ?? '') : '';
  const hay = fold([row.driver.name, row.id, ALIASES[row.id] ?? '', base, words(row)].join(' '));
  return want.every((w) => hay.includes(w));
}

export interface EngineSection {
  group: EngineGroup;
  rows: (EngineRow & { indent: boolean })[];
}

export function engineSections(rows: readonly EngineRow[], query: string, words: (row: EngineRow) => string): EngineSection[] {
  const hits = rows.filter((r) => engineMatches(r, query, words));
  const shown = new Set(hits.map((r) => r.id));
  return ENGINE_GROUPS.flatMap((group) => {
    const own = hits.filter((r) => r.group === group).map((r) => ({ ...r, indent: r.child && shown.has(splitChoice(r.id).driver) }));
    return own.length ? [{ group, rows: own }] : [];
  });
}

export function stepEngine(ids: readonly ChoiceId[], from: ChoiceId, dir: 1 | -1, usable: (id: ChoiceId) => boolean, far = 1): ChoiceId | null {
  const start = ids.indexOf(from);
  let at: ChoiceId | null = start < 0 ? null : from;
  let moved = 0;
  for (let i = start < 0 ? (dir > 0 ? 0 : ids.length - 1) : start + dir; i >= 0 && i < ids.length; i += dir) {
    const id = ids[i];
    if (id === undefined || !usable(id)) continue;
    at = id;
    if (++moved >= far) break;
  }
  return at;
}
