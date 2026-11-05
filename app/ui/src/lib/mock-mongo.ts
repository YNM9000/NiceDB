import type { ConnectionProfile, RunRequest, Schema } from './api';
import type { Capabilities, DocumentWrite, ObjectRef, Outcome_Serialize as Outcome, RunEvent_Serialize as RunEvent } from './bindings';
import { relationalCaps } from './family';
import { clickhouseCaps } from './mock-clickhouse';
import { oracleCaps } from './mock-oracle';
import { enterpriseCaps } from './mock-enterprise-engines';
import { checkJson, splitCommands, uncomment } from './mql';
import { DEFAULT_SESSION } from './session';
import { REDIS_CAPS } from './mock-redis';
import { nosqlCaps } from './mock-nosql-engines';

type Driver = ConnectionProfile['target']['driver'];

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function capsOf(driver: Driver): Capabilities {
  switch (driver) {
    case 'redis':
      return REDIS_CAPS;
    case 'mongodb':
      return {
        family: 'document',
        language: { kind: 'mql' },
        schema: 'inferred',
        results: 'documents',
        tree: { databases: true, namespaces: 'multiple', groups: ['tables', 'views'], server: true },
        words: { table: 'collection', column: 'field' },
        transactions: false,
        editable: true,
        explain: true,
        er: true,
        ddl: true,
        sort: false,
        params: false,
        tools: ['activity', 'space', 'users'],
        cancel: 'server',
      };
    case 'sqlite':
    case 'libsql':
    case 'duckdb': {
      const c = relationalCaps(driver === 'duckdb' ? 'duckdb' : 'sqlite');
      return {
        ...c,
        tree: { databases: false, namespaces: 'multiple', groups: driver === 'duckdb' ? ['tables', 'views', 'functions', 'sequences', 'types'] : ['tables', 'views', 'triggers'], server: false },
        tools: c.tools.filter((t) => t !== 'activity' && t !== 'users'),
      };
    }
    case 'mysql': {
      const c = relationalCaps('mysql');
      return { ...c, tree: { ...c.tree, namespaces: 'multiple', groups: ['tables', 'views', 'functions', 'sequences', 'triggers'] } };
    }
    case 'sqlserver':
      return relationalCaps('mssql');
    case 'clickhouse':
      return clickhouseCaps();
    case 'oracle':
      return oracleCaps();
    case 'firebird':
    case 'db2':
    case 'hana':
    case 'odbc':
      return enterpriseCaps(driver);
    case 'cassandra':
    case 'dynamodb':
    case 'elasticsearch':
      return nosqlCaps(driver);
    default:
      return relationalCaps('postgres');
  }
}

export const MONGO_DEMO: ConnectionProfile[] = params().has('mongo')
  ? [
      {
        id: 'mongo',
        name: 'Gestionale documenti',
        folder: null,
        environment: params().get('mongo') === 'prod' ? 'production' : 'development',
        color: null,
        session: DEFAULT_SESSION,
        template: { kind: 'none' },
        raw_fields: {},
        target: { driver: 'mongodb', host: '127.0.0.1', port: 57017, srv: false, database: 'gestionale', user: 'nicedb', auth_source: 'admin', options: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null },
      },
    ]
  : [];

const CITIES: [string, string][] = [['Bergamo', 'BG'], ['Brescia', 'BS'], ['Seriate', 'BG'], ['Lumezzane', 'BS'], ['Dalmine', 'BG'], ['Treviglio', 'BG']];
const NAMES = ['Edilnord', 'Termoidraulica', 'Officine Meccaniche', 'Serramenti', 'Carpenteria', 'Falegnameria'];
const FAMILIES = ['Bassi', 'Valtrompia', 'Gatti', 'Pedrali', 'Locatelli', 'Bonomi'];
const STATES = ['confermato', 'spedito', 'chiuso', 'chiuso', 'annullato', 'bozza'];
const pick = <T>(list: readonly T[], i: number): T => list[i % list.length] as T;

const store: Record<string, Record<string, unknown>[]> = {};

