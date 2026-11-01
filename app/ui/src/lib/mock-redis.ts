import type { ConnectionProfile, RunRequest, Schema } from './api';
import type { Capabilities, KeyEntry, KeyInfo_Serialize as KeyInfo, KeyPage_Serialize as KeyPage, KeyRead, KeyScan_Deserialize as KeyScan, KeyValue_Serialize as KeyValue, KeyWrite_Deserialize as KeyWrite, Outcome_Serialize as Outcome, Report, RunEvent_Serialize as RunEvent } from './bindings';
import { readsOnly, splitLines, words } from './redis';
import { DEFAULT_SESSION } from './session';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const REDIS_CAPS: Capabilities = {
  family: 'key_value',
  language: { kind: 'redis' },
  schema: 'inferred',
  results: 'keys',
  tree: { databases: true, namespaces: 'multiple', groups: ['tables'], server: false },
  words: { table: 'key', column: 'field' },
  transactions: false,
  editable: true,
  explain: false,
  er: false,
  ddl: false,
  sort: false,
  params: false,
  tools: ['server'],
  cancel: 'server',
};

export const REDIS_DEMO: ConnectionProfile[] = params().has('redis')
  ? [
      {
        id: 'redis',
        name: 'Cache gestionale',
        folder: null,
        environment: params().get('redis') === 'prod' ? 'production' : 'development',
        color: null,
        session: DEFAULT_SESSION,
        template: { kind: 'none' },
        raw_fields: {},
        target: { driver: 'redis', host: '127.0.0.1', port: 56379, database: 0, user: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null, mode: { kind: 'single' } },
      },
    ]
  : [];

type Value = { kind: 'string'; v: string } | { kind: 'hash'; v: [string, string][] } | { kind: 'list'; v: string[] } | { kind: 'set'; v: string[] } | { kind: 'zset'; v: [string, number][] } | { kind: 'stream'; v: [string, [string, string][]][] };
interface Entry {
  value: Value;
  ttl: number | null;
}

const NOMI = ['Marco', 'Elena', 'Paolo', 'Giulia', 'Luca', 'Sara', 'Davide', 'Chiara'];
const CITTA = ['Bergamo', 'Brescia', 'Seriate', 'Lumezzane', 'Dalmine', 'Treviglio', 'Chiari', 'Rovato'];
const ATTIVITA = ['Edilnord', 'Termoidraulica', 'Officine Meccaniche', 'Serramenti', 'Carpenteria', 'Falegnameria'];
const STATI = ['aperto', 'confermato', 'spedito', 'consegnato'];
const pick = <T>(list: readonly T[], i: number): T => list[i % list.length] as T;

