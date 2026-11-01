import type { Api, ConnectionProfile, PlanReport, Schema } from './api';
import type { Capabilities, ObjectRef_Deserialize as ObjectRef } from './bindings';
import { relationalCaps } from './family';
import { DEFAULT_SESSION } from './session';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Engine = 'firebird' | 'db2' | 'hana' | 'odbc';

export function enterpriseCaps(driver: Engine): Capabilities {
  if (driver === 'odbc') {
    const c = relationalCaps('ansi');
    return { ...c, explain: false, tree: { databases: false, namespaces: 'multiple', groups: ['tables', 'views'], server: false }, tools: c.tools.filter((t) => ['compare', 'rows', 'transfer', 'search', 'generate'].includes(t)) };
  }
  const c = relationalCaps(driver);
  const tools = c.tools.filter((t) => t !== 'backup');
  if (driver === 'firebird') return { ...c, tree: { databases: true, namespaces: 'always', groups: ['tables', 'views', 'functions', 'sequences', 'triggers'], server: true }, tools };
  const groups: Capabilities['tree']['groups'] = driver === 'db2' ? ['tables', 'views', 'materialized_views', 'functions', 'sequences', 'triggers'] : ['tables', 'views', 'functions', 'sequences', 'triggers'];
  return { ...c, tree: { databases: false, namespaces: 'always', groups, server: true }, tools };
}

const profile = (id: string, name: string, target: ConnectionProfile['target']): ConnectionProfile => ({
  id,
  name,
  folder: null,
  environment: 'development',
  color: null,
  session: DEFAULT_SESSION,
  template: { kind: 'none' },
  raw_fields: {},
  target,
});

export const ENTERPRISE_DEMO: ConnectionProfile[] = params().has('enterprise')
  ? [
      profile('fb', 'Magazzino Firebird', { driver: 'firebird', host: '127.0.0.1', port: 53050, database: '/var/lib/firebird/data/gestionale.fdb', user: 'nicedb', role: null, ssh: null, proxy: null }),
      profile('db2', 'Gestione Db2', { driver: 'db2', host: '127.0.0.1', port: 55000, database: 'GESTION', user: 'db2inst1', tls: 'disable', ca_file: null, ssh: null, proxy: null }),
      profile('hana', 'Contabilità HANA', { driver: 'hana', host: 'hana.bassi.lan', port: 30041, database: 'HXE', user: 'NICEDB', tls: 'verify', ca_file: null, ssh: null, proxy: null }),
      profile('odbc', 'Magazzino Informix', { driver: 'odbc', dsn: 'MAGAZZINO', driver_path: '', connect_string: '', user: 'informix' }),
    ]
  : [];

const col = (name: string, type_name: string, primary_key = false, nullable = !primary_key) => ({ name, type_name, nullable, primary_key });
const ix = (name: string, columns: string[], unique = false) => ({ name, columns, unique });
const fk = (name: string, column: string, ns: string, table: string) => ({ name, columns: [column], ref_namespace: ns, ref_table: table, ref_columns: ['ID'], on_delete: 'no_action' as const, on_update: 'no_action' as const });
const rel = (name: string, rows: number | null, columns: ReturnType<typeof col>[], indexes: ReturnType<typeof ix>[] = [], foreign_keys: ReturnType<typeof fk>[] = []) => ({ name, columns, indexes, foreign_keys, estimated_rows: rows });

