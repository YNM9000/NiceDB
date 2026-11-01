import type { Api, ConnectionProfile, PlanReport, RunEvent, RunRequest, Schema } from './api';
import type { Activity_Serialize as Activity, Capabilities, Monitor_Serialize as Monitor, ObjectRef_Deserialize as ObjectRef, Page_Serialize as Page, Principals_Serialize as Principals, Sizes_Serialize as Sizes, SlowQueries_Serialize as SlowQueries } from './bindings';
import { relationalCaps } from './family';
import { DEFAULT_SESSION } from './session';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

export function oracleCaps(): Capabilities {
  const c = relationalCaps('oracle');
  return {
    ...c,
    tree: { databases: false, namespaces: 'always', groups: ['tables', 'views', 'materialized_views', 'functions', 'packages', 'sequences', 'synonyms', 'types', 'triggers'], server: true },
    tools: c.tools.filter((t) => t !== 'backup'),
    cancel: params().get('oracle') === 'admin' ? 'server' : 'between_fetches',
  };
}

export const ORACLE_DEMO: ConnectionProfile[] = params().has('oracle')
  ? [
      {
        id: 'oracle',
        name: 'Gestionale Oracle',
        folder: null,
        environment: params().get('oracle') === 'prod' ? 'production' : 'development',
        color: null,
        session: DEFAULT_SESSION,
        template: { kind: 'none' },
        raw_fields: {},
        target: { driver: 'oracle', host: '127.0.0.1', port: 51521, database: 'FREEPDB1', sid: false, connect_string: '', user: 'nicedb', tls: 'disable', ca_file: null, role: 'normal', ssh: null, proxy: null },
      },
    ]
  : [];

const col = (name: string, type_name: string, primary_key = false, nullable = !primary_key) => ({ name, type_name, nullable, primary_key });
const ix = (name: string, columns: string[], unique = false) => ({ name, columns, unique });
const fk = (name: string, column: string, table: string) => ({ name, columns: [column], ref_namespace: 'NICEDB', ref_table: table, ref_columns: ['ID'], on_delete: 'no_action' as const, on_update: 'no_action' as const });
const rel = (name: string, rows: number | null, columns: ReturnType<typeof col>[], indexes: ReturnType<typeof ix>[] = [], foreign_keys: ReturnType<typeof fk>[] = []) => ({ name, columns, indexes, foreign_keys, estimated_rows: rows });