function seed(): Map<string, Entry>[] {
  const db = new Map<string, Entry>();
  const put = (k: string, value: Value, ttl: number | null = null) => db.set(k, { value, ttl });
  put('gestionale:versione', { kind: 'string', v: '2026.10' });
  put('gestionale:manutenzione', { kind: 'string', v: '0' });
  for (let i = 1; i <= 120; i++) {
    const id = ((i * 7919) % 65536).toString(16).padStart(4, '0') + i.toString(16).padStart(4, '0');
    put(`sessione:web:${id}`, { kind: 'hash', v: [['utente', pick(NOMI, i)], ['agente', String(1 + (i % 8))], ['citta', pick(CITTA, i)], ['carrello', String(i % 5)], ['ultima_pagina', `/ordini/${20000 - i}`]] }, (1800 + i * 9) * 1000);
  }
  for (let i = 1; i <= 12; i++) put(`sessione:app:tablet-${String(i).padStart(2, '0')}`, { kind: 'hash', v: [['agente', String(1 + (i % 8))], ['versione', `3.4.${i % 4}`]] });
  for (let i = 1; i <= 300; i++) put(`cache:cliente:${i}`, { kind: 'string', v: JSON.stringify({ id: i, ragione_sociale: `${pick(ATTIVITA, i)} ${i} S.r.l.`, citta: pick(CITTA, i), fido: 5000 + ((i * 25) % 20000), bloccato: i % 17 === 0 }) }, (3600 + i) * 1000);
  for (let i = 1; i <= 60; i++) put(`cache:articolo:ART-${String(i).padStart(5, '0')}:prezzo`, { kind: 'string', v: `${3 + (i % 180)}.${String((i * 37) % 100).padStart(2, '0')}` });
  for (let d = 1; d <= 29; d++) {
    put(`contatore:ordini:2026-10-${String(d).padStart(2, '0')}`, { kind: 'string', v: String(40 + ((d * 13) % 70)) });
    put(`contatore:fatture:2026-10-${String(d).padStart(2, '0')}`, { kind: 'string', v: String(20 + ((d * 7) % 40)) });
  }
  put('contatore:ordini:ultimo', { kind: 'string', v: '20000' });
  put('classifica:clienti:fatturato', { kind: 'zset', v: Array.from({ length: 200 }, (_, i) => [`cliente:${i + 1}`, 1000 + (((i + 1) * 7919) % 250000) + ((i + 1) % 100) / 100] as [string, number]) });
  put('eventi:ordini', { kind: 'stream', v: Array.from({ length: 60 }, (_, i) => [`1761734400000-${i}`, [['ordine', String(19701 + i)], ['cliente', String(1 + ((i * 37) % 2400))], ['stato', pick(STATI, i)], ['totale', `${50 + ((i * 97) % 4000)}.${String(i % 100).padStart(2, '0')}`]]] as [string, [string, string][]]) });
  put('coda:email', { kind: 'list', v: Array.from({ length: 25 }, (_, i) => JSON.stringify({ a: `cliente${i + 1}@esempio.it`, modello: 'conferma-ordine', ordine: 19976 + i })) });
  put('coda:stampa', { kind: 'list', v: ['DDT 2026/1840', 'DDT 2026/1841', 'Fattura 2026/7240'] });
  for (let i = 1; i <= 20; i++) put(`tag:articolo:ART-${String(i).padStart(5, '0')}`, { kind: 'set', v: [i % 2 ? 'ferramenta' : 'idraulica', i % 3 ? 'catalogo' : 'offerta'] });
  put('agenti:online', { kind: 'set', v: ['1', '3', '4', '7'] });
  put('blocco:fatturazione', { kind: 'string', v: 'worker-2' }, 120_000);
  const archive = new Map<string, Entry>();
  for (let i = 1; i <= 12; i++) archive.set(`archivio:ordini:2025-${String(i).padStart(2, '0')}`, { value: { kind: 'string', v: String(900 + i * 11) }, ttl: null });
  return [db, archive, ...Array.from({ length: 14 }, () => new Map<string, Entry>())];
}

const dbs = seed();
const dbOf = (database: string | undefined): Map<string, Entry> => dbs[Number((database ?? 'db0').replace(/^db/, '')) || 0] ?? dbs[0] ?? new Map();

function globRe(glob: string): RegExp {
  let re = '';
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i] ?? '';
    if (c === '\\') re += (glob[++i] ?? '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    else if (c === '*') re += '.*';
    else if (c === '?') re += '.';
    else re += c.replace(/[.+^${}()|\\]/g, '\\$&');
  }
  return new RegExp(`^${re}$`, 's');
}

const lengthOf = (v: Value) => (v.kind === 'string' ? v.v.length : v.v.length);
const sizeOf = (k: string, v: Value) => 48 + k.length + (v.kind === 'string' ? v.v.length : JSON.stringify(v.v).length);
const info = (k: string, e: Entry): KeyInfo => ({ key: k, kind: e.value.kind, ttl_ms: e.ttl, bytes: sizeOf(k, e.value) });

export async function redisScan(database: string | undefined, s: KeyScan): Promise<KeyPage> {
  await sleep(40);
  const db = dbOf(database);
  const re = globRe(s.pattern || '*');
  const all = [...db.keys()];
  const start = Number(s.cursor) || 0;
  const end = Math.min(all.length, start + Math.max(s.count, 10));
  const keys = all.slice(start, end).flatMap((k) => {
    const e = db.get(k);
    return e && re.test(k) && (!s.kind || e.value.kind === s.kind) ? [info(k, e)] : [];
  });
  return { keys, cursor: end >= all.length ? '0' : String(end), total: db.size };
}