function gestionale(ns: string, t: { int: string; text: (n: number) => string; money: string; date: string }, pk: (table: string) => string) {
  return {
    name: ns,
    tables: [
      rel('ARTICOLI', 4000, [col('ID', t.int, true), col('CODICE', t.text(20), false, false), col('DESCRIZIONE', t.text(200), false, false), col('PREZZO_LISTINO', t.money, false, false), col('GIACENZA', t.int, false, false)], [ix(pk('ARTICOLI'), ['ID'], true), ix('IX_ARTICOLI_CODICE', ['CODICE'], true)]),
      rel('CLIENTI', 2400, [col('ID', t.int, true), col('RAGIONE_SOCIALE', t.text(200), false, false), col('PARTITA_IVA', t.text(11), false, false), col('CITTA', t.text(80), false, false), col('PROVINCIA', t.text(2), false, false), col('FIDO', t.money, false, false)], [ix(pk('CLIENTI'), ['ID'], true)]),
      rel('ORDINI', 120000, [col('ID', t.int, true), col('NUMERO', t.text(20), false, false), col('DATA', t.date, false, false), col('CLIENTE_ID', t.int, false, false), col('TOTALE', t.money, false, false)], [ix(pk('ORDINI'), ['ID'], true), ix('IX_ORDINI_CLIENTE', ['CLIENTE_ID'])], [fk('FK_ORDINI_CLIENTE', 'CLIENTE_ID', ns, 'CLIENTI')]),
      rel('RIGHE_ORDINE', 1200000, [col('ID', t.int, true), col('ORDINE_ID', t.int, false, false), col('ARTICOLO_ID', t.int, false, false), col('QUANTITA', t.money, false, false), col('PREZZO', t.money, false, false)], [ix(pk('RIGHE_ORDINE'), ['ID'], true)], [fk('FK_RIGHE_ORDINE', 'ORDINE_ID', ns, 'ORDINI'), fk('FK_RIGHE_ARTICOLO', 'ARTICOLO_ID', ns, 'ARTICOLI')]),
    ],
    views: [rel('V_ORDINI_APERTI', null, [col('NUMERO', t.text(20)), col('RAGIONE_SOCIALE', t.text(200)), col('TOTALE', t.money)])],
    functions: [{ name: 'GIACENZA_TOTALE', signature: '' }],
    sequences: [{ name: 'SEQ_PROTOCOLLO', detail: null }],
    types: [],
    triggers: [{ name: 'TR_ORDINI_TOTALE', detail: 'RIGHE_ORDINE' }],
  };
}

function schemaOf(driver: Engine): Schema {
  if (driver === 'firebird') {
    return { databases: ['gestionale.fdb'], current_database: 'gestionale.fdb', namespaces: [gestionale('PUBLIC', { int: 'INTEGER', text: (n) => `VARCHAR(${n})`, money: 'NUMERIC(12, 2)', date: 'DATE' }, (t) => `PK_${t}`)] };
  }
  if (driver === 'db2') {
    const own = gestionale('DB2INST1', { int: 'INTEGER', text: (n) => `VARCHAR(${n})`, money: 'DECIMAL(12,2)', date: 'DATE' }, (t) => `SQL2611011${t.length}`);
    return { databases: [], current_database: 'GESTION', namespaces: [{ ...own, materialized_views: [rel('FATTURATO_MENSILE', 34, [col('MESE', 'DATE'), col('TOTALE', 'DECIMAL(31,2)')])] }, { name: 'CONTABILITA', tables: [rel('MOVIMENTI', 410000, [col('ID', 'BIGINT', true), col('CONTO', 'VARCHAR(12)', false, false), col('DARE', 'DECIMAL(14,2)'), col('AVERE', 'DECIMAL(14,2)')], [ix('SQL261101120044', ['ID'], true)])], views: [], functions: [], sequences: [], types: [], triggers: [] }] };
  }
  if (driver === 'hana') {
    return { databases: [], current_database: 'HXE', namespaces: [gestionale('NICEDB', { int: 'INTEGER', text: (n) => `NVARCHAR(${n})`, money: 'DECIMAL(12,2)', date: 'DATE' }, (t) => `_SYS_TREE_CS_#${t.length}_#PK`)] };
  }
  return { databases: [], current_database: 'magazzino', namespaces: [gestionale('informix', { int: 'integer', text: (n) => `varchar(${n})`, money: 'decimal(12,2)', date: 'date' }, (t) => `u${t.length}_1`)] };
}

const DDL: Record<string, string> = {
  IX_ORDINI_CLIENTE: 'CREATE INDEX IX_ORDINI_CLIENTE ON ORDINI (CLIENTE_ID);',
  PK_CLIENTI: 'ALTER TABLE CLIENTI ADD CONSTRAINT PK_CLIENTI PRIMARY KEY (ID) USING INDEX PK_CLIENTI;',
};