export function oracleSchema(): Schema {
  return {
    databases: ['FREEPDB1'],
    current_database: 'FREEPDB1',
    namespaces: [
      {
        name: 'NICEDB',
        tables: [
          rel('AGENTI', 8, [col('ID', 'NUMBER(10)', true), col('NOME', 'VARCHAR2(60 CHAR)', false, false), col('COGNOME', 'VARCHAR2(60 CHAR)', false, false), col('ZONA', 'VARCHAR2(60 CHAR)', false, false), col('PROVVIGIONE', 'NUMBER(4,2)', false, false)], [ix('SYS_C008412', ['ID'], true)]),
          rel('ARTICOLI', 4000, [col('ID', 'NUMBER(10)', true), col('CODICE', 'VARCHAR2(20 CHAR)', false, false), col('DESCRIZIONE', 'VARCHAR2(200 CHAR)', false, false), col('UM', 'VARCHAR2(8 CHAR)', false, false), col('PREZZO_LISTINO', 'NUMBER(10,2)', false, false), col('ALIQUOTA_IVA', 'NUMBER(3)', false, false), col('GIACENZA', 'NUMBER(10)', false, false), col('FORNITORE_ID', 'NUMBER(10)')], [ix('SYS_C008421', ['ID'], true), ix('SYS_C008422', ['CODICE'], true)], [fk('SYS_C008423', 'FORNITORE_ID', 'FORNITORI')]),
          rel('CLIENTI', 2400, [col('ID', 'NUMBER(10)', true), col('RAGIONE_SOCIALE', 'VARCHAR2(200 CHAR)', false, false), col('PARTITA_IVA', 'CHAR(11)', false, false), col('CODICE_FISCALE', 'CHAR(11)', false, false), col('CITTA', 'VARCHAR2(80 CHAR)', false, false), col('PROVINCIA', 'CHAR(2)', false, false), col('AGENTE_ID', 'NUMBER(10)'), col('FIDO', 'NUMBER(12,2)', false, false), col('CREATO_IL', 'TIMESTAMP(6) WITH TIME ZONE', false, false)], [ix('SYS_C008414', ['ID'], true), ix('SYS_C008415', ['PARTITA_IVA'], true)], [fk('SYS_C008416', 'AGENTE_ID', 'AGENTI')]),
          rel('FATTURE', 90000, [col('ID', 'NUMBER(10)', true), col('NUMERO', 'VARCHAR2(20 CHAR)', false, false), col('DATA', 'DATE', false, false), col('CLIENTE_ID', 'NUMBER(10)', false, false), col('IMPONIBILE', 'NUMBER(12,2)', false, false), col('IVA', 'NUMBER(12,2)', false, false), col('TOTALE', 'NUMBER(12,2)', false, false), col('SCADENZA', 'DATE', false, false), col('PAGATA', 'BOOLEAN', false, false), col('NOTE', 'JSON')], [ix('SYS_C008440', ['ID'], true), ix('FATTURE_CLIENTE', ['CLIENTE_ID'])], [fk('SYS_C008442', 'CLIENTE_ID', 'CLIENTI')]),
          rel('FORNITORI', 60, [col('ID', 'NUMBER(10)', true), col('RAGIONE_SOCIALE', 'VARCHAR2(200 CHAR)', false, false), col('PARTITA_IVA', 'CHAR(11)', false, false), col('CITTA', 'VARCHAR2(80 CHAR)', false, false)], [ix('SYS_C008418', ['ID'], true)]),
          rel('ORDINI', 120000, [col('ID', 'NUMBER(10)', true), col('NUMERO', 'VARCHAR2(20 CHAR)', false, false), col('DATA', 'DATE', false, false), col('CLIENTE_ID', 'NUMBER(10)', false, false), col('STATO', 'VARCHAR2(12 CHAR)', false, false), col('TOTALE', 'NUMBER(12,2)', false, false)], [ix('SYS_C008428', ['ID'], true), ix('ORDINI_CLIENTE', ['CLIENTE_ID'])], [fk('SYS_C008430', 'CLIENTE_ID', 'CLIENTI')]),
          rel('PAGAMENTI', 72000, [col('ID', 'NUMBER(10)', true), col('FATTURA_ID', 'NUMBER(10)', false, false), col('DATA', 'DATE', false, false), col('IMPORTO', 'NUMBER(12,2)', false, false), col('METODO', 'VARCHAR2(12 CHAR)', false, false)], [ix('SYS_C008446', ['ID'], true), ix('PAGAMENTI_FATTURA', ['FATTURA_ID'])], [fk('SYS_C008447', 'FATTURA_ID', 'FATTURE')]),
          rel('RIGHE_ORDINE', 1200000, [col('ID', 'NUMBER(19)', true), col('ORDINE_ID', 'NUMBER(10)', false, false), col('ARTICOLO_ID', 'NUMBER(10)', false, false), col('QUANTITA', 'NUMBER(10,2)', false, false), col('PREZZO', 'NUMBER(10,2)', false, false), col('SCONTO', 'NUMBER(4,2)', false, false)], [ix('SYS_C008434', ['ID'], true)], [fk('SYS_C008435', 'ORDINE_ID', 'ORDINI'), fk('SYS_C008436', 'ARTICOLO_ID', 'ARTICOLI')]),
        ],
        views: [rel('V_SCADENZARIO', null, [col('NUMERO', 'VARCHAR2(20 CHAR)'), col('RAGIONE_SOCIALE', 'VARCHAR2(200 CHAR)'), col('TOTALE', 'NUMBER(12,2)'), col('SCADENZA', 'DATE'), col('GIORNI', 'NUMBER')])],
        materialized_views: [rel('FATTURATO_MENSILE', 34, [col('MESE', 'DATE'), col('FATTURE', 'NUMBER'), col('TOTALE', 'NUMBER')])],
        functions: [{ name: 'TOTALE_CLIENTE', signature: 'CID NUMBER' }],
        packages: [{ name: 'GESTIONE', detail: 'body' }],
        sequences: [{ name: 'PROTOCOLLO', detail: null }],
        synonyms: [{ name: 'CLI', detail: 'NICEDB.CLIENTI' }],
        types: [{ name: 'T_INDIRIZZO', detail: 'OBJECT' }],
        triggers: [{ name: 'PAGAMENTI_DATA', detail: 'PAGAMENTI' }],
      },
      {
        name: 'CONTABILITA',
        tables: [rel('MOVIMENTI', 410000, [col('ID', 'NUMBER(19)', true), col('DATA', 'DATE', false, false), col('CONTO', 'VARCHAR2(12 CHAR)', false, false), col('DARE', 'NUMBER(14,2)'), col('AVERE', 'NUMBER(14,2)')], [ix('MOVIMENTI_PK', ['ID'], true)])],
        views: [],
        functions: [],
        sequences: [],
        types: [],
        triggers: [],
      },
    ],
  };
}

