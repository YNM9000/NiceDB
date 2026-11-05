import type { Api, ConnectionProfile, ExplainRequest, PlanReport, RunEvent, RunRequest, Schema } from './api';
import type { Capabilities, DocumentWrite, Node_Serialize as Node, ObjectRef_Deserialize as ObjectRef, Report } from './bindings';
import { consoleCommands, indexOf, parseRequest } from './nosql-lang';
import { DEFAULT_SESSION } from './session';

type Driver = 'cassandra' | 'dynamodb' | 'elasticsearch';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function nosqlCaps(driver: Driver): Capabilities {
  const base = { schema: 'inferred', results: 'documents', transactions: false, editable: true, er: false, ddl: true, sort: false, params: false, tools: ['cluster'], cancel: 'between_fetches' } as const;
  if (driver === 'cassandra') {
    return { ...base, family: 'wide_column', language: { kind: 'cql' }, schema: 'declared', tree: { databases: true, namespaces: 'multiple', groups: ['tables', 'materialized_views', 'types'], server: false }, words: { table: 'table', column: 'column' }, explain: true, tools: ['cluster'] };
  }
  if (driver === 'dynamodb') {
    return { ...base, family: 'wide_column', language: { kind: 'partiql' }, tree: { databases: false, namespaces: 'multiple', groups: ['tables'], server: false }, words: { table: 'table', column: 'field' }, explain: false, tools: ['cluster'] };
  }
  return { ...base, family: 'search', language: { kind: 'search' }, tree: { databases: false, namespaces: 'multiple', groups: ['tables', 'aliases', 'data_streams'], server: false }, words: { table: 'index', column: 'field' }, explain: true, tools: ['cluster'] };
}

const profile = (id: string, name: string, target: ConnectionProfile['target']): ConnectionProfile => ({
  id,
  name,
  folder: null,
  environment: params().get('nosql2') === 'prod' ? 'production' : 'development',
  color: null,
  session: DEFAULT_SESSION,
  template: { kind: 'none' },
  raw_fields: {},
  target,
});

export const NOSQL_DEMO: ConnectionProfile[] = params().has('nosql2')
  ? [
      profile('scylla', 'Ordini per cliente', { driver: 'cassandra', host: '127.0.0.1', port: 59042, database: 'gestionale', user: '', consistency: 'local_quorum', datacenter: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null }),
      profile('dynamo', 'Catalogo DynamoDB', { driver: 'dynamodb', region: 'eu-west-1', endpoint: 'http://127.0.0.1:58000', profile: '', user: 'nicedb' }),
      profile('opensearch', 'Ricerca ordini', { driver: 'elasticsearch', host: '127.0.0.1', port: 59200, database: '', user: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null }),
    ]
  : [];

const col = (name: string, type_name: string, primary_key = false) => ({ name, type_name, nullable: !primary_key, primary_key });
const rel = (name: string, columns: ReturnType<typeof col>[], estimated_rows: number | null, detail: string, indexes: { name: string; columns: string[]; unique: boolean }[] = []) => ({ name, columns, indexes, foreign_keys: [], estimated_rows, detail });
const ns = (name: string, extra: Partial<Schema['namespaces'][number]>): Schema['namespaces'][number] => ({ name, tables: [], views: [], functions: [], sequences: [], types: [], triggers: [], ...extra });

const CLIENTI = ['Bassi Ferramenta', 'Colombo & Figli', 'Edilnord', 'Ferretti Arredi', 'Galli Impianti', 'Lombardi Serramenti', 'Moretti Legnami', 'Rinaldi Termoidraulica'];
const CITTA = ['Bergamo', 'Brescia', 'Como', 'Lecco', 'Monza', 'Cremona'];
const STATI = ['aperto', 'confermato', 'spedito', 'chiuso'];