export async function redisRead(database: string | undefined, r: KeyRead): Promise<KeyValue> {
  await sleep(30);
  const e = dbOf(database).get(r.key);
  if (!e) return { key: r.key, kind: 'none', ttl_ms: null, length: null, data: { shape: 'missing' }, next: null };
  const v = e.value;
  const start = Number(r.cursor) || 0;
  const page = <T>(list: T[]) => list.slice(start, start + r.count);
  const next = v.kind !== 'string' && start + r.count < v.v.length ? String(start + r.count) : null;
  const entries: KeyEntry[] =
    v.kind === 'hash'
      ? page(v.v).map(([name, value]) => ({ name, value }))
      : v.kind === 'list'
        ? page(v.v).map((value, i) => ({ name: String(start + i), value }))
        : v.kind === 'set'
          ? page(v.v).map((name) => ({ name, value: '' }))
          : v.kind === 'zset'
            ? page([...v.v].sort((a, b) => b[1] - a[1])).map(([name, s]) => ({ name, value: String(s) }))
            : v.kind === 'stream'
              ? page(v.v).map(([name, f]) => ({ name, value: JSON.stringify(Object.fromEntries(f)) }))
              : [];
  return { key: r.key, kind: v.kind, ttl_ms: e.ttl, length: lengthOf(v), data: v.kind === 'string' ? { shape: 'text', text: v.v, binary: false, truncated: false } : { shape: 'entries', entries }, next };
}

const queryError = (message: string) => ({ code: 'driver', detail: { kind: 'query', message, code: 'ERR', detail: null, hint: null, offset: null } });

export async function redisWrite(database: string | undefined, w: KeyWrite): Promise<Outcome> {
  await sleep(60);
  const db = dbOf(database);
  const e = w.op === 'delete' ? undefined : db.get(w.key);
  const of = <K extends Value['kind']>(kind: K): Extract<Value, { kind: K }> => {
    if (!e) {
      const fresh = { value: { kind, v: kind === 'string' ? '' : [] } as Value, ttl: null };
      db.set(w.op === 'delete' ? '' : w.key, fresh);
      return fresh.value as Extract<Value, { kind: K }>;
    }
    if (e.value.kind !== kind) throw queryError('WRONGTYPE Operation against a key holding the wrong kind of value');
    return e.value as Extract<Value, { kind: K }>;
  };
  const out = (command: string, n: number | null = 1): Outcome => ({ command, rows_affected: n });
  switch (w.op) {
    case 'set_string':
      of('string').v = w.value;
      return out('SET', null);
    case 'json_set':
      of('string').v = w.value;
      return out('JSON.SET', null);
    case 'hash_set': {
      const h = of('hash');
      const at = h.v.findIndex(([f]) => f === w.field);
      if (at >= 0) h.v[at] = [w.field, w.value];
      else h.v.push([w.field, w.value]);
      return out('HSET', at >= 0 ? 0 : 1);
    }
    case 'hash_delete': {
      const h = of('hash');
      h.v = h.v.filter(([f]) => !w.fields.includes(f));
      return out('HDEL');
    }
    case 'list_set':
      of('list').v[w.index ?? 0] = w.value;
      return out('LSET', null);
    case 'list_push': {
      const l = of('list');
      if (w.head) l.v.unshift(...[...w.values].reverse());
      else l.v.push(...w.values);
      return out(w.head ? 'LPUSH' : 'RPUSH', l.v.length);
    }
    case 'list_remove': {
      const l = of('list');
      if (l.v[w.index ?? -1] !== w.value) throw queryError('the list changed since it was read: reload it and try again');
      l.v.splice(w.index ?? 0, 1);
      return out('EVAL');
    }
    case 'set_add': {
      const s = of('set');
      s.v = [...new Set([...s.v, ...w.members])];
      return out('SADD', w.members.length);
    }
    case 'set_remove': {
      const s = of('set');
      s.v = s.v.filter((x) => !w.members.includes(x));
      return out('SREM', w.members.length);
    }
    case 'zset_add': {
      const z = of('zset');
      z.v = [...z.v.filter(([x]) => x !== w.member), [w.member, w.score ?? 0]];
      return out('ZADD');
    }
    case 'zset_remove': {
      const z = of('zset');
      z.v = z.v.filter(([x]) => !w.members.includes(x));
      return out('ZREM', w.members.length);
    }
    case 'stream_add':
      of('stream').v.push([`${Date.now()}-0`, w.fields.map((f) => [f.name, f.value])]);
      return out('XADD', null);
    case 'stream_delete': {
      const x = of('stream');
      x.v = x.v.filter(([id]) => !w.ids.includes(id));
      return out('XDEL', w.ids.length);
    }
    case 'expire':
      if (!e) throw queryError(`the key ${w.key} is gone`);
      e.ttl = w.ttl_ms ?? null;
      return out(w.ttl_ms ? 'PEXPIRE' : 'PERSIST');
    case 'rename':
      if (!e) throw queryError('ERR no such key');
      if (db.has(w.to)) throw queryError(`a key named ${w.to} already exists`);
      db.delete(w.key);
      db.set(w.to, e);
      return out('RENAMENX');
    case 'delete': {
      let n = 0;
      for (const k of w.keys) n += db.delete(k) ? 1 : 0;
      return out('UNLINK', n);
    }
    case 'create': {
      if (e) throw queryError(`a key named ${w.key} already exists`);
      const { name, value } = w.first;
      const v: Value =
        w.kind === 'hash' ? { kind: 'hash', v: [[name, value]] } : w.kind === 'list' ? { kind: 'list', v: [value] } : w.kind === 'set' ? { kind: 'set', v: [name] } : w.kind === 'zset' ? { kind: 'zset', v: [[name, Number(value)]] } : w.kind === 'stream' ? { kind: 'stream', v: [[`${Date.now()}-0`, [[name, value]]]] } : { kind: 'string', v: value };
      db.set(w.key, { value: v, ttl: w.ttl_ms ?? null });
      return out(w.kind === 'string' ? 'SET' : 'CREATE');
    }
  }
}