const DDL: Record<string, string> = {
  CLIENTI: `CREATE TABLE "NICEDB"."CLIENTI"
   (\t"ID" NUMBER(10,0) GENERATED BY DEFAULT ON NULL AS IDENTITY MINVALUE 1 MAXVALUE 9999999999999999999999999999 INCREMENT BY 1 START WITH 1 CACHE 20 NOORDER  NOCYCLE  NOKEEP  NOSCALE  NOT NULL ENABLE,
\t"RAGIONE_SOCIALE" VARCHAR2(200 CHAR) NOT NULL ENABLE,
\t"PARTITA_IVA" CHAR(11 BYTE) NOT NULL ENABLE,
\t"CODICE_FISCALE" CHAR(11 BYTE) NOT NULL ENABLE,
\t"CITTA" VARCHAR2(80 CHAR) NOT NULL ENABLE,
\t"PROVINCIA" CHAR(2 BYTE) NOT NULL ENABLE,
\t"AGENTE_ID" NUMBER(10,0),
\t"FIDO" NUMBER(12,2) DEFAULT 0 NOT NULL ENABLE,
\t"CREATO_IL" TIMESTAMP (6) WITH TIME ZONE DEFAULT SYSTIMESTAMP NOT NULL ENABLE,
\t PRIMARY KEY ("ID")
  USING INDEX  ENABLE,
\t UNIQUE ("PARTITA_IVA")
  USING INDEX  ENABLE,
\t FOREIGN KEY ("AGENTE_ID")
\t  REFERENCES "NICEDB"."AGENTI" ("ID") ENABLE
   ) ;`,
  GESTIONE: `CREATE OR REPLACE EDITIONABLE PACKAGE "NICEDB"."GESTIONE" AS
    FUNCTION giacenza_totale RETURN NUMBER;
    PROCEDURE chiudi_ordine (oid IN NUMBER);
END gestione;
/
CREATE OR REPLACE EDITIONABLE PACKAGE BODY "NICEDB"."GESTIONE" AS
    FUNCTION giacenza_totale RETURN NUMBER IS
        n NUMBER;
    BEGIN
        SELECT SUM(giacenza) INTO n FROM articoli;
        RETURN n;
    END;

    PROCEDURE chiudi_ordine (oid IN NUMBER) IS
    BEGIN
        UPDATE ordini SET stato = 'chiuso' WHERE id = oid;
    END;
END gestione;
/`,
};

const STATI = ['confermato', 'spedito', 'chiuso', 'chiuso', 'chiuso', 'annullato', 'bozza'];
const day = (i: number) => new Date(Date.UTC(2024, 0, 1) + (i % 1000) * 86400000).toISOString().slice(0, 10);