function cqlRows(): Record<string, unknown>[] {
  return Array.from({ length: 240 }, (_, i) => ({
    cliente: CLIENTI[i % CLIENTI.length],
    anno: 2026 - (i % 3),
    numero: 4800 - i,
    stato: STATI[i % 4],
    totale: (((i * 7919) % 90000) / 100 + 40).toFixed(2),
    righe: [{ articolo: `ART-${100 + (i % 37)}`, quantita: 1 + (i % 6), prezzo: ((i % 50) + 9.5).toFixed(2) }],
    tag: i % 5 === 0 ? ['urgente'] : [],
    creato: `2026-${String(1 + (i % 10)).padStart(2, '0')}-${String(1 + (i % 27)).padStart(2, '0')} 09:${String(i % 60).padStart(2, '0')}:00.000Z`,
  }));
}

function ddbItems(): Record<string, unknown>[] {
  return Array.from({ length: 120 }, (_, i) => ({
    articolo: `ART-${100 + Math.floor(i / 3)}`,
    variante: ['base', 'pro', 'kit'][i % 3],
    categoria: ['viteria', 'utensili', 'idraulica', 'elettrico'][i % 4],
    prezzo: { $numberDecimal: ((i % 40) * 1.5 + 2.9).toFixed(2) },
    giacenza: (i * 37) % 500,
    colori: { $set: ['nero', 'grigio'].slice(0, 1 + (i % 2)) },
    aggiornato: `2026-10-${String(1 + (i % 29)).padStart(2, '0')}`,
  }));
}

function esHits(index: string): Record<string, unknown>[] {
  return Array.from({ length: 60 }, (_, i) => ({
    _id: String(9000 + i),
    _index: index === 'ordini' ? `ordini-${2026 - (i % 2)}` : index,
    numero: `2026/${String(4800 - i).padStart(5, '0')}`,
    cliente: { nome: CLIENTI[i % CLIENTI.length], citta: CITTA[i % CITTA.length] },
    stato: STATI[i % 4],
    totale: Math.round(((i * 7919) % 90000) + 4000) / 100,
    note: i % 3 === 0 ? 'consegna al piano, citofonare prima' : 'ritiro in sede',
    data: `2026-10-${String(1 + (i % 29)).padStart(2, '0')}`,
  }));
}

const CQL_COLS = [col('cliente', 'text', true), col('anno', 'int', true), col('numero', 'int', true), col('creato', 'timestamp'), col('righe', 'list<frozen<riga>>'), col('stato', 'text'), col('tag', 'set<text>'), col('totale', 'decimal')];

export function nosqlSchema(driver: Driver, database: string): Schema {
  if (driver === 'cassandra') {
    const space = ns(database, {
      tables: [
        rel('ordini_per_cliente', CQL_COLS, null, 'PRIMARY KEY ((cliente, anno), numero DESC)', [{ name: 'PRIMARY KEY', columns: ['cliente', 'anno', 'numero'], unique: true }, { name: 'ordini_stato', columns: ['stato'], unique: false }]),
        rel('clienti', [col('id', 'uuid', true), col('citta', 'text'), col('fido', 'decimal'), col('nome', 'text'), col('piva', 'text')], null, 'PRIMARY KEY ((id))', [{ name: 'PRIMARY KEY', columns: ['id'], unique: true }]),
        rel('eventi_magazzino', [col('articolo', 'text', true), col('giorno', 'date', true), col('istante', 'timeuuid', true), col('causale', 'text'), col('quantita', 'int')], null, 'PRIMARY KEY ((articolo, giorno), istante DESC)', [{ name: 'PRIMARY KEY', columns: ['articolo', 'giorno', 'istante'], unique: true }]),
      ],
      materialized_views: [rel('ordini_per_stato', [col('stato', 'text', true), col('cliente', 'text', true), col('anno', 'int', true), col('numero', 'int', true), col('totale', 'decimal')], null, 'ordini_per_cliente · PRIMARY KEY ((stato), cliente, anno, numero)')],
      types: [{ name: 'riga', detail: 'type' }],
    });
    return { databases: ['archivio', 'gestionale', 'magazzino'], current_database: database, namespaces: [space] };
  }
  if (driver === 'dynamodb') {
    const space = ns('eu-west-1', {
      tables: [
        rel('Catalogo', [col('articolo', 'S', true), col('variante', 'S', true), col('aggiornato', 'S'), col('categoria', 'S'), col('colori', 'SS'), col('giacenza', 'N'), col('prezzo', 'N')], 120, 'on demand', [{ name: 'per_categoria', columns: ['categoria', 'prezzo'], unique: false }]),
        rel('Sessioni', [col('utente', 'S', true), col('scade', 'N'), col('token', 'S')], 38, '5 RCU · 5 WCU'),
        rel('Movimenti', [col('articolo', 'S', true), col('istante', 'S', true), col('causale', 'S'), col('quantita', 'N 91% · S 9%')], 5400, 'on demand', [{ name: 'per_causale', columns: ['causale', 'istante'], unique: false }]),
      ],
    });
    return { databases: ['eu-west-1'], current_database: 'eu-west-1', namespaces: [space] };
  }
  const fields = [col('_id', 'keyword', true), col('cliente', 'object'), col('cliente.citta', 'keyword'), col('cliente.nome', 'text'), col('cliente.nome.keyword', 'keyword'), col('data', 'date'), col('note', 'text'), col('numero', 'keyword'), col('stato', 'keyword'), col('totale', 'scaled_float')];
  const space = ns('docker-cluster', {
    tables: [
      rel('clienti', [col('_id', 'keyword', true), col('citta', 'keyword'), col('nome', 'text'), col('piva', 'keyword')], 2400, 'green · 1.2mb'),
      rel('ordini-2025', fields, 182000, 'green · 48.6mb'),
      rel('ordini-2026', fields, 151500, 'yellow · 41.2mb'),
    ],
    aliases: [rel('ordini', fields, 333500, 'ordini-2025, ordini-2026')],
    data_streams: [rel('log-app', [col('_id', 'keyword', true), col('@timestamp', 'date'), col('livello', 'keyword'), col('messaggio', 'text')], 92000, 'green · 3')],
  });
  return { databases: ['docker-cluster'], current_database: 'docker-cluster', namespaces: [space] };
}