export function redisSchema(database: string | undefined): Schema {
  const db = dbOf(database);
  const groups = new Map<string, Map<string, number>>();
  for (const [k, e] of db) {
    const head = k.split(':')[0] ?? k;
    const g = groups.get(head) ?? new Map<string, number>();
    g.set(e.value.kind, (g.get(e.value.kind) ?? 0) + 1);
    groups.set(head, g);
  }
  const current = database ?? 'db0';
  return {
    databases: Array.from({ length: 16 }, (_, i) => `db${i}`),
    current_database: current,
    namespaces: [
      {
        name: current,
        tables: [...groups].sort((a, b) => a[0].localeCompare(b[0])).map(([name, kinds]) => ({
          name,
          columns: [...kinds].map(([kind, n]) => ({ name: kind, type_name: String(n), nullable: true, primary_key: false })),
          indexes: [],
          foreign_keys: [],
          estimated_rows: [...kinds.values()].reduce((a, b) => a + b, 0),
        })),
        views: [],
        functions: [],
        sequences: [],
        types: [],
        triggers: [],
      },
    ],
  };
}

export async function redisReport(): Promise<Report> {
  await sleep(50);
  const pairs = (id: string, rows: [string, string][]) => ({ id, group: 'info' as const, columns: ['name', 'value'], rows });
  const keys = dbs.reduce((n, d) => n + d.size, 0);
  return {
    sections: [
      pairs('server', [['redis_version', '7.4.12'], ['redis_mode', 'standalone'], ['os', 'Linux 6.12 x86_64'], ['tcp_port', '6379'], ['uptime_in_seconds', '273840'], ['uptime_in_days', '3'], ['executable', '/data/redis-server'], ['config_file', '']]),
      pairs('clients', [['connected_clients', '4'], ['blocked_clients', '0'], ['maxclients', '10000'], ['client_recent_max_input_buffer', '20480']]),
      pairs('memory', [['used_memory', '1802336'], ['used_memory_human', '1,72M'], ['used_memory_peak_human', '2,10M'], ['maxmemory', '0'], ['maxmemory_human', '0B'], ['maxmemory_policy', 'noeviction'], ['mem_fragmentation_ratio', '1.42']]),
      pairs('persistence', [['loading', '0'], ['rdb_changes_since_last_save', '12'], ['rdb_last_bgsave_status', 'ok'], ['aof_enabled', '0']]),
      pairs('stats', [['total_connections_received', '312'], ['total_commands_processed', '48210'], ['instantaneous_ops_per_sec', '18'], ['keyspace_hits', '9820'], ['keyspace_misses', '611'], ['expired_keys', '1204'], ['evicted_keys', '0']]),
      pairs('replication', [['role', 'master'], ['connected_slaves', '0']]),
      pairs('cpu', [['used_cpu_sys', '41.20'], ['used_cpu_user', '63.05']]),
      pairs('keyspace', dbs.flatMap((d, i) => (d.size ? [[`db${i}`, `keys=${d.size},expires=${[...d.values()].filter((e) => e.ttl !== null).length},avg_ttl=0`] as [string, string]] : []))),
      { id: 'commandstats', group: 'info', columns: ['command', 'calls', 'usec', 'usec_per_call', 'failed_calls'], rows: [['HGETALL', '18204', '40120', '2.20', '0'], ['GET', '15011', '19870', '1.32', '0'], ['SET', '7420', '12011', '1.62', '0'], ['SCAN', '311', '9120', '29.32', '0'], ['XADD', '300', '1200', '4.00', '0']] },
      { id: 'clients_list', group: 'clients', columns: ['id', 'addr', 'name', 'user', 'db', 'age', 'idle', 'cmd', 'flags'], rows: [['7', '127.0.0.1:52144', 'NiceDB', 'default', '0', '312', '0', 'client|list', 'N'], ['12', '10.0.4.18:40112', 'gestionale-web', 'default', '0', '86400', '1', 'hgetall', 'N'], ['13', '10.0.4.18:40118', 'gestionale-web', 'default', '0', '86400', '3', 'get', 'N'], ['21', '10.0.4.22:51002', 'fatturazione', 'default', '0', '7200', '42', 'xreadgroup', 'b']] },
      { id: 'slowlog', group: 'slowlog', columns: ['id', 'time', 'usec', 'command', 'client', 'name'], rows: [['14', '1761731940', '18450', 'KEYS cache:*', '10.0.4.18:40112', 'gestionale-web'], ['13', '1761728100', '12210', 'SCAN 0 MATCH sessione:* COUNT 10000', '127.0.0.1:52144', 'NiceDB'], ['12', '1761720011', '10480', 'DEL cache:listino:html', '10.0.4.22:51002', 'fatturazione']] },
      { id: 'memory_stats', group: 'memory', columns: ['name', 'value'], rows: [['peak.allocated', '2203136'], ['total.allocated', '1802336'], ['startup.allocated', '946400'], ['keys.count', String(keys)], ['keys.bytes-per-key', '231'], ['dataset.bytes', '640120'], ['dataset.percentage', '74.9'], ['fragmentation', '1.42']] },
      { id: 'memory_doctor', group: 'memory', columns: ['text'], rows: [['Hi Sam, I can\'t find any memory issue in your instance. I can only account for what occurs on this base.']] },
    ],
  };
}