function rowsOf(table: string, from: number, count: number): { columns: [string, string][]; rows: (string | null)[][] } {
  if (table === 'CLIENTI') {
    const nomi = ['Edilnord', 'Termoidraulica', 'Officine Meccaniche', 'Serramenti', 'Carpenteria', 'Falegnameria', 'Colorificio', 'Impianti Elettrici', 'Autofficina', 'Cantina'];
    const citta = ['Bergamo', 'Brescia', 'Seriate', 'Lumezzane', 'Dalmine', 'Treviglio', 'Chiari', 'Rovato', "Palazzolo sull'Oglio", 'Gussago'];
    return {
      columns: [['ID', 'NUMBER(10)'], ['RAGIONE_SOCIALE', 'VARCHAR2(800)'], ['CITTA', 'VARCHAR2(320)'], ['PROVINCIA', 'CHAR(2)'], ['FIDO', 'NUMBER(12,2)'], ['CREATO_IL', 'TIMESTAMP(6) WITH TIME ZONE']],
      rows: Array.from({ length: count }, (_, k) => {
        const i = from + k + 1;
        return [String(i), `${nomi[i % 10]} S.r.l.${i > 100 ? ` ${i}` : ''}`, citta[i % 10] ?? '', i % 2 ? 'BS' : 'BG', String((5 + (i % 20)) * 1000), `2026-${String(1 + (i % 10)).padStart(2, '0')}-${String(1 + (i % 28)).padStart(2, '0')} 08:${String(i % 60).padStart(2, '0')}:00.000000 +00:00`];
      }),
    };
  }
  return {
    columns: [['ID', 'NUMBER(10)'], ['NUMERO', 'VARCHAR2(80)'], ['DATA', 'DATE'], ['CLIENTE_ID', 'NUMBER(10)'], ['STATO', 'VARCHAR2(48)'], ['TOTALE', 'NUMBER(12,2)']],
    rows: Array.from({ length: count }, (_, k) => {
      const i = from + k + 1;
      return [String(i), `2026/${String(i).padStart(6, '0')}`, day(i), String(1 + ((i * 31) % 2400)), STATI[i % 7] ?? 'bozza', (((i * 37) % 9000) + 120.5).toFixed(2)];
    }),
  };
}

const running = new Map<string, () => void>();

function statements(text: string): { from: number; to: number; sql: string }[] {
  const out: { from: number; to: number; sql: string }[] = [];
  const re = /^\s*\/\s*$/m;
  let start = 0;
  const push = (end: number, keep: boolean) => {
    const raw = text.slice(start, end);
    const lead = raw.length - raw.trimStart().length;
    let sql = raw.trim();
    if (!keep) sql = sql.replace(/;$/, '').trim();
    if (sql) out.push({ from: start + lead, to: start + lead + sql.length, sql });
  };
  while (start < text.length) {
    const rest = text.slice(start);
    const block = /^\s*(declare|begin|create\s+(or\s+replace\s+)?(procedure|function|package|trigger|type))\b/i.test(rest);
    if (block) {
      const slash = re.exec(rest);
      const end = slash ? start + slash.index : text.length;
      push(end, true);
      start = slash ? end + slash[0].length : text.length;
      continue;
    }
    const semi = rest.indexOf(';');
    const end = semi < 0 ? text.length : start + semi;
    push(end, false);
    start = end + 1;
  }
  return out;
}

async function run(req: RunRequest, emit: (e: RunEvent) => void) {
  let cancelled = false;
  running.set(req.run_id, () => (cancelled = true));
  const started = performance.now();
  const all = statements(req.text);
  const s = req.scope;
  const chosen = s.kind === 'script' ? all : s.kind === 'statement' ? [all.find((x) => x.from <= s.cursor && s.cursor <= x.to + 1) ?? all[all.length - 1]].filter((x) => !!x) : all.filter((x) => x.to > Math.min(s.from, s.to) && x.from < Math.max(s.from, s.to));
  for (const [index, st] of chosen.entries()) {
    if (cancelled) break;
    emit({ type: 'statement', index, from: st.from, to: st.to, sql: st.sql, unsortable: null });
    await sleep(40);
    if (!/^\s*(select|with)\b/i.test(st.sql)) {
      const dml = /^\s*(insert|update|delete|merge)\b/i.test(st.sql);
      emit({ type: 'done', index, elapsed_ms: 14, row_count: 0, rows_affected: dml ? 3 : null, command: (/^\s*(\w+)/.exec(st.sql)?.[1] ?? '').toUpperCase(), truncated: false, row_limit: null });
      continue;
    }
    const table = /\bfrom\s+(?:"?\w+"?\.)?"?(\w+)"?/i.exec(st.sql)?.[1]?.toUpperCase() ?? 'ORDINI';
    const limit = Number(/\bfetch\s+first\s+(\d+)/i.exec(st.sql)?.[1] ?? 1000);
    const got = rowsOf(table, 0, Math.min(limit, 2000));
    emit({ type: 'columns', index, columns: got.columns.map(([name, type_name]) => ({ name, type_name })) });
    emit({ type: 'rows', index, rows: got.rows });
    emit({ type: 'done', index, elapsed_ms: 31, row_count: got.rows.length, rows_affected: null, command: null, truncated: false, row_limit: null });
  }
  running.delete(req.run_id);
  emit({ type: 'finished', elapsed_ms: Math.round(performance.now() - started), cancelled });
}