const FB_PLAN = `Select Expression
    -> Nested Loop Join (inner)
        -> Filter
            -> Table "CLIENTI" as "C" Full Scan
        -> Filter
            -> Table "ORDINI" as "O" Access By ID
                -> Bitmap
                    -> Index "IX_ORDINI_CLIENTE" Range Scan (full match)`;

type Node = PlanReport['plan']['nodes'][number];
const node = (id: number, parent: number | null, depth: number, op: string, kind: Node['kind'], extra: Partial<Node> = {}): Node => ({
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

function plan(driver: Engine, req: { text: string; mode: string }): PlanReport {
  const nodes =
    driver === 'firebird'
      ? [
          node(0, null, 0, 'Select Expression', 'result'),
          node(1, 0, 1, 'Nested Loop Join (inner)', 'nested_loop'),
          node(2, 1, 2, 'Filter', 'other'),
          node(3, 2, 3, 'Table "CLIENTI" as "C" Full Scan', 'full_scan', { relation: 'CLIENTI', alias: 'C' }),
          node(4, 1, 2, 'Filter', 'other'),
          node(5, 4, 3, 'Table "ORDINI" as "O" Access By ID', 'bitmap_scan', { relation: 'ORDINI', alias: 'O' }),
          node(6, 5, 4, 'Bitmap', 'other'),
          node(7, 6, 5, 'Index "IX_ORDINI_CLIENTE" Range Scan (full match)', 'bitmap_index', { index: 'IX_ORDINI_CLIENTE' }),
        ]
      : [
          node(0, null, 0, 'RETURN', 'result', { est_rows: 1180, total_cost: 412.6 }),
          node(1, 0, 1, 'HSJOIN', 'hash_join', { est_rows: 1180, total_cost: 412.1 }),
          node(2, 1, 2, 'TBSCAN', 'full_scan', { relation: 'CLIENTI', est_rows: 40, total_cost: 14.2 }),
          node(3, 1, 2, 'FETCH', 'bitmap_scan', { relation: 'ORDINI', est_rows: 120000, total_cost: 396.8 }),
          node(4, 3, 3, 'IXSCAN', 'index_scan', { index: 'IX_ORDINI_CLIENTE', est_rows: 120000, total_cost: 88.4 }),
        ];
  for (const n of nodes) if (n.parent !== null) nodes[n.parent]!.children.push(n.id);
  return {
    plan: { engine: driver === 'firebird' ? 'firebird' : 'db2', analyzed: false, nodes, total_ms: null, planning_ms: null, slowest: null, worst_estimate: null, raw: driver === 'firebird' ? FB_PLAN : nodes.map((n) => `${n.id}\t${n.parent ?? ''}\t${n.op}`).join('\n') },
    findings: [],
    execution: { kind: 'estimated', reason: req.mode === 'estimate' ? 'requested' : 'engine_cannot_measure' },
    unreadable: null,
    sql: req.text.trim().replace(/;$/, ''),
    from: 0,
    to: req.text.length,
    dialect: driver === 'firebird' ? 'firebird' : 'db2',
    elapsed_ms: 18,
    at: Date.now(),
  } as PlanReport;
}

export function enterpriseMock(base: Api, profileOf: (sessionId: string) => ConnectionProfile | undefined): Partial<Api> {
  const engine = (sid: string): Engine | null => {
    const d = profileOf(sid.split('@')[0] ?? sid)?.target.driver;
    return d === 'firebird' || d === 'db2' || d === 'hana' || d === 'odbc' ? d : null;
  };
  return {
    schemaLoad: async (sid) => {
      const e = engine(sid);
      return e ? (await sleep(40), schemaOf(e)) : base.schemaLoad(sid);
    },
    objectDdl: async (sid, object: ObjectRef) => (engine(sid) ? (DDL[object.name] ?? `CREATE TABLE ${object.name} (\n  ID INTEGER NOT NULL,\n  PRIMARY KEY (ID)\n);`) : base.objectDdl(sid, object)),
    explain: async (req) => {
      const e = engine(req.session_id);
      return e === 'firebird' || e === 'db2' ? (await sleep(120), plan(e, req)) : base.explain(req);
    },
  };
}
