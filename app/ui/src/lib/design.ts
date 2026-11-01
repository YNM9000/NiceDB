import { m } from '../paraglide/messages';
import type { DesignColumn_Deserialize as DesignColumn, DesignEngine, DesignIndex_Deserialize as DesignIndex, DesignNote_Serialize as DesignNote, FkAction, TableDesign_Deserialize as TableDesign } from './bindings';
import type { Dialect } from './bindings';
import { flavorOf } from './tools';

export type { DesignColumn, DesignEngine, DesignIndex, DesignNote, TableDesign };

export type ArgKind = 'none' | 'length' | 'precision';

const CATALOG: Record<DesignEngine, [string, ArgKind][]> = {
  postgres: [['integer', 'none'], ['bigint', 'none'], ['smallint', 'none'], ['numeric', 'precision'], ['real', 'none'], ['double precision', 'none'], ['boolean', 'none'], ['text', 'none'], ['varchar', 'length'], ['char', 'length'], ['date', 'none'], ['time', 'none'], ['timestamp', 'none'], ['timestamptz', 'none'], ['interval', 'none'], ['uuid', 'none'], ['json', 'none'], ['jsonb', 'none'], ['bytea', 'none'], ['inet', 'none']],
  cockroach: [['INT8', 'none'], ['INT4', 'none'], ['INT2', 'none'], ['DECIMAL', 'precision'], ['FLOAT8', 'none'], ['BOOL', 'none'], ['STRING', 'none'], ['VARCHAR', 'length'], ['CHAR', 'length'], ['DATE', 'none'], ['TIME', 'none'], ['TIMESTAMP', 'none'], ['TIMESTAMPTZ', 'none'], ['INTERVAL', 'none'], ['UUID', 'none'], ['JSONB', 'none'], ['BYTES', 'none'], ['INET', 'none']],
  mysql: [['int', 'none'], ['bigint', 'none'], ['smallint', 'none'], ['tinyint', 'none'], ['decimal', 'precision'], ['double', 'none'], ['float', 'none'], ['varchar', 'length'], ['char', 'length'], ['text', 'none'], ['mediumtext', 'none'], ['longtext', 'none'], ['date', 'none'], ['datetime', 'none'], ['timestamp', 'none'], ['time', 'none'], ['json', 'none'], ['blob', 'none'], ['varbinary', 'length']],
  mssql: [['int', 'none'], ['bigint', 'none'], ['smallint', 'none'], ['tinyint', 'none'], ['bit', 'none'], ['decimal', 'precision'], ['numeric', 'precision'], ['money', 'none'], ['float', 'none'], ['real', 'none'], ['nvarchar', 'length'], ['varchar', 'length'], ['nchar', 'length'], ['char', 'length'], ['date', 'none'], ['datetime2', 'length'], ['datetime', 'none'], ['time', 'length'], ['datetimeoffset', 'length'], ['uniqueidentifier', 'none'], ['varbinary', 'length'], ['xml', 'none']],
  sqlite: [['INTEGER', 'none'], ['REAL', 'none'], ['TEXT', 'none'], ['BLOB', 'none'], ['NUMERIC', 'none'], ['VARCHAR', 'length'], ['DATE', 'none'], ['DATETIME', 'none'], ['BOOLEAN', 'none']],
  firebird: [['INTEGER', 'none'], ['BIGINT', 'none'], ['SMALLINT', 'none'], ['NUMERIC', 'precision'], ['DECIMAL', 'precision'], ['DOUBLE PRECISION', 'none'], ['BOOLEAN', 'none'], ['VARCHAR', 'length'], ['CHAR', 'length'], ['BLOB SUB_TYPE TEXT', 'none'], ['BLOB', 'none'], ['DATE', 'none'], ['TIME', 'none'], ['TIMESTAMP', 'none'], ['TIMESTAMP WITH TIME ZONE', 'none'], ['INT128', 'none'], ['DECFLOAT', 'none']],
  duckdb: [['INTEGER', 'none'], ['BIGINT', 'none'], ['SMALLINT', 'none'], ['DOUBLE', 'none'], ['DECIMAL', 'precision'], ['BOOLEAN', 'none'], ['VARCHAR', 'none'], ['DATE', 'none'], ['TIME', 'none'], ['TIMESTAMP', 'none'], ['TIMESTAMPTZ', 'none'], ['INTERVAL', 'none'], ['UUID', 'none'], ['JSON', 'none'], ['BLOB', 'none']],
};