function cliente(i: number): Record<string, unknown> {
  const [citta, provincia] = pick(CITIES, i);
  return {
    _id: i,
    ragione_sociale: `${pick(NAMES, i)} ${pick(FAMILIES, Math.floor(i / 6))} S.r.l.`,
    partita_iva: String(10000000000 + i * 7919),
    indirizzo: { via: `Via Colombo ${1 + (i % 90)}`, citta, provincia, cap: String(24000 + (i % 900)) },
    agente: { _id: 1 + (i % 8), zona: citta },
    fido: { $numberDecimal: String((5 + (i % 20)) * 1000) },
    creato_il: { $date: new Date(Date.UTC(2026, 9, 1) - i * 86400000).toISOString() },
    tag: i % 3 === 0 ? ['edilizia', 'nord'] : ['industria'],
    ...(i % 7 === 0 ? { pec: `amministrazione${i}@pec.example.it` } : {}),
    ...(i % 13 === 0 ? { note: null } : {}),
  };
}

function ordine(i: number): Record<string, unknown> {
  const righe = Array.from({ length: 1 + (i % 4) }, (_, r) => ({ articolo: `ART-${String(1 + ((i * 7 + r * 13) % 4000)).padStart(5, '0')}`, quantita: 1 + ((i + r) % 24), prezzo: Math.round((0.15 + ((i + r) % 400) * 0.37) * 100) / 100, sconto: pick([0, 0, 5, 10], i + r) }));
  const totale = righe.reduce((s, r) => s + r.quantita * r.prezzo * (1 - r.sconto / 100), 0);
  const c = 1 + ((i * 31) % 240);
  return {
    _id: { $oid: (0x6ae360d08712d97c50444b7en + BigInt(i)).toString(16).padStart(24, '0') },
    numero: `2026/${String(i).padStart(6, '0')}`,
    data: { $date: new Date(Date.UTC(2024, 0, 1) + (i % 1000) * 86400000).toISOString() },
    cliente: { _id: c, ragione_sociale: cliente(c).ragione_sociale },
    stato: pick(STATES, i),
    righe,
    totale: { $numberDecimal: totale.toFixed(2) },
    ...(i % 9 === 0 ? { spedizione: { corriere: pick(['BRT', 'GLS', 'SDA'], i), tracking: `TR${i}`, consegnato: i % 2 === 0 } } : {}),
  };
}

function docs(collection: string): Record<string, unknown>[] {
  const make = collection === 'clienti' ? cliente : collection === 'ordini' || collection === 'ordini_aperti' ? ordine : null;
  if (!make) return [];
  store[collection] ??= Array.from({ length: collection === 'clienti' ? 240 : 600 }, (_, i) => make(i + 1));
  return store[collection] ?? [];
}

const col = (name: string, type_name: string, nullable = false) => ({ name, type_name, nullable, primary_key: name === '_id' });
const idx = (name: string, columns: string[], unique = false) => ({ name, columns, unique });