function fail(index: number, message: string): RunEvent {
  return { type: 'failed', index, elapsed_ms: 1, error: { kind: 'query', message, code: 'ERR', detail: null, hint: null, offset: null }, position: null, at: null };
}

function reply(database: string | undefined, args: string[]): { columns: string[]; rows: (string | null)[][] } | string {
  const db = dbOf(database);
  const [name = '', key = ''] = [args[0]?.toUpperCase(), args[1]];
  const e = db.get(key);
  const one = (v: string | null) => ({ columns: ['value'], rows: [[v]] });
  const list = (vs: string[]) => ({ columns: ['#', 'value'], rows: vs.map((v, i) => [String(i + 1), v]) });
  switch (name) {
    case 'PING':
      return one('PONG');
    case 'DBSIZE':
      return one(String(db.size));
    case 'GET':
      return one(e?.value.kind === 'string' ? e.value.v : null);
    case 'TYPE':
      return one(e?.value.kind ?? 'none');
    case 'TTL':
      return one(String(e ? (e.ttl === null ? -1 : Math.round(e.ttl / 1000)) : -2));
    case 'EXISTS':
      return one(String(args.slice(1).filter((k) => db.has(k)).length));
    case 'HGETALL':
      return { columns: ['field', 'value'], rows: e?.value.kind === 'hash' ? e.value.v : [] };
    case 'LRANGE':
      return list(e?.value.kind === 'list' ? e.value.v : []);
    case 'SMEMBERS':
      return list(e?.value.kind === 'set' ? e.value.v : []);
    case 'ZRANGE':
    case 'ZREVRANGE':
      return { columns: ['#', '1', '2'], rows: e?.value.kind === 'zset' ? [...e.value.v].sort((a, b) => b[1] - a[1]).slice(0, 100).map(([m, s], i) => [String(i + 1), m, String(s)]) : [] };
    case 'KEYS':
    case 'SCAN': {
      const at = args.findIndex((a) => a.toUpperCase() === 'MATCH');
      const re = globRe(name === 'KEYS' ? key : at > 0 ? (args[at + 1] ?? '*') : '*');
      const found = [...db.keys()].filter((k) => re.test(k)).slice(0, name === 'KEYS' ? undefined : 100);
      return name === 'KEYS' ? list(found) : { columns: ['#', 'value'], rows: [['1', '0'], ['2', JSON.stringify(found)]] };
    }
    case 'SET':
      db.set(key, { value: { kind: 'string', v: args[2] ?? '' }, ttl: null });
      return one('OK');
    case 'DEL':
    case 'UNLINK':
      return one(String(args.slice(1).filter((k) => db.delete(k)).length));
    case 'INCR': {
      const n = (Number(e?.value.kind === 'string' ? e.value.v : 0) || 0) + 1;
      db.set(key, { value: { kind: 'string', v: String(n) }, ttl: e?.ttl ?? null });
      return one(String(n));
    }
    case 'INFO':
      return one('# Server\r\nredis_version:7.4.12\r\n');
    case 'SELECT':
      return 'SELECT is not sent from the editor';
    default:
      return `ERR unknown command '${args[0] ?? ''}' in the demo`;
  }
}