const DDL: Record<Driver, (o: ObjectRef) => string> = {
  cassandra: (o) =>
    o.name === 'ordini_per_cliente'
      ? `CREATE TABLE gestionale.ordini_per_cliente (\n    cliente text,\n    anno int,\n    numero int,\n    creato timestamp,\n    righe list<frozen<riga>>,\n    stato text,\n    tag set<text>,\n    totale decimal,\n    PRIMARY KEY ((cliente, anno), numero)\n) WITH CLUSTERING ORDER BY (numero DESC)\n    AND compaction = {'class': 'SizeTieredCompactionStrategy'}\n    AND default_time_to_live = 0;\n\nCREATE INDEX ordini_stato ON gestionale.ordini_per_cliente(stato);`
      : `CREATE TABLE gestionale.${o.name} (\n    id uuid PRIMARY KEY\n);`,
  dynamodb: (o) => JSON.stringify({ CreateTable: { TableName: o.name, KeySchema: [{ AttributeName: 'articolo', KeyType: 'HASH' }, { AttributeName: 'variante', KeyType: 'RANGE' }], AttributeDefinitions: [{ AttributeName: 'articolo', AttributeType: 'S' }, { AttributeName: 'variante', AttributeType: 'S' }], BillingMode: 'PAY_PER_REQUEST' } }, null, 2),
  elasticsearch: (o) => `PUT /${o.name}\n${JSON.stringify({ settings: { index: { number_of_shards: '1', number_of_replicas: '1' } }, mappings: { properties: { numero: { type: 'keyword' }, stato: { type: 'keyword' }, totale: { type: 'scaled_float', scaling_factor: 100 }, note: { type: 'text' } } } }, null, 2)}`,
};