export function mongoSchema(database: string): Schema {
  const base = { databases: ['admin', 'archivio', 'gestionale'], current_database: database, functions: [], sequences: [], types: [], triggers: [] };
  if (database !== 'gestionale') return { databases: base.databases, current_database: database, namespaces: [{ name: database, tables: database === 'archivio' ? [{ name: 'storico', columns: [col('_id', 'objectId'), col('anno', 'int'), col('ordini', 'int'), col('fatturato', 'decimal')], indexes: [idx('_id_', ['_id'], true)], foreign_keys: [], estimated_rows: 2 }] : [], views: [], functions: [], sequences: [], types: [], triggers: [] }] };
  const clienti = [col('_id', 'int'), col('ragione_sociale', 'string'), col('partita_iva', 'string'), col('indirizzo', 'document'), col('indirizzo.via', 'string'), col('indirizzo.citta', 'string'), col('indirizzo.provincia', 'string'), col('indirizzo.cap', 'string'), col('agente', 'document'), col('agente._id', 'int'), col('agente.zona', 'string'), col('fido', 'decimal 98% · string 2%'), col('creato_il', 'date'), col('tag', 'array'), col('pec', 'string', true), col('note', 'null', true)];
  const ordini = [col('_id', 'objectId'), col('numero', 'string'), col('data', 'date'), col('cliente', 'document'), col('cliente._id', 'int'), col('cliente.ragione_sociale', 'string'), col('stato', 'string'), col('righe', 'array'), col('righe.articolo', 'string'), col('righe.quantita', 'int'), col('righe.prezzo', 'double 96% · int 4%'), col('righe.sconto', 'int'), col('totale', 'decimal'), col('spedizione', 'document', true), col('spedizione.corriere', 'string', true), col('spedizione.tracking', 'string', true), col('spedizione.consegnato', 'bool', true)];
  const articoli = [col('_id', 'string'), col('descrizione', 'string'), col('um', 'string'), col('prezzo_listino', 'double 99% · int 1%'), col('aliquota_iva', 'int'), col('giacenza', 'int')];
  return {
    databases: base.databases,
    current_database: database,
    namespaces: [
      {
        name: database,
        tables: [
          { name: 'agenti', columns: [col('_id', 'int'), col('nome', 'string'), col('cognome', 'string'), col('zona', 'string'), col('provvigione', 'decimal')], indexes: [idx('_id_', ['_id'], true)], foreign_keys: [], estimated_rows: 8 },
          { name: 'articoli', columns: articoli, indexes: [idx('_id_', ['_id'], true)], foreign_keys: [], estimated_rows: 4000 },
          { name: 'clienti', columns: clienti, indexes: [idx('_id_', ['_id'], true), idx('partita_iva_1', ['partita_iva'], true), idx('indirizzo.provincia_1_indirizzo.citta_1', ['indirizzo.provincia', 'indirizzo.citta'])], foreign_keys: [], estimated_rows: 2400 },
          { name: 'ordini', columns: ordini, indexes: [idx('_id_', ['_id'], true), idx('numero_1', ['numero'], true), idx('cliente._id_1_data_-1', ['cliente._id', 'data']), idx('stato_1', ['stato'])], foreign_keys: [], estimated_rows: 20000 },
        ],
        views: [{ name: 'ordini_aperti', columns: ordini.filter((c) => ['_id', 'numero', 'data', 'cliente', 'cliente.ragione_sociale', 'totale'].includes(c.name)), indexes: [], foreign_keys: [], estimated_rows: null }],
        ...{ functions: [], sequences: [], types: [], triggers: [] },
      },
    ],
  };
}

const running = new Map<string, () => void>();

function fail(index: number, message: string, at: number | null, text: string): RunEvent {
  const before = at === null ? '' : text.slice(0, at);
  return {
    type: 'failed',
    index,
    elapsed_ms: 1,
    error: { kind: 'query', message, code: null, detail: null, hint: null, offset: null },
    position: at === null ? null : { line: before.split('\n').length, column: at - before.lastIndexOf('\n') },
    at,
  };
}

function matches(doc: Record<string, unknown>, filter: Record<string, unknown>): boolean {
  return Object.entries(filter).every(([k, v]) => {
    const got = k.split('.').reduce<unknown>((o, p) => (o && typeof o === 'object' ? (o as Record<string, unknown>)[p] : undefined), doc);
    if (v && typeof v === 'object' && '$in' in v) return ((v as { $in: unknown[] }).$in ?? []).includes(got);
    return got === v;
  });
}