const INT: Record<DesignEngine, string> = { postgres: 'integer', cockroach: 'INT8', mysql: 'int', mssql: 'int', sqlite: 'INTEGER', duckdb: 'INTEGER', firebird: 'INTEGER' };
const TEXT: Record<DesignEngine, string> = { postgres: 'text', cockroach: 'STRING', mysql: 'varchar(100)', mssql: 'nvarchar(100)', sqlite: 'TEXT', duckdb: 'VARCHAR', firebird: 'VARCHAR(100)' };

export const ENGINE_NAME: Record<DesignEngine, string> = { postgres: 'PostgreSQL', cockroach: 'CockroachDB', mysql: 'MySQL / MariaDB', mssql: 'SQL Server', sqlite: 'SQLite', duckdb: 'DuckDB', firebird: 'Firebird' };

export function engineOf(dialect: Dialect, product: string): DesignEngine | null {
  const f = flavorOf(dialect, product);
  if (f === 'hana' || f === 'oracle' || f === 'db2' || f === 'ansi') return null;
  return f === 'clickhouse' ? 'duckdb' : f;
}

export const typesOf = (e: DesignEngine) => CATALOG[e].map(([t]) => t);

export function typeParts(t: string): { base: string; args: string } {
  const at = t.indexOf('(');
  if (at < 0 || !t.trimEnd().endsWith(')')) return { base: t.trim(), args: '' };
  return { base: t.slice(0, at).trim(), args: t.slice(at + 1, t.trimEnd().length - 1).trim() };
}

export const typeJoin = (base: string, args: string) => (args.trim() ? `${base.trim()}(${args.trim()})` : base.trim());

const ALIASES: Record<string, ArgKind> = { 'character varying': 'length', character: 'length', bpchar: 'length', 'bit varying': 'length', 'timestamp without time zone': 'none', 'timestamp with time zone': 'none', 'time without time zone': 'none', int4: 'none', int8: 'none', int2: 'none', float4: 'none', float8: 'none', bool: 'none', decimal: 'precision', numeric: 'precision', number: 'precision' };

export function argKind(e: DesignEngine, base: string): ArgKind {
  const b = base.trim().toLowerCase();
  return CATALOG[e].find(([t]) => t.toLowerCase() === b)?.[1] ?? ALIASES[b] ?? 'length';
}

export const hasDryRun = (e: DesignEngine) => e === 'postgres' || e === 'sqlite' || e === 'mssql' || e === 'duckdb';
export const canReorder = (e: DesignEngine) => e === 'mysql' || e === 'sqlite' || e === 'duckdb';
export const hasComments = (e: DesignEngine) => e !== 'sqlite';
export const hasIdentity = (e: DesignEngine) => e !== 'duckdb';

export const FK_ACTIONS: FkAction[] = ['no_action', 'restrict', 'cascade', 'set_null', 'set_default'];
export const fkActionsOf = (e: DesignEngine): FkAction[] => (e === 'duckdb' ? ['no_action', 'restrict'] : FK_ACTIONS);
export const FK_SQL: Record<FkAction, string> = { no_action: 'NO ACTION', restrict: 'RESTRICT', cascade: 'CASCADE', set_null: 'SET NULL', set_default: 'SET DEFAULT' };

export function newColumn(e: DesignEngine, taken: string[]): DesignColumn {
  let n = taken.length + 1;
  while (taken.includes(m.dz_default_column({ n }))) n++;
  return { origin: null, name: m.dz_default_column({ n }), type_name: TEXT[e], nullable: true, default: null, identity: false, comment: null, default_name: null };
}

export function blankDesign(e: DesignEngine, namespace: string): TableDesign {
  return {
    namespace,
    name: '',
    comment: null,
    columns: [{ origin: null, name: 'id', type_name: INT[e], nullable: false, default: null, identity: hasIdentity(e), comment: null, default_name: null }],
    primary_key: ['id'],
    pk_name: null,
    indexes: [],
    foreign_keys: [],
    checks: [],
    triggers: [],
  };
}

const swap = (list: string[], from: string, to: string) => list.map((c) => (c === from ? to : c));

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

