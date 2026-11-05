import type { Schema } from './api';
import type { Dependency_Serialize as Dependency, Dialect, GenRequest, ObjectRef_Deserialize as ObjectRef } from './bindings';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const WIDTH = 64;
const RESERVED = new Set('add all alter and any as asc between by case cast check column constraint create cross current default delete desc distinct drop else end except exists false fetch for foreign from full grant group having in index inner insert into is join key left like limit not null offset on or order outer primary references right row rows select set table then to top true union unique update user using values view when where with'.split(' '));

function name(d: Dialect, n: string): string {
  const plain = d === 'postgres' || d === 'duckdb' ? /^[a-z_][a-z0-9_]*$/ : /^[A-Za-z_][A-Za-z0-9_]*$/;
  if (plain.test(n) && !RESERVED.has(n.toLowerCase())) return n;
  const [open, close] = d === 'mysql' ? ['`', '`'] : d === 'mssql' ? ['[', ']'] : ['"', '"'];
  return open + n.replaceAll(close, close + close) + close;
}

const group = (items: string[], open: string, close: string) => {
  const flat = items.join(', ');
  return flat.length + open.length <= WIDTH ? `${open}(${flat})${close}` : `${open}(\n  ${items.join(',\n  ')}\n)${close}`;
};
const assign = (items: string[]) => (items.join(', ').length <= WIDTH && items.length <= 2 ? ` ${items.join(', ')}` : `\n  ${items.join(',\n  ')}`);

function params(req: GenRequest): string[] {
  const out: string[] = [];
  for (const c of req.columns) {
    let p = c.name.replace(/[^A-Za-z0-9]/g, '_').toLowerCase();
    if (!p || /^\d/.test(p)) p = `c${p}`;
    const base = p;
    for (let n = 2; out.includes(p); n++) p = `${base}_${n}`;
    out.push(p);
  }
  return out;
}

export function mockGenerate(d: Dialect, req: GenRequest): string | null {
  const keyed = req.columns.some((c) => c.primary_key);
  const g = req.generator;
  if (req.view && !['select', 'copy', 'drop'].includes(g)) return null;
  if (['update', 'delete', 'upsert'].includes(g) && !keyed) return null;
  if (!['select', 'copy', 'drop'].includes(g) && !req.columns.length) return null;
  const q = (n: string) => name(d, n);
  const t = req.table.namespace ? `${q(req.table.namespace)}.${q(req.table.name)}` : q(req.table.name);
  const ps = params(req);
  const cols = req.columns.map((c, i) => ({ n: q(c.name), p: ps[i] ?? '', key: c.primary_key, numbered: c.identity ?? null, type: c.type_name ?? '', notNull: !!c.not_null }));
  const given = cols.filter((c) => !c.numbered);
  const always = cols.some((c) => c.numbered === 'always');
  const names = cols.map((c) => c.n);
  const marks = cols.map((c) => `:${c.p}`);
  const keys = cols.filter((c) => c.key);
  const rest = cols.filter((c) => !c.key);
  const where = (l: string, r: (c: (typeof cols)[number]) => string) => keys.map((c) => `${l}${c.n} = ${r(c)}`).join('\n  AND ');
  const insert = `INSERT INTO ${group(names, `${t} `, '')}\n${group(marks, 'VALUES ', '')}`;
  switch (g) {
    case 'select':
      return `SELECT${names.length ? assign(names) : ' *'}\nFROM ${t};`;
    case 'insert':
      if (!given.length) return d === 'mysql' ? `INSERT INTO ${t} () VALUES ();` : `INSERT INTO ${t} DEFAULT VALUES;`;
      return `INSERT INTO ${group(given.map((c) => c.n), `${t} `, '')}\n${group(given.map((c) => `:${c.p}`), 'VALUES ', '')};`;
    case 'update': {
      const set = rest.length ? rest.map((c) => `${c.n} = :${c.p}`) : keys.map((c) => `${c.n} = :${c.p}_new`);
      return `UPDATE ${t}\nSET${assign(set)}\nWHERE ${where('', (c) => `:${c.p}`)};`;
    }
    case 'delete':
      return `DELETE FROM ${t}\nWHERE ${where('', (c) => `:${c.p}`)};`;
    case 'upsert':
      if (d === 'mysql') return `${insert}\nON DUPLICATE KEY UPDATE${assign(rest.length ? rest.map((c) => `${c.n} = VALUES(${c.n})`) : keys.map((c) => `${c.n} = ${c.n}`))};`;
      if (d === 'mssql') {
        const matched = rest.length ? `\nWHEN MATCHED THEN UPDATE SET${assign(rest.map((c) => `${c.n} = s.${c.n}`))}` : '';
        const merge = `MERGE INTO ${t} AS t\nUSING ${group(marks, '(VALUES ', ')')} AS s ${group(names, '', '')}\nON ${where('t.', (c) => `s.${c.n}`)}${matched}\nWHEN NOT MATCHED THEN INSERT ${group(names, '', '')}\n  ${group(cols.map((c) => `s.${c.n}`), 'VALUES ', '')};`;
        return always ? `SET IDENTITY_INSERT ${t} ON;\n${merge}\nSET IDENTITY_INSERT ${t} OFF;` : merge;
      }
      if (d === 'postgres' && always) {
        const over = `INSERT INTO ${group(names, `${t} `, '')}\nOVERRIDING SYSTEM VALUE\n${group(marks, 'VALUES ', '')}`;
        return rest.length ? `${over}\nON CONFLICT (${keys.map((c) => c.n).join(', ')}) DO UPDATE SET${assign(rest.map((c) => `${c.n} = EXCLUDED.${c.n}`))};` : `${over}\nON CONFLICT (${keys.map((c) => c.n).join(', ')}) DO NOTHING;`;
      }
      return rest.length ? `${insert}\nON CONFLICT (${keys.map((c) => c.n).join(', ')}) DO UPDATE SET${assign(rest.map((c) => `${c.n} = EXCLUDED.${c.n}`))};` : `${insert}\nON CONFLICT (${keys.map((c) => c.n).join(', ')}) DO NOTHING;`;
    case 'copy': {
      const to = req.table.namespace ? `${q(req.table.namespace)}.${q(req.copy_name)}` : q(req.copy_name);
      if (d === 'mssql' && !req.view && keys.length) return `SELECT *\nINTO ${to}\nFROM ${t};\nALTER TABLE ${to} ADD CONSTRAINT ${q(`pk_${req.copy_name}`)} PRIMARY KEY ${group(keys.map((c) => c.n), '', '')};`;
      if (d === 'mssql') return `SELECT *\nINTO ${to}\nFROM ${t};`;
      if ((d === 'sqlite' || d === 'duckdb') && !req.view && cols.length && cols.every((c) => c.type)) {
        const lines = cols.map((c) => `${c.n} ${c.type}${c.notNull || c.key ? ' NOT NULL' : ''}`);
        if (keys.length) lines.push(`PRIMARY KEY (${keys.map((c) => c.n).join(', ')})`);
        return `CREATE TABLE ${to} (\n  ${lines.join(',\n  ')}\n);\nINSERT INTO ${to}\nSELECT * FROM ${t};`;
      }
      if (d === 'postgres' && !req.view) return `CREATE TABLE ${to} (LIKE ${t} INCLUDING ALL);\nINSERT INTO ${to}\nSELECT * FROM ${t};`;
      if (d === 'mysql' && !req.view) return `CREATE TABLE ${to} LIKE ${t};\nINSERT INTO ${to}\nSELECT * FROM ${t};`;
      return `CREATE TABLE ${to} AS\nSELECT * FROM ${t};`;
    }
    case 'drop':
      return `DROP ${req.view ? 'VIEW' : 'TABLE'} ${t};`;
  }
}