function report(driver: Driver): Report {
  const info = (rows: string[][]) => ({ id: driver === 'cassandra' ? 'cluster' : driver === 'dynamodb' ? 'region' : 'health', group: 'info' as const, columns: ['name', 'value'], rows });
  const grid = (id: string, columns: string[], rows: string[][]) => ({ id, group: 'grid' as const, columns, rows });
  if (driver === 'cassandra') {
    const nodes = [
      ['10.0.4.11', 'datacenter1', 'rack1', '3.0.8', '256', '6f1c…a201'],
      ['10.0.4.12', 'datacenter1', 'rack1', '3.0.8', '256', '93d0…7c55'],
      ['10.0.4.13', 'datacenter1', 'rack2', '3.0.8', '256', 'c7e2…0b19'],
    ];
    const ring = Array.from({ length: 24 }, (_, i) => {
      const t = BigInt(-9223372036854775807n) + BigInt(i) * 768614336404564650n;
      return [String(t), String(t + 768614336404564650n), nodes[i % 3]?.[0] ?? '', 'datacenter1'];
    });
    return {
      sections: [
        info([['product', 'ScyllaDB'], ['version', '6.2.3'], ['consistency', 'LOCAL_QUORUM'], ['cluster_name', 'Magazzino'], ['partitioner', 'org.apache.cassandra.dht.Murmur3Partitioner'], ['cql_version', '3.3.1'], ['nodes', '3']]),
        grid('nodes', ['address', 'datacenter', 'rack', 'version', 'tokens', 'host_id'], nodes),
        grid('ring', ['token', 'next_token', 'address', 'datacenter'], ring),
        grid('keyspaces', ['keyspace', 'replication', 'durable_writes'], [
          ['archivio', "{'class': 'NetworkTopologyStrategy', 'datacenter1': '3'}", 'true'],
          ['gestionale', "{'class': 'NetworkTopologyStrategy', 'datacenter1': '3'}", 'true'],
          ['magazzino', "{'class': 'SimpleStrategy', 'replication_factor': '2'}", 'true'],
        ]),
      ],
    };
  }
  if (driver === 'dynamodb') {
    return {
      sections: [
        info([['product', 'DynamoDB Local'], ['region', 'eu-west-1'], ['endpoint', '127.0.0.1:58000'], ['tables', '3'], ['items', '5558'], ['size_bytes', '1893402']]),
        grid('tables', ['table', 'status', 'items', 'size_bytes', 'capacity', 'key', 'gsi', 'lsi'], [
          ['Catalogo', 'ACTIVE', '120', '48210', 'on demand', 'articolo, variante', '1', '0'],
          ['Movimenti', 'ACTIVE', '5400', '1840511', 'on demand', 'articolo, istante', '1', '0'],
          ['Sessioni', 'ACTIVE', '38', '4681', '5 RCU · 5 WCU', 'utente', '0', '0'],
        ]),
      ],
    };
  }
  return {
    sections: [
      info([['product', 'OpenSearch'], ['version', '2.18.0'], ['cluster_name', 'docker-cluster'], ['status', 'yellow'], ['number_of_nodes', '1'], ['number_of_data_nodes', '1'], ['active_primary_shards', '9'], ['active_shards', '9'], ['relocating_shards', '0'], ['initializing_shards', '0'], ['unassigned_shards', '3'], ['active_shards_percent_as_number', '75.0']]),
      grid('nodes', ['name', 'ip', 'node.role', 'master', 'heap.percent', 'ram.percent', 'cpu', 'load_1m', 'disk.used_percent', 'version'], [['os-01', '10.88.0.7', 'dimr', '*', '41', '87', '6', '0.42', '38.17', '2.18.0']]),
      grid('indices', ['health', 'status', 'index', 'pri', 'rep', 'docs.count', 'store.size'], [
        ['green', 'open', 'clienti', '1', '0', '2400', '1.2mb'],
        ['green', 'open', 'ordini-2025', '1', '0', '182000', '48.6mb'],
        ['yellow', 'open', 'ordini-2026', '1', '1', '151500', '41.2mb'],
      ]),
      grid('shards', ['index', 'shard', 'prirep', 'state', 'docs', 'store', 'node'], [
        ['clienti', '0', 'p', 'STARTED', '2400', '1.2mb', 'os-01'],
        ['ordini-2025', '0', 'p', 'STARTED', '182000', '48.6mb', 'os-01'],
        ['ordini-2026', '0', 'p', 'STARTED', '151500', '41.2mb', 'os-01'],
        ['ordini-2026', '0', 'r', 'UNASSIGNED', '', '', ''],
      ]),
    ],
  };
}

const running = new Map<string, () => void>();