const node = (id: number, parent: number | null, depth: number, op: string, kind: PlanReport['plan']['nodes'][number]['kind'], extra: Partial<PlanReport['plan']['nodes'][number]> = {}): PlanReport['plan']['nodes'][number] => ({
  id,
  parent,
  depth,
  children: [],
  op,
  kind,
  relation: null,
  alias: null,
  index: null,
  condition: null,
  filter: null,
  est_rows: null,
  rows: null,
  loops: null,
  rows_removed: null,
  est_examined: null,
  startup_cost: null,
  total_cost: null,
  time_ms: null,
  self_ms: null,
  width: null,
  buffers: null,
  spill: null,
  parallel: false,
  details: [],
  ...extra,
});

function plan(req: { text: string; mode: string }): PlanReport {
  const asked = req.mode !== 'estimate';
  const measured = asked && params().get('oracle') === 'admin';
  const nodes = [
    node(0, null, 0, 'SELECT STATEMENT', 'result', { est_rows: 12000, total_cost: 412, ...(measured ? { rows: 11760, loops: 1, time_ms: 184.2 } : {}) }),
    node(1, 0, 1, 'HASH JOIN', 'hash_join', { est_rows: 12000, total_cost: 412, condition: '"C"."ID"="O"."CLIENTE_ID"', ...(measured ? { rows: 11760, loops: 1, time_ms: 184.2, buffers: { hit: 1830, read: 0, written: 0, temp_read: 0, temp_written: 0 } } : {}) }),
    node(2, 1, 2, 'TABLE ACCESS FULL', 'full_scan', { relation: 'CLIENTI', est_rows: 1200, total_cost: 14, filter: '"C"."PROVINCIA"=\'BG\'', ...(measured ? { rows: 1200, loops: 1, time_ms: 1.4 } : {}) }),
    node(3, 1, 2, 'TABLE ACCESS FULL', 'full_scan', { relation: 'ORDINI', est_rows: 120000, total_cost: 397, ...(measured ? { rows: 120000, loops: 1, time_ms: 96.7, buffers: { hit: 1790, read: 0, written: 0, temp_read: 0, temp_written: 0 } } : {}) }),
  ];
  nodes[0]!.children = [1];
  nodes[1]!.children = [2, 3];
  if (measured) {
    nodes[0]!.self_ms = 0;
    nodes[1]!.self_ms = 86.1;
    nodes[2]!.self_ms = 1.4;
    nodes[3]!.self_ms = 96.7;
  }
  const raw = `Plan hash value: 2341252972\n\n------------------------------------------------------------------------------\n| Id  | Operation          | Name    | Rows  | Bytes | Cost (%CPU)| Time     |\n------------------------------------------------------------------------------\n|   0 | SELECT STATEMENT   |         | 12000 |   480K|   412   (1)| 00:00:01 |\n|*  1 |  HASH JOIN         |         | 12000 |   480K|   412   (1)| 00:00:01 |\n|*  2 |   TABLE ACCESS FULL| CLIENTI |  1200 | 30000 |    14   (0)| 00:00:01 |\n|   3 |   TABLE ACCESS FULL| ORDINI  |   120K|  1758K|   397   (1)| 00:00:01 |\n------------------------------------------------------------------------------`;
  return {
    plan: { engine: 'oracle', analyzed: measured, nodes, total_ms: measured ? 184.2 : null, planning_ms: null, slowest: measured ? 3 : null, worst_estimate: null, raw },
    findings: [],
    execution: measured ? { kind: 'measured' } : { kind: 'estimated', reason: asked ? 'no_privilege' : 'requested' },
    unreadable: null,
    sql: req.text.trim().replace(/;$/, ''),
    from: 0,
    to: req.text.length,
    dialect: 'oracle',
    elapsed_ms: 190,
    at: Date.now(),
  } as PlanReport;
}