export async function redisRun(database: string | undefined, req: RunRequest, emit: (e: RunEvent) => void) {
  const started = performance.now();
  const text = req.text;
  const all = splitLines(text);
  const s = req.scope;
  const chosen = s.kind === 'script' ? all : s.kind === 'selection' ? all.filter(([a, b]) => b > Math.min(s.from, s.to) && a < Math.max(s.from, s.to)) : [all.find(([a, b]) => a <= s.cursor && s.cursor <= b) ?? [...all].reverse().find(([, b]) => b <= s.cursor) ?? all[0]].filter((r): r is [number, number] => !!r);
  for (const [index, [from, to]] of chosen.entries()) {
    const part = text.slice(from, to);
    emit({ type: 'statement', index, from, to, sql: part, unsortable: null });
    await sleep(15);
    const out = reply(database, words(part));
    if (typeof out === 'string') {
      emit(fail(index, out));
      if (req.continue_on_error) continue;
      break;
    }
    emit({ type: 'columns', index, columns: out.columns.map((name) => ({ name, type_name: null })) });
    emit({ type: 'rows', index, rows: out.rows });
    const n = Number(out.rows[0]?.[0]);
    emit({ type: 'done', index, elapsed_ms: 2, row_count: out.rows.length, rows_affected: readsOnly(part) || !Number.isFinite(n) ? null : n, command: words(part)[0]?.toUpperCase() ?? '', truncated: false, row_limit: null });
  }
  emit({ type: 'finished', elapsed_ms: Math.max(4, Math.round(performance.now() - started)), cancelled: false });
}