function statements(text: string): [number, number][] {
  const out: [number, number][] = [];
  let i = 0;
  while (i < text.length) {
    while (i < text.length && /[\s;]/.test(text[i] ?? '')) i++;
    if (text.startsWith('--', i)) {
      i = text.indexOf('\n', i) < 0 ? text.length : text.indexOf('\n', i);
      continue;
    }
    if (i >= text.length) break;
    const start = i;
    if (text[i] === '{') {
      let depth = 0;
      for (; i < text.length; i++) {
        if (text[i] === '{') depth++;
        else if (text[i] === '}' && --depth === 0) break;
      }
      out.push([start, Math.min(i + 1, text.length)]);
      i++;
    } else {
      const end = text.indexOf(';', i);
      const stop = end < 0 ? text.length : end;
      out.push([start, start + text.slice(start, stop).trimEnd().length]);
      i = stop + 1;
    }
  }
  return out;
}

function pick(all: [number, number][], req: RunRequest): [number, number][] {
  const s = req.scope;
  if (s.kind === 'script') return all;
  if (s.kind === 'selection') return all.filter(([a, b]) => b > Math.min(s.from, s.to) && a < Math.max(s.from, s.to));
  const hit = all.find(([a, b]) => a <= s.cursor && s.cursor <= b) ?? [...all].reverse().find(([, b]) => b <= s.cursor) ?? all[0];
  return hit ? [hit] : [];
}

async function documents(index: number, list: Record<string, unknown>[], emit: (e: RunEvent) => void, stopped: () => boolean) {
  let sent = 0;
  while (sent < list.length && !stopped()) {
    const n = Math.min(sent === 0 ? 200 : 4000, list.length - sent);
    emit({ type: 'documents', index, documents: list.slice(sent, sent + n).map((d) => JSON.stringify(d)) });
    sent += n;
    await sleep(15);
  }
  return sent;
}

function grid(index: number, columns: string[], rows: (string | null)[][], emit: (e: RunEvent) => void) {
  const numeric = (i: number) => rows.every((r) => r[i] === null || r[i] === undefined || Number.isFinite(Number(r[i])));
  emit({ type: 'columns', index, columns: columns.map((name, i) => ({ name, type_name: numeric(i) ? 'double' : null })) });
  emit({ type: 'rows', index, rows });
  return rows.length;
}

const reads = (driver: Driver, part: string) => {
  const t = part.trim();
  if (driver === 'elasticsearch') {
    const r = parseRequest(t);
    return r ? r.method === 'GET' || r.method === 'HEAD' || /_(search|count|sql|explain|mapping)/.test(r.path) : /^select/i.test(t);
  }
  if (t.startsWith('{')) return /"(Scan|Query|GetItem|BatchGetItem|DescribeTable|ListTables)"/.test(t.slice(0, 30));
  return /^(select|describe|desc|use|consistency)\b/i.test(t);
};

async function answer(driver: Driver, part: string, index: number, emit: (e: RunEvent) => void, stopped: () => boolean): Promise<number> {
  const t = part.trim();
  if (driver === 'cassandra') {
    if (/^consistency/i.test(t)) return documents(index, [{ consistency: (t.split(/\s+/)[1] ?? 'LOCAL_QUORUM').toUpperCase() }], emit, stopped);
    const limit = Number(/limit\s+(\d+)/i.exec(t)?.[1] ?? 1e9);
    const who = /cliente\s*=\s*'([^']+)'/i.exec(t)?.[1];
    return documents(index, cqlRows().filter((r) => !who || r.cliente === who).slice(0, limit), emit, stopped);
  }
  if (driver === 'dynamodb') {
    const items = ddbItems();
    const cat = /categoria\s*=\s*'([^']+)'/i.exec(t)?.[1];
    return documents(index, items.filter((r) => !cat || r.categoria === cat), emit, stopped);
  }
  if (SQL.test(t)) return grid(index, ['stato', 'ordini', 'totale'], STATI.map((s, i) => [s, String(15 + i * 4), String(1240.5 + i * 310)]), emit);
  const r = parseRequest(t);
  if (!r) return 0;
  if (/_cat\/indices/.test(r.path)) return grid(index, ['health', 'status', 'index', 'docs.count', 'store.size'], [['green', 'open', 'clienti', '2400', '1.2mb'], ['green', 'open', 'ordini-2025', '182000', '48.6mb'], ['yellow', 'open', 'ordini-2026', '151500', '41.2mb']], emit);
  if (/_search/.test(r.path) && /"aggs"|"aggregations"/.test(r.body) && /"size"\s*:\s*0/.test(r.body)) {
    return grid(index, ['per_stato', 'per_stato.doc_count', 'incasso'], STATI.map((s, i) => [s, String(83000 - i * 11000), String(4183022.5 - i * 520000)]), emit);
  }
  if (/_search/.test(r.path)) return documents(index, esHits(indexOf(r.path) ?? 'ordini').slice(0, Number(/"size"\s*:\s*(\d+)/.exec(r.body)?.[1] ?? 10)), emit, stopped);
  if (/_count/.test(r.path)) return documents(index, [{ count: 333500, _shards: { total: 2, successful: 2, skipped: 0, failed: 0 } }], emit, stopped);
  return documents(index, [{ acknowledged: true }], emit, stopped);
}