const SIZES: Sizes = {
  usage_known: false,
  tables: [
    { namespace: 'NICEDB', name: 'RIGHE_ORDINE', rows: 1_200_000, data_bytes: 52 * MB, index_bytes: 21 * MB, total_bytes: 73 * MB },
    { namespace: 'CONTABILITA', name: 'MOVIMENTI', rows: 410_000, data_bytes: 18 * MB, index_bytes: 7 * MB, total_bytes: 25 * MB },
    { namespace: 'NICEDB', name: 'FATTURE', rows: 90_000, data_bytes: 8 * MB, index_bytes: 3 * MB, total_bytes: 11 * MB },
    { namespace: 'NICEDB', name: 'ORDINI', rows: 120_000, data_bytes: 6 * MB, index_bytes: 4 * MB, total_bytes: 10 * MB },
    { namespace: 'NICEDB', name: 'CLIENTI', rows: 2400, data_bytes: 0.4 * MB, index_bytes: 0.13 * MB, total_bytes: 0.53 * MB },
  ],
  indexes: [
    { namespace: 'NICEDB', table: 'RIGHE_ORDINE', name: 'SYS_C008434', bytes: 21 * MB, scans: null, unique: true, unused: false },
    { namespace: 'NICEDB', table: 'ORDINI', name: 'ORDINI_CLIENTE', bytes: 2.1 * MB, scans: null, unique: false, unused: false },
    { namespace: 'NICEDB', table: 'FATTURE', name: 'FATTURE_CLIENTE', bytes: 1.6 * MB, scans: null, unique: false, unused: false },
  ],
};

const MONITOR: Monitor = {
  connections: { total: 9, active: 2, idle_in_transaction: 1, waiting: 0, max: 472 },
  transactions: [{ id: '412,18803', user: 'CONTABILE', database: 'CONTABILITA', age_ms: 92_000, state: 'inactive', query: "UPDATE movimenti SET conto = '60.01.02' WHERE id = :1" }],
  databases: [{ name: 'FREEPDB1', bytes: 1.84 * 1024 * MB }],
  cache_hit: 0.987,
  long_ops: [
    { session: '388,52211', operation: 'Table Scan', target: 'CONTABILITA.MOVIMENTI', progress: 0.42, elapsed_ms: 38_000, remaining_ms: 52_000 },
    { session: '402,1177', operation: 'Gather Schema Statistics', target: null, progress: 0.8, elapsed_ms: 91_000, remaining_ms: 23_000 },
  ],
  gaps: [],
};

const SLOW: SlowQueries = {
  source: 'plan_cache',
  gap: null,
  queries: [
    { text: 'SELECT c.ragione_sociale, SUM(f.totale) FROM fatture f JOIN clienti c ON c.id = f.cliente_id GROUP BY c.ragione_sociale ORDER BY 2 DESC', sample: null, calls: 38, total_ms: 11_420, mean_ms: 300.5, rows: 91_200 },
    { text: 'SELECT * FROM ordini WHERE cliente_id = :1 ORDER BY data DESC FETCH FIRST 50 ROWS ONLY', sample: null, calls: 1840, total_ms: 4_310, mean_ms: 2.3, rows: 92_000 },
  ],
};

const ACTIVITY: Activity = {
  sessions: [
    { id: '388,52211', user: 'CONTABILE', database: 'CONTABILITA', client: 'ufficio-07', application: 'SQL Developer', state: 'active', query: 'SELECT conto, SUM(dare) - SUM(avere) FROM movimenti GROUP BY conto', duration_ms: 38_000, wait: 'User I/O: db file scattered read', blocked_by: [] },
    { id: '412,18803', user: 'CONTABILE', database: 'CONTABILITA', client: 'ufficio-07', application: 'SQL Developer', state: 'inactive', query: "UPDATE movimenti SET conto = '60.01.02' WHERE id = :1", duration_ms: null, wait: null, blocked_by: [] },
    { id: '415,2290', user: 'NICEDB', database: 'NICEDB', client: 'magazzino', application: 'NiceDB', state: 'active', query: "UPDATE movimenti SET dare = 0 WHERE conto = '60.01.02'", duration_ms: 12_400, wait: 'Application: enq: TX - row lock contention', blocked_by: ['412,18803'] },
  ],
  locks: [{ waiting: '415,2290', blocking: '412,18803', object: 'CONTABILITA.MOVIMENTI', mode: 'enq: TX - row lock contention', waited_ms: 12_400 }],
  partial: false,
  can_cancel: true,
};