export function renameIn(expr: string, from: string, to: string): string {
  const name = escapeRe(from);
  return expr.replace(new RegExp(`"${name}"|\`${name}\`|\\[${name}\\]|(?<![\\w$"\`\\[])${name}(?![\\w$])`, 'g'), (hit) => {
    const q = hit[0];
    if (q === '"' || q === '`') return `${q}${to}${q}`;
    if (q === '[') return `[${to}]`;
    return to;
  });
}

const isExpr = (i: DesignIndex, k: number) => !!i.expressions?.[k];

function renameRefs(d: TableDesign, from: string, to: string) {
  d.primary_key = swap(d.primary_key ?? [], from, to);
  for (const i of d.indexes ?? []) {
    i.columns = i.columns.map((c, k) => (isExpr(i, k) ? renameIn(c, from, to) : c === from ? to : c));
    if (i.predicate) i.predicate = renameIn(i.predicate, from, to);
  }
  for (const f of d.foreign_keys ?? []) {
    f.columns = swap(f.columns, from, to);
    if (f.ref_table === d.name && !f.ref_namespace) f.ref_columns = swap(f.ref_columns, from, to);
  }
}

const parked = new WeakMap<object, string>();
let parkSeq = 0;

export const shownRef = (ref: string) => ref.replace(/^\u0001\d+:/, '');

export function renameColumn(d: TableDesign, at: number, to: string) {
  const c = d.columns[at];
  if (!c || c.name === to) return;
  const from = parked.get(c) ?? c.name;
  const other = d.columns.find((x, i) => i !== at && x.name === to && !parked.has(x));
  if (other) {
    const hold = `\u0001${++parkSeq}:${to}`;
    parked.set(other, hold);
    renameRefs(d, to, hold);
  }
  c.name = to;
  parked.delete(c);
  renameRefs(d, from, to);
}

export function removeColumn(d: TableDesign, at: number) {
  const c = d.columns[at];
  if (!c) return;
  d.columns.splice(at, 1);
  d.primary_key = (d.primary_key ?? []).filter((x) => x !== c.name);
  d.indexes = (d.indexes ?? [])
    .map((i) => {
      const keep = i.columns.map((x, k) => isExpr(i, k) || x !== c.name);
      const pick = <T,>(list: T[] | undefined) => (list ?? []).filter((_, k) => keep[k]);
      return { ...i, columns: pick(i.columns), descending: pick(i.descending), expressions: pick(i.expressions) };
    })
    .filter((i) => i.columns.length > 0);
  d.foreign_keys = (d.foreign_keys ?? []).filter((f) => !f.columns.includes(c.name));
}

export function movePart(i: DesignIndex, k: number, by: number) {
  const to = k + by;
  if (to < 0 || to >= i.columns.length) return;
  for (const key of ['columns', 'descending', 'expressions'] as const) {
    const list = [...(i[key] ?? [])] as (string | boolean)[];
    while (list.length < i.columns.length) list.push(false);
    const [x] = list.splice(k, 1);
    list.splice(to, 0, x as string | boolean);
    (i as Record<typeof key, unknown>)[key] = list;
  }
}

export function removePart(i: DesignIndex, k: number) {
  i.columns = i.columns.filter((_, j) => j !== k);
  i.descending = (i.descending ?? []).filter((_, j) => j !== k);
  i.expressions = (i.expressions ?? []).filter((_, j) => j !== k);
}

export function addPart(i: DesignIndex, part: string, expression = false) {
  const n = i.columns.length;
  i.columns = [...i.columns, part];
  i.descending = [...Array.from({ length: n }, (_, j) => i.descending?.[j] ?? false), false];
  i.expressions = [...Array.from({ length: n }, (_, j) => i.expressions?.[j] ?? false), expression];
}

export function flipPart(i: DesignIndex, k: number) {
  i.descending = Array.from({ length: i.columns.length }, (_, j) => (j === k ? !i.descending?.[j] : (i.descending?.[j] ?? false)));
}

export const hasPartial = (e: DesignEngine) => e !== 'mysql' && e !== 'duckdb';

export function moveColumn(d: TableDesign, at: number, by: number) {
  const to = at + by;
  if (to < 0 || to >= d.columns.length) return;
  const [c] = d.columns.splice(at, 1);
  if (c) d.columns.splice(to, 0, c);
}