const SQL = /^\s*(select|show|describe|desc)\b/i;

async function run(driver: Driver, req: RunRequest, emit: (e: RunEvent) => void) {
  let cancelled = false;
  running.set(req.run_id, () => (cancelled = true));
  const text = req.text;
  const all = driver === 'elasticsearch' ? consoleCommands(text) : statements(text);
  for (const [index, [from, to]] of pick(all, req).entries()) {
    if (cancelled) break;
    const part = text.slice(from, to);
    emit({ type: 'statement', index, from, to, sql: part, unsortable: null });
    await sleep(40);
    const count = await answer(driver, part, index, emit, () => cancelled);
    const write = !reads(driver, part);
    emit({ type: 'done', index, elapsed_ms: 12 + index, row_count: count, rows_affected: write ? 1 : null, command: part.trim().split(/\s+/)[0]?.replace(/[{"]/g, '').toUpperCase() ?? '', truncated: false, row_limit: null });
  }
  running.delete(req.run_id);
  emit({ type: 'finished', elapsed_ms: 60, cancelled });
}

function planNode(id: number, parent: number | null, depth: number, op: string, kind: Node['kind'], time_ms: number, extra: Partial<Node> = {}): Node {
  return { id, parent, depth, children: [], op, kind, relation: null, alias: null, index: null, condition: null, filter: null, est_rows: null, rows: null, loops: null, rows_removed: null, est_examined: null, startup_cost: null, total_cost: null, time_ms, self_ms: time_ms, width: null, buffers: null, spill: null, parallel: false, details: [], ...extra };
}

function plan(driver: Driver, req: ExplainRequest): PlanReport {
  const nodes: Node[] =
    driver === 'cassandra'
      ? [
          planNode(0, null, 0, 'Execute CQL3 query', 'result', 1.84, { details: [{ key: 'coordinator', value: '10.0.4.11' }] }),
          planNode(1, 0, 1, '10.0.4.11', 'gather', 1.84, { relation: '10.0.4.11' }),
          planNode(2, 1, 2, 'Parsing a statement', 'other', 0.02, { details: [{ key: 'thread', value: 'shard 0' }] }),
          planNode(3, 1, 2, 'Processing a statement', 'other', 0.05, { details: [{ key: 'thread', value: 'shard 0' }] }),
          planNode(4, 1, 2, 'Creating read executor for token -4069959284402364209 with all: {10.0.4.11, 10.0.4.12} targets: {10.0.4.11}', 'other', 0.09),
          planNode(5, 1, 2, 'Executing single-partition query on ordini_per_cliente', 'index_scan', 0.61, { relation: 'ordini_per_cliente' }),
          planNode(6, 1, 2, 'Read 24 live and 0 tombstone cells', 'other', 0.38),
          planNode(7, 1, 2, 'Querying is done', 'other', 0.12),
          planNode(8, 0, 1, '10.0.4.12', 'gather', 0.71, { relation: '10.0.4.12' }),
          planNode(9, 8, 2, 'read_data: querying locally', 'other', 0.21),
          planNode(10, 8, 2, 'Read 24 live and 0 tombstone cells', 'other', 0.5),
        ]
      : [
          planNode(0, null, 0, 'search', 'result', 7, { rows: 30117 }),
          planNode(1, 0, 1, 'shard [os-01][ordini-2026][0]', 'gather', 4.6, { relation: 'ordini-2026' }),
          planNode(2, 1, 2, 'BooleanQuery', 'index_scan', 2.9, { condition: '+stato:aperto +totale:[100.0 TO *]' }),
          planNode(3, 2, 3, 'TermQuery', 'index_scan', 0.8, { condition: 'stato:aperto' }),
          planNode(4, 2, 3, 'IndexOrDocValuesQuery', 'index_scan', 1.7, { condition: 'totale:[10000 TO 9223372036854775807]' }),
          planNode(5, 1, 2, 'SimpleTopScoreDocCollector', 'limit', 0.6, { details: [{ key: 'reason', value: 'search_top_hits' }] }),
          planNode(6, 1, 2, 'GlobalOrdinalsStringTermsAggregator', 'aggregate', 1.1, { condition: 'per_cliente' }),
          planNode(7, 0, 1, 'shard [os-01][ordini-2025][0]', 'gather', 2.1, { relation: 'ordini-2025' }),
          planNode(8, 7, 2, 'BooleanQuery', 'index_scan', 1.5, { condition: '+stato:aperto +totale:[100.0 TO *]' }),
          planNode(9, 7, 2, 'SimpleTopScoreDocCollector', 'limit', 0.6, { details: [{ key: 'reason', value: 'search_top_hits' }] }),
        ];
  for (const n of nodes) if (n.parent !== null) nodes[n.parent]?.children.push(n.id);
  for (const n of nodes) n.self_ms = Math.max(0, (n.time_ms ?? 0) - n.children.reduce((s, c) => s + (nodes[c]?.time_ms ?? 0), 0));
  const text = req.text.trim();
  return {
    plan: { engine: driver === 'cassandra' ? 'cassandra' : 'elasticsearch', analyzed: true, nodes, total_ms: nodes[0]?.time_ms ?? null, planning_ms: null, slowest: driver === 'cassandra' ? 5 : 4, worst_estimate: null, raw: '{}' },
    findings: [],
    execution: { kind: 'measured' },
    unreadable: null,
    sql: text,
    from: 0,
    to: text.length,
    dialect: null,
    elapsed_ms: 19,
    at: Date.now(),
  };
}

export function nosqlMock(base: Api, profileOf: (sessionId: string) => ConnectionProfile | undefined, database: (sessionId: string) => string): Partial<Api> {
  const mine = (sid: string): Driver | null => {
    const d = profileOf(sid.split('@')[0] ?? sid)?.target.driver;
    return d === 'cassandra' || d === 'dynamodb' || d === 'elasticsearch' ? d : null;
  };
  return {
    schemaLoad: async (sid) => {
      const d = mine(sid);
      return d ? (await sleep(60), nosqlSchema(d, database(sid))) : base.schemaLoad(sid);
    },
    objectDdl: async (sid, object) => {
      const d = mine(sid);
      return d ? DDL[d](object) : base.objectDdl(sid, object);
    },
    run: (req, emit) => {
      const d = mine(req.session_id);
      return d ? run(d, req, emit) : base.run(req, emit);
    },
    runCancel: async (id) => {
      const stop = running.get(id);
      if (stop) return stop();
      return base.runCancel(id);
    },
    serverReport: async (sid) => {
      const d = mine(sid);
      return d ? (await sleep(80), report(d)) : base.serverReport(sid);
    },
    explain: async (req) => {
      const d = mine(req.session_id);
      if (!d) return base.explain(req);
      await sleep(200);
      return plan(d, req);
    },
    documentsWrite: async (sid, write: DocumentWrite, confirm) => {
      if (!mine(sid)) return base.documentsWrite(sid, write, confirm);
      const access = await base.writeAccess(sid);
      if (access.guarded && !access.open) throw { code: 'read_only' };
      await sleep(40);
      return { command: 'write', rows_affected: write.kind === 'delete' ? write.ids.length : 1 };
    },
  };
}