const VIEW_READS: Record<string, string[]> = { v_scadenzario: ['fatture', 'clienti'] };

export function mockDependencies(schema: Schema, o: ObjectRef): Dependency[] {
  const out: Dependency[] = [];
  const dep = (direction: Dependency['direction'], via: Dependency['via'], kind: Dependency['kind'], namespace: string, name: string, detail: string | null = null) => out.push({ direction, via, kind, namespace, name, detail });
  for (const ns of schema.namespaces) {
    for (const t of ns.tables) {
      for (const fk of t.foreign_keys) {
        if (ns.name === o.namespace && t.name === o.name) dep('uses', 'foreign_key', 'table', fk.ref_namespace, fk.ref_table, fk.name);
        if (fk.ref_namespace === o.namespace && fk.ref_table === o.name) dep('used_by', 'foreign_key', 'table', ns.name, t.name, fk.name);
      }
    }
  }
  for (const [view, reads] of Object.entries(VIEW_READS)) {
    if (o.name === view) for (const r of reads) dep('uses', 'definition', 'table', 'public', r);
    if (reads.includes(o.name)) dep('used_by', 'definition', 'view', 'public', view);
  }
  if (o.name === 'fatture') {
    dep('used_by', 'definition', 'routine', 'public', 'totale_cliente');
    dep('uses', 'owned', 'sequence', 'public', 'fatture_id_seq');
  }
  if (o.name === 'righe_ordine') dep('used_by', 'trigger', 'trigger', 'public', 'aggiorna_totale', 'ricalcola_totale');
  return out;
}

export function dependenciesMock(schemaOf: (sid: string) => Promise<Schema>) {
  return {
    objectDependencies: async (sid: string, object: ObjectRef) => {
      await sleep(120);
      return mockDependencies(await schemaOf(sid), object);
    },
    sqlGenerate: async (dialect: Dialect, request: GenRequest) => mockGenerate(dialect, request),
    tableIdentity: async (_sid: string, _namespace: string, name: string) => (name === 'promozioni' ? [{ name: 'id', identity: 'always' as const }] : []),
  };
}