const PRINCIPALS: Principals = {
  principals: [
    { name: 'CONTABILE', host: null, attributes: ['login'], member_of: ['LETTURA_GESTIONALE'] },
    { name: 'LETTURA_GESTIONALE', host: null, attributes: [], member_of: [] },
    { name: 'NICEDB', host: null, attributes: ['login', 'create_role'], member_of: ['DB_DEVELOPER_ROLE', 'RESOURCE'] },
    { name: 'SYSTEM', host: null, attributes: ['login', 'superuser', 'create_db', 'create_role'], member_of: ['DBA'] },
  ],
  grants: [
    { grantee: 'LETTURA_GESTIONALE', kind: 'table', schema: 'NICEDB', object: 'CLIENTI', privilege: 'SELECT', grantable: false },
    { grantee: 'LETTURA_GESTIONALE', kind: 'table', schema: 'NICEDB', object: 'ORDINI', privilege: 'SELECT', grantable: false },
    { grantee: 'CONTABILE', kind: 'global', schema: null, object: null, privilege: 'CREATE SESSION', grantable: false },
    { grantee: 'NICEDB', kind: 'global', schema: null, object: null, privilege: 'CREATE SESSION', grantable: false },
  ],
  partial: false,
  quotas: [],
};

export function oracleMock(base: Api, profile: (sessionId: string) => ConnectionProfile | undefined): Partial<Api> {
  const mine = (sid: string) => profile(sid.split('@')[0] ?? sid)?.target.driver === 'oracle';
  return {
    schemaLoad: async (sid) => (mine(sid) ? (await sleep(60), oracleSchema()) : base.schemaLoad(sid)),
    objectDdl: async (sid, object: ObjectRef) => (mine(sid) ? (DDL[object.name] ?? `CREATE TABLE "${object.namespace}"."${object.name}"\n   (\t"ID" NUMBER(10,0) NOT NULL ENABLE,\n\t PRIMARY KEY ("ID")\n  USING INDEX  ENABLE\n   ) ;`) : base.objectDdl(sid, object)),
    run: (req, emit) => (mine(req.session_id) ? run(req, emit) : base.run(req, emit)),
    runCancel: async (id) => {
      const stop = running.get(id);
      if (stop) return stop();
      return base.runCancel(id);
    },
    explain: async (req) => (mine(req.session_id) ? (await sleep(220), plan(req)) : base.explain(req)),
    dataBrowse: async (sid, b): Promise<Page> => {
      if (!mine(sid)) return base.dataBrowse(sid, b);
      await sleep(50);
      const got = rowsOf(b.table.name, b.offset ?? 0, b.limit ?? 100);
      return { columns: got.columns.map(([name, type_name]) => ({ name, type_name })), rows: got.rows, has_more: true, next: null, total: b.table.name === 'CLIENTI' ? 2400 : 120000, estimated: false, sql: `SELECT * FROM "${b.table.namespace}"."${b.table.name}" OFFSET ${b.offset ?? 0} ROWS FETCH NEXT ${b.limit} ROWS ONLY`, elapsed_ms: 42 };
    },
    serverSizes: async (sid) => (mine(sid) ? structuredClone(SIZES) : base.serverSizes(sid)),
    serverMonitor: async (sid) => (mine(sid) ? structuredClone(MONITOR) : base.serverMonitor(sid)),
    serverSlowQueries: async (sid) => (mine(sid) ? structuredClone(SLOW) : base.serverSlowQueries(sid)),
    serverActivity: async (sid) => (mine(sid) ? structuredClone(ACTIVITY) : base.serverActivity(sid)),
    principalsList: async (sid) => (mine(sid) ? structuredClone(PRINCIPALS) : base.principalsList(sid)),
  };
}