export async function mongoRun(req: RunRequest, emit: (e: RunEvent) => void) {
  let cancelled = false;
  running.set(req.run_id, () => (cancelled = true));
  const started = performance.now();
  const text = req.text;
  const all = splitCommands(uncomment(text));
  const s = req.scope;
  const chosen = s.kind === 'script' ? all : s.kind === 'selection' ? all.filter(([a, b]) => b > Math.min(s.from, s.to) && a < Math.max(s.from, s.to)) : [all.find(([a, b]) => a <= s.cursor && s.cursor <= b) ?? [...all].reverse().find(([, b]) => b <= s.cursor) ?? all[0]].filter((r): r is [number, number] => !!r);
  for (const [index, [from, to]] of chosen.entries()) {
    if (cancelled) break;
    const part = text.slice(from, to);
    emit({ type: 'statement', index, from, to, sql: part, unsortable: null });
    const bad = checkJson(uncomment(part));
    if (bad) {
      emit(fail(index, 'not valid JSON', from + bad.at, text));
      if (req.continue_on_error) continue;
      break;
    }
    const cmd = JSON.parse(uncomment(part)) as Record<string, unknown>;
    const name = Object.keys(cmd)[0] ?? '';
    const coll = String(cmd[name] ?? '');
    if (name === 'find' || name === 'aggregate') {
      const filter = (name === 'find' ? cmd.filter : (cmd.pipeline as Record<string, unknown>[] | undefined)?.find((st) => '$match' in st)?.$match) as Record<string, unknown> | undefined;
      const limit = Number(cmd.limit ?? 1000);
      const out = docs(coll).filter((d) => matches(d, filter ?? {})).slice(0, limit);
      if (name === 'aggregate' && (cmd.pipeline as Record<string, unknown>[] | undefined)?.some((st) => '$group' in st)) {
        const groups = new Map<string, number>();
        for (const d of out) groups.set(String(d.stato), (groups.get(String(d.stato)) ?? 0) + 1);
        emit({ type: 'documents', index, documents: [...groups].map(([k, n]) => JSON.stringify({ _id: k, ordini: n })) });
        emit({ type: 'done', index, elapsed_ms: 3, row_count: groups.size, rows_affected: null, command: name, truncated: false, row_limit: null });
        continue;
      }
      let sent = 0;
      while (sent < out.length && !cancelled) {
        const n = Math.min(sent === 0 ? 200 : 4000, out.length - sent);
        emit({ type: 'documents', index, documents: out.slice(sent, sent + n).map((d) => JSON.stringify(d)) });
        sent += n;
        await sleep(20);
      }
      emit({ type: 'done', index, elapsed_ms: 18, row_count: sent, rows_affected: null, command: name, truncated: false, row_limit: null });
      continue;
    }
    if (!docs(coll).length && !['ping', 'buildInfo', 'dbStats'].includes(name)) {
      emit(fail(index, `ns does not exist: gestionale.${coll}`, null, text));
      break;
    }
    const n = name === 'count' ? docs(coll).length : 1;
    const reply = name === 'count' ? { n, ok: 1 } : ['insert', 'update', 'delete'].includes(name) ? { n, ok: 1 } : { ok: 1 };
    emit({ type: 'documents', index, documents: [JSON.stringify(reply)] });
    emit({ type: 'done', index, elapsed_ms: 2, row_count: 1, rows_affected: ['insert', 'update', 'delete'].includes(name) ? n : null, command: name, truncated: false, row_limit: null });
  }
  running.delete(req.run_id);
  emit({ type: 'finished', elapsed_ms: Math.max(18, Math.round(performance.now() - started)), cancelled });
}

export function mongoCancel(runId: string): boolean {
  const stop = running.get(runId);
  stop?.();
  return !!stop;
}

export async function mongoWrite(write: DocumentWrite): Promise<Outcome> {
  await sleep(80);
  const list = docs(write.collection);
  const key = (v: unknown) => JSON.stringify(v);
  if (write.kind === 'insert') {
    for (const d of write.documents) list.push(JSON.parse(d) as Record<string, unknown>);
    return { command: 'insert', rows_affected: write.documents.length };
  }
  if (write.kind === 'replace') {
    const at = list.findIndex((d) => key(d._id) === key(JSON.parse(write.id)));
    if (at < 0) throw { code: 'driver', detail: { kind: 'query', message: 'the document is gone', code: null, detail: null, hint: null, offset: null } };
    list[at] = { ...(JSON.parse(write.document) as Record<string, unknown>), _id: list[at]?._id };
    return { command: 'update', rows_affected: 1 };
  }
  const gone = new Set(write.ids);
  const before = list.length;
  store[write.collection] = list.filter((d) => !gone.has(key(d._id)));
  return { command: 'delete', rows_affected: before - (store[write.collection]?.length ?? 0) };
}

export function mongoDdl(o: ObjectRef): string {
  if (o.kind === 'view') return JSON.stringify({ create: o.name, viewOn: 'ordini', pipeline: [{ $match: { stato: { $in: ['confermato', 'spedito'] } } }] }, null, 2);
  const indexes = mongoSchema('gestionale').namespaces[0]?.tables.find((t) => t.name === (o.table ?? o.name))?.indexes.filter((i) => i.name !== '_id_') ?? [];
  const create = JSON.stringify({ create: o.name }, null, 2);
  const script = JSON.stringify({ createIndexes: o.table ?? o.name, indexes: indexes.filter((i) => o.kind !== 'index' || i.name === o.name).map((i) => ({ key: Object.fromEntries(i.columns.map((c) => [c, 1])), name: i.name, ...(i.unique ? { unique: true } : {}) })) }, null, 2);
  return o.kind === 'index' ? script : indexes.length ? `${create}\n\n${script}` : create;
}