export function togglePrimary(d: TableDesign, name: string) {
  const pk = d.primary_key ?? [];
  d.primary_key = pk.includes(name) ? pk.filter((x) => x !== name) : [...pk, name];
  const c = d.columns.find((x) => x.name === name);
  if (c && d.primary_key.includes(name)) c.nullable = false;
}

const slug = (s: string) => s.replace(/[^A-Za-z0-9_]+/g, '_').replace(/^_+|_+$/g, '').toLowerCase();

export function uniqueName(base: string, taken: string[]) {
  let n = slug(base) || 'x';
  for (let i = 2; taken.includes(n); i++) n = `${slug(base)}_${i}`;
  return n;
}

export function takenNames(d: TableDesign) {
  return [...(d.indexes ?? []).map((i) => i.name), ...(d.foreign_keys ?? []).map((f) => f.name), ...(d.checks ?? []).map((c) => c.name)];
}

export type RowMark = 'new' | 'changed' | null;

export function columnMark(before: TableDesign | null, c: DesignColumn): RowMark {
  if (!before) return null;
  const old = c.origin ? before.columns.find((x) => x.name === c.origin) : undefined;
  if (!old) return 'new';
  const same = old.name === c.name && old.type_name === c.type_name && old.nullable === c.nullable && (old.default ?? '') === (c.default ?? '') && old.identity === c.identity && (old.comment ?? '') === (c.comment ?? '');
  return same ? null : 'changed';
}

export const droppedColumns = (before: TableDesign | null, after: TableDesign) => (before ? before.columns.filter((b) => !after.columns.some((c) => c.origin === b.name)) : []);

export const scriptText = (statements: string[]) => statements.map((s) => `${s};`).join('\n\n');

const SUBJECT: Record<string, () => string> = {
  table: m.dz_bad_table,
  columns: m.dz_bad_columns,
  column_name: m.dz_bad_column_name,
  column_type: m.dz_bad_column_type,
  primary_key: m.dz_bad_primary_key,
  index: m.dz_bad_index,
  foreign_key: m.dz_bad_foreign_key,
  check: m.dz_bad_check,
};

export type NoteLevel = 'bad' | 'warn' | 'info';

export function noteLevel(n: DesignNote): NoteLevel {
  switch (n.code) {
    case 'invalid':
    case 'drops_data':
    case 'trigger_blocks':
      return 'bad';
    case 'rebuild':
    case 'reorder_unsupported':
    case 'identity_unsupported':
    case 'primary_key_required':
    case 'fk_action_unsupported':
    case 'not_null_needs_value':
    case 'partial_unsupported':
      return 'warn';
    default:
      return 'info';
  }
}

export function noteText(n: DesignNote, e: DesignEngine): string {
  const s = n.subject ?? '';
  const engine = ENGINE_NAME[e];
  switch (n.code) {
    case 'invalid':
      return SUBJECT[s]?.() ?? m.dz_bad_named({ name: s });
    case 'drops_data':
      return m.dz_note_drops({ name: s });
    case 'rebuild':
      return m.dz_note_rebuild({ engine });
    case 'reorder_rewrites':
      return m.dz_note_reorder_rewrites({ name: s });
    case 'reorder_unsupported':
      return m.dz_note_reorder_unsupported({ engine });
    case 'default_constraint':
      return m.dz_note_default_constraint({ name: s });
    case 'identity_unsupported':
      return m.dz_note_identity({ name: s, engine });
    case 'comment_unsupported':
      return m.dz_note_comment({ engine });
    case 'not_null_needs_value':
      return m.dz_note_not_null({ name: s });
    case 'type_change':
      return m.dz_note_type({ name: s });
    case 'primary_key_required':
      return m.dz_note_pk_required({ engine });
    case 'fk_action_unsupported':
      return m.dz_note_fk_action({ name: s, engine });
    case 'trigger_recreated':
      return m.dz_note_trigger({ name: s });
    case 'trigger_blocks':
      return m.dz_note_trigger_blocks({ name: s, what: n.detail ?? '' });
    case 'partial_unsupported':
      return m.dz_note_partial({ name: s, engine });
    case 'rename_via_temp':
      return m.dz_note_rename_temp({ name: s });
    case 'rename_table_unsupported':
      return m.dz_note_rename_table({ name: s, engine });
  }
}

export const applyHint = (e: DesignEngine) => (hasDryRun(e) ? m.dz_apply_dry_hint() : m.dz_apply_direct_hint({ engine: ENGINE_NAME[e] }));
