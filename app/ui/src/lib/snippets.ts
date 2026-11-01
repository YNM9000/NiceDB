import { m } from '../paraglide/messages';
import type { Snippet } from './api';
import type { Dialect } from './bindings';

export interface ReadySnippet {
  id: string;
  name: string;
  trigger: string;
  description: string;
  body: string;
  builtin: boolean;
  dialect: string | null;
}

type Body = string | (Partial<Record<Dialect, string | null>> & { other: string | null }) | ((d: Dialect) => string);

interface Builtin {
  id: string;
  trigger: string;
  name: () => string;
  description: () => string;
  body: Body;
}

const bodyFor = (b: Body, d: Dialect): string | null => (typeof b === 'string' ? b : typeof b === 'function' ? b(d) : b[d] === undefined ? b.other : b[d]);

const BUILTINS: Builtin[] = [
  {
    id: 'topn',
    trigger: 'topn',
    name: m.snip_topn,
    description: m.snip_topn_desc,
    body: {
      mssql: 'SELECT TOP (${3:100}) *\nFROM ${1:table}\nORDER BY ${2:column} DESC;',
      oracle: 'SELECT *\nFROM ${1:table}\nORDER BY ${2:column} DESC\nFETCH FIRST ${3:100} ROWS ONLY;',
      cql: 'SELECT *\nFROM ${1:table}\nWHERE ${2:key} = ${3:value}\nORDER BY ${4:clustering} DESC\nLIMIT ${5:100};',
      partiql: null,
      other: 'SELECT *\nFROM ${1:table}\nORDER BY ${2:column} DESC\nLIMIT ${3:100};',
    },
  },
  {
    id: 'countby',
    trigger: 'countby',
    name: m.snip_countby,
    description: m.snip_countby_desc,
    body: { cql: null, partiql: null, other: 'SELECT ${1:column}, COUNT(*) AS n\nFROM ${2:table}\nGROUP BY ${1:column}\nORDER BY n DESC;' },
  },
  {
    id: 'dups',
    trigger: 'dups',
    name: m.snip_dups,
    description: m.snip_dups_desc,
    body: { cql: null, partiql: null, other: 'SELECT ${1:column}, COUNT(*) AS n\nFROM ${2:table}\nGROUP BY ${1:column}\nHAVING COUNT(*) > 1\nORDER BY n DESC;' },
  },
  {
    id: 'tsize',
    trigger: 'tsize',
    name: m.snip_tsize,
    description: m.snip_tsize_desc,
    body: {
      postgres: "SELECT pg_size_pretty(pg_total_relation_size('${1:schema.table}')) AS size;",
      mysql: "SELECT table_name, ROUND((data_length + index_length) / 1024 / 1024, 2) AS mb\nFROM information_schema.tables\nWHERE table_schema = DATABASE() AND table_name = '${1:table}';",
      sqlite: "SELECT SUM(pgsize) / 1024.0 AS kb\nFROM dbstat\nWHERE name = '${1:table}';",
      mssql: "EXEC sp_spaceused '${1:table}';",
      oracle: "SELECT segment_name, segment_type, ROUND(SUM(bytes) / 1024 / 1024, 2) AS mb\nFROM user_segments\nWHERE segment_name = UPPER('${1:table}')\nGROUP BY segment_name, segment_type;",
      clickhouse: "SELECT formatReadableSize(sum(bytes_on_disk)) AS size, sum(rows) AS rows\nFROM system.parts\nWHERE active AND database = currentDatabase() AND table = '${1:table}';",
      cql: "SELECT sum(partitions_count), max(mean_partition_size)\nFROM system.size_estimates\nWHERE keyspace_name = '${1:keyspace}' AND table_name = '${2:table}';",
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: "SELECT estimated_size, column_count\nFROM duckdb_tables()\nWHERE table_name = '${1:table}';",
    },
  },
  {
    id: 'nulls',
    trigger: 'nulls',
    name: m.snip_nulls,
    description: m.snip_nulls_desc,
    body: (d) => (d === 'cql' || d === 'partiql' ? '' : `SELECT COUNT(*) AS ${m.snip_alias_total()}, COUNT(\${1:column}) AS ${m.snip_alias_filled()},\n       COUNT(*) - COUNT(\${1:column}) AS ${m.snip_alias_empty()}\nFROM \${2:table};`),
  },
  {
    id: 'join',
    trigger: 'join',
    name: m.snip_join,
    description: m.snip_join_desc,
    body: { cql: null, partiql: null, opensearch: null, elasticsearch: null, other: 'SELECT a.*, b.${4:column}\nFROM ${1:table} a\nJOIN ${2:other} b ON b.${3:key} = a.${3:key}\nWHERE ${5:1=1};' },
  },
  {
    id: 'cte',
    trigger: 'cte',
    name: m.snip_cte,
    description: m.snip_cte_desc,
    body: { cql: null, partiql: null, opensearch: null, elasticsearch: null, other: 'WITH ${1:base} AS (\n  SELECT ${2:*}\n  FROM ${3:table}\n  WHERE ${4:1=1}\n)\nSELECT * FROM ${1:base};' },
  },
  {
    id: 'insert',
    trigger: 'ins',
    name: m.snip_insert,
    description: m.snip_insert_desc,
    body: {
      partiql: "INSERT INTO \"${1:table}\" VALUE {'${2:key}': ${3:value}};",
      opensearch: null,
      elasticsearch: null,
      other: 'INSERT INTO ${1:table} (${2:column})\nVALUES (${3:value});',
    },
  },
  {
    id: 'page',
    trigger: 'page',
    name: m.snip_page,
    description: m.snip_page_desc,
    body: {
      mssql: 'SELECT *\nFROM ${1:table}\nORDER BY ${2:column}\nOFFSET ${3:0} ROWS FETCH NEXT ${4:50} ROWS ONLY;',
      oracle: 'SELECT *\nFROM ${1:table}\nORDER BY ${2:column}\nOFFSET ${3:0} ROWS FETCH NEXT ${4:50} ROWS ONLY;',
      cql: null,
      partiql: null,
      other: 'SELECT *\nFROM ${1:table}\nORDER BY ${2:column}\nLIMIT ${4:50} OFFSET ${3:0};',
    },
  },
  {
    id: 'upsert',
    trigger: 'upsert',
    name: m.snip_upsert,
    description: m.snip_upsert_desc,
    body: {
      mysql: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:value}, ${5:value})\nON DUPLICATE KEY UPDATE ${3:column} = VALUES(${3:column});',
      mssql: 'MERGE ${1:table} AS t\nUSING (SELECT ${4:value} AS ${2:key}, ${5:value} AS ${3:column}) AS s\nON t.${2:key} = s.${2:key}\nWHEN MATCHED THEN UPDATE SET ${3:column} = s.${3:column}\nWHEN NOT MATCHED THEN INSERT (${2:key}, ${3:column}) VALUES (s.${2:key}, s.${3:column});',
      clickhouse: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:value}, ${5:value});',
      oracle: 'MERGE INTO ${1:table} t\nUSING (SELECT ${4:value} AS ${2:key}, ${5:value} AS ${3:column} FROM dual) s\nON (t.${2:key} = s.${2:key})\nWHEN MATCHED THEN UPDATE SET t.${3:column} = s.${3:column}\nWHEN NOT MATCHED THEN INSERT (${2:key}, ${3:column}) VALUES (s.${2:key}, s.${3:column});',
      cql: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:value}, ${5:value});',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: 'INSERT INTO ${1:table} (${2:key}, ${3:column})\nVALUES (${4:value}, ${5:value})\nON CONFLICT (${2:key}) DO UPDATE SET ${3:column} = excluded.${3:column};',
    },
  },
  {
    id: 'lastn',
    trigger: 'lastn',
    name: m.snip_lastn,
    description: m.snip_lastn_desc,
    body: {
      clickhouse: 'SELECT *\nFROM ${1:table}\nORDER BY ${3:column} DESC\nLIMIT ${4:3} BY ${2:group};',
      cql: 'SELECT *\nFROM ${1:table}\nPER PARTITION LIMIT ${2:3};',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: 'SELECT *\nFROM (\n  SELECT t.*, ROW_NUMBER() OVER (PARTITION BY ${2:group} ORDER BY ${3:column} DESC) AS rn\n  FROM ${1:table} t\n) x\nWHERE rn <= ${4:3};',
    },
  },
  {
    id: 'ctable',
    trigger: 'ctable',
    name: m.snip_ctable,
    description: m.snip_ctable_desc,
    body: {
      postgres: 'CREATE TABLE ${1:table} (\n  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n  ${2:name} text NOT NULL,\n  created_at timestamptz NOT NULL DEFAULT now()\n);',
      mysql: 'CREATE TABLE ${1:table} (\n  id bigint AUTO_INCREMENT PRIMARY KEY,\n  ${2:name} varchar(255) NOT NULL,\n  created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP\n);',
      sqlite: 'CREATE TABLE ${1:table} (\n  id INTEGER PRIMARY KEY,\n  ${2:name} TEXT NOT NULL,\n  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\n);',
      mssql: 'CREATE TABLE ${1:table} (\n  id bigint IDENTITY(1,1) PRIMARY KEY,\n  ${2:name} nvarchar(255) NOT NULL,\n  created_at datetime2 NOT NULL DEFAULT SYSUTCDATETIME()\n);',
      oracle: 'CREATE TABLE ${1:table} (\n  id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,\n  ${2:name} VARCHAR2(255 CHAR) NOT NULL,\n  created_at TIMESTAMP WITH TIME ZONE DEFAULT SYSTIMESTAMP NOT NULL\n);',
      cql: 'CREATE TABLE ${1:table} (\n  id uuid PRIMARY KEY,\n  ${2:name} text,\n  created_at timestamp\n);',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      clickhouse: 'CREATE TABLE ${1:table} (\n  id UInt64,\n  ${2:name} String,\n  created_at DateTime DEFAULT now()\n)\nENGINE = MergeTree\nORDER BY id;',
      other: 'CREATE TABLE ${1:table} (\n  id BIGINT PRIMARY KEY,\n  ${2:name} VARCHAR NOT NULL,\n  created_at TIMESTAMP NOT NULL DEFAULT current_timestamp\n);',
    },
  },
  {
    id: 'addcol',
    trigger: 'addcol',
    name: m.snip_addcol,
    description: m.snip_addcol_desc,
    body: {
      postgres: 'ALTER TABLE ${1:table} ADD COLUMN ${2:column} ${3:text};',
      mysql: 'ALTER TABLE ${1:table} ADD COLUMN ${2:column} ${3:varchar(255)};',
      sqlite: 'ALTER TABLE ${1:table} ADD COLUMN ${2:column} ${3:TEXT};',
      mssql: 'ALTER TABLE ${1:table} ADD ${2:column} ${3:nvarchar(255)};',
      oracle: 'ALTER TABLE ${1:table} ADD (${2:column} ${3:VARCHAR2(255 CHAR)});',
      clickhouse: 'ALTER TABLE ${1:table} ADD COLUMN ${2:column} ${3:String};',
      cql: 'ALTER TABLE ${1:table} ADD ${2:column} ${3:text};',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: 'ALTER TABLE ${1:table} ADD COLUMN ${2:column} ${3:VARCHAR};',
    },
  },
  {
    id: 'index',
    trigger: 'index',
    name: m.snip_index,
    description: m.snip_index_desc,
    body: {
      clickhouse: 'ALTER TABLE ${2:table} ADD INDEX ${1:name} ${3:column} TYPE minmax GRANULARITY 4;',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: 'CREATE INDEX ${1:name} ON ${2:table} (${3:column});',
    },
  },
  {
    id: 'tx',
    trigger: 'tx',
    name: m.snip_tx,
    description: m.snip_tx_desc,
    body: {
      mysql: 'START TRANSACTION;\n\n${1}\n\nCOMMIT;',
      mssql: 'BEGIN TRANSACTION;\n\n${1}\n\nCOMMIT TRANSACTION;',
      oracle: 'SET TRANSACTION READ WRITE;\n\n${1}\n\nCOMMIT;',
      clickhouse: null,
      cql: 'BEGIN BATCH\n  ${1}\nAPPLY BATCH;',
      partiql: null,
      opensearch: null,
      elasticsearch: null,
      other: 'BEGIN;\n\n${1}\n\nCOMMIT;',
    },
  },
  {
    id: 'block',
    trigger: 'block',
    name: m.snip_block,
    description: m.snip_block_desc,
    body: {
      oracle: 'DECLARE\n  ${1:n} NUMBER;\nBEGIN\n  ${2:NULL};\nEND;\n/',
      other: null,
    },
  },
];

export function builtinSnippets(dialect: Dialect): ReadySnippet[] {
  return BUILTINS.flatMap((b) => {
    const body = bodyFor(b.body, dialect);
    return !body ? [] : [{ id: `builtin:${b.id}`, name: b.name(), trigger: b.trigger, description: b.description(), body, builtin: true, dialect: null }];
  });
}

export function snippetsFor(dialect: Dialect, user: readonly Snippet[]): ReadySnippet[] {
  const mine = user.filter((s) => !s.dialect || s.dialect === dialect).map((s) => ({ ...s, builtin: false }));
  return [...mine, ...builtinSnippets(dialect)];
}

export function plain(body: string): string {
  return body.replace(/\$\{\d+:([^}]*)\}/g, '$1').replace(/\$\{\d+\}/g, '');
}
