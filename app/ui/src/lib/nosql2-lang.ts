import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { sql, SQLDialect, type SQLNamespace } from '@codemirror/lang-sql';
import { StreamLanguage, type StringStream } from '@codemirror/language';
import { linter, type Diagnostic } from '@codemirror/lint';
import type { Extension } from '@codemirror/state';
import type { Schema } from './api';
import { checkJson } from './mql';
import { m } from '../paraglide/messages';
import { quoteName } from './sqlname';

const CQL_KEYWORDS =
  'select from where and or in contains key limit per partition allow filtering order by asc desc distinct json as cast token writetime ttl maxwritetime insert into values if not exists using timestamp update set delete begin batch apply unlogged counter create alter drop truncate keyspace keyspaces table tables column columns type types index custom materialized view primary with clustering compact storage replication durable_writes function functions aggregate returns called on null input language role roles user users superuser login password grant revoke permission permissions all to of use describe desc consistency tracing list add rename is group';
const CQL_TYPES = 'ascii bigint blob boolean counter date decimal double duration float inet int smallint text time timestamp timeuuid tinyint uuid varchar varint list set map tuple frozen vector';
const CQL_FUNCTIONS = 'now uuid toJson fromJson toDate toTimestamp toUnixTimestamp currentTimestamp currentDate currentTime currentTimeUUID minTimeuuid maxTimeuuid count min max sum avg dateOf unixTimestampOf blobAsText textAsBlob';

export const CqlDialect = SQLDialect.define({
  keywords: CQL_KEYWORDS,
  types: CQL_TYPES,
  builtin: CQL_FUNCTIONS,
  doubleDollarQuotedStrings: true,
  identifierQuotes: '"',
  slashComments: true,
});

const PARTIQL_KEYWORDS = 'select from where and or not in between is missing null insert into value update set remove delete exists begins_with contains size attribute_type existing returning modified new old all';

export const PartiqlDialect = SQLDialect.define({
  keywords: PARTIQL_KEYWORDS,
  builtin: 'begins_with contains size attribute_type exists',
  identifierQuotes: '"',
});

const named = (d: 'cql' | 'partiql', label: string, type: string): Completion => ({ label, type, apply: quoteName(d, label) });

function sqlSchema(schema: Schema | null): { schema: SQLNamespace; defaultSchema?: string } {
  const out: Record<string, SQLNamespace> = {};
  for (const db of schema?.databases ?? []) out[db] = {};
  for (const ns of schema?.namespaces ?? []) {
    const rels: Record<string, SQLNamespace> = {};
    for (const r of [...ns.tables, ...ns.views, ...(ns.materialized_views ?? [])]) rels[r.name] = { self: named('cql', r.name, 'table'), children: r.columns.map((c) => named('cql', c.name, 'property')) };
    out[ns.name] = { self: named('cql', ns.name, 'namespace'), children: rels };
  }
  const current = schema?.namespaces[0]?.name;
  return { schema: out, ...(current ? { defaultSchema: current } : {}) };
}

export function cqlLanguage(schema: Schema | null, completes = true): Extension {
  return sql({ dialect: CqlDialect, ...(completes ? sqlSchema(schema) : {}), upperCaseKeywords: true });
}

export function partiqlLanguage(schema: Schema | null, completes = true): Extension {
  const tables: Record<string, SQLNamespace> = {};
  for (const ns of schema?.namespaces ?? []) for (const r of ns.tables) tables[r.name] = { self: named('partiql', r.name, 'table'), children: r.columns.map((c) => named('partiql', c.name, 'property')) };
  return sql({ dialect: PartiqlDialect, ...(completes ? { schema: tables } : {}), upperCaseKeywords: true });
}

const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'HEAD', 'PATCH'];
const SQL_START = /^\s*(select|show|describe|desc|explain)\b/i;

export function consoleCommands(text: string): [number, number][] {
  const out: [number, number][] = [];
  let open: { start: number; sql: boolean } | null = null;
  let end = 0;
  let depth = 0;
  let at = 0;
  const close = (start: number, stop: number) => {
    let body = text.slice(start, stop).trimEnd();
    if (body.endsWith(';')) body = body.slice(0, -1).trimEnd();
    if (body) out.push([start, start + body.length]);
  };
  for (const line of text.split(/(?<=\n)/)) {
    const start = at;
    at += line.length;
    const t = line.trim();
    const lead = start + (line.length - line.trimStart().length);
    const comment = t.startsWith('#') || t.startsWith('//');
    if (depth <= 0) {
      const method = METHODS.includes((t.split(/\s+/)[0] ?? '').toUpperCase()) && t.length > 0;
      if (method || SQL_START.test(t)) {
        if (open) close(open.start, end);
        open = { start: lead, sql: !method };
        depth = 0;
      } else if (!open && (!t || comment)) continue;
    }
    if (!open) continue;
    if (t && !(depth <= 0 && comment)) {
      end = start + line.trimEnd().length;
      if (!open.sql) depth += braceDepth(line);
    }
    if (open.sql && t.endsWith(';')) {
      close(open.start, end);
      open = null;
    }
  }
  if (open) close(open.start, Math.max(end, open.start));
  return out;
}

function braceDepth(line: string): number {
  let d = 0;
  let inString = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (inString) {
      if (c === '\\') i++;
      else if (c === '"') inString = false;
    } else if (c === '"') inString = true;
    else if (c === '{' || c === '[') d++;
    else if (c === '}' || c === ']') d--;
  }
  return d;
}

export function parseRequest(command: string): { method: string; path: string; body: string } | null {
  const nl = command.indexOf('\n');
  const head = (nl < 0 ? command : command.slice(0, nl)).trim();
  const [method, ...rest] = head.split(/\s+/);
  if (!method || !METHODS.includes(method.toUpperCase())) return null;
  return { method: method.toUpperCase(), path: rest.join(' ') || '/', body: nl < 0 ? '' : command.slice(nl + 1).trim() };
}

export function indexOf(path: string): string | null {
  const first = path.split('?')[0]?.split('/').find((s) => s) ?? '';
  return first && !first.startsWith('_') ? decodeURIComponent(first) : null;
}

const ndjson = (path: string) => /_(bulk|msearch)(\?|$)/.test(path.split('?')[0] ?? '');

export function formatConsole(text: string): string {
  let out = '';
  let last = 0;
  for (const [from, to] of consoleCommands(text)) {
    out += text.slice(last, from);
    const part = text.slice(from, to);
    const r = parseRequest(part);
    if (!r || !r.body || ndjson(r.path) || /(^|\n)\s*(#|\/\/)/.test(r.body)) out += part;
    else {
      try {
        out += `${r.method} ${r.path}\n${JSON.stringify(JSON.parse(r.body), null, 2)}`;
      } catch {
        out += part;
      }
    }
    last = to;
  }
  return out + text.slice(last);
}

interface ConsoleState {
  head: boolean;
  sql: boolean;
}

const consoleStream = StreamLanguage.define<ConsoleState>({
  name: 'search-console',
  startState: () => ({ head: false, sql: false }),
  token(stream: StringStream, state: ConsoleState): string | null {
    if (stream.sol()) {
      state.head = false;
      if (stream.match(/^\s*(#|\/\/).*/)) return 'lineComment';
      if (stream.match(/^(GET|POST|PUT|DELETE|HEAD|PATCH)\b/i)) {
        state.head = true;
        state.sql = false;
        return 'keyword';
      }
      if (stream.match(SQL_START, false)) state.sql = true;
    }
    if (stream.eatSpace()) return null;
    if (state.head) {
      stream.skipToEnd();
      return 'typeName';
    }
    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) return stream.match(/^\s*:/, false) ? 'propertyName' : 'string';
    if (stream.match(/^'(?:[^'\\]|\\.)*'?/)) return 'string';
    if (stream.match(/^-?\d+(\.\d+)?([eE][+-]?\d+)?/)) return 'number';
    if (stream.match(/^(true|false)\b/)) return 'bool';
    if (stream.match(/^null\b/)) return 'null';
    if (state.sql && stream.match(/^[A-Za-z_][\w.]*/)) {
      return /^(select|from|where|group|by|order|limit|and|or|not|as|count|sum|avg|min|max|having|desc|asc|show|describe|tables|like|in|is|null|match|query)$/i.test(stream.current()) ? 'keyword' : 'variableName';
    }
    stream.next();
    return 'punctuation';
  },
});

const ENDPOINTS = ['_search', '_count', '_mapping', '_settings', '_doc', '_create', '_update', '_explain', '_validate/query?explain', '_delete_by_query', '_update_by_query', '_refresh', '_stats', '_alias', '_field_caps', '_bulk', '_msearch', '_mget'];
const ROOTS = ['_cat/indices?v', '_cat/nodes?v', '_cat/shards?v', '_cat/aliases?v', '_cluster/health', '_cluster/stats', '_nodes/stats', '_aliases', '_data_stream', '_sql', '_plugins/_sql', '_tasks', '_search', '_mapping'];
const DSL = ['query', 'bool', 'must', 'must_not', 'should', 'filter', 'minimum_should_match', 'match', 'match_all', 'match_phrase', 'multi_match', 'query_string', 'simple_query_string', 'term', 'terms', 'range', 'gte', 'gt', 'lte', 'lt', 'exists', 'prefix', 'wildcard', 'regexp', 'fuzzy', 'ids', 'nested', 'path', 'size', 'from', 'sort', 'order', '_source', 'fields', 'highlight', 'track_total_hits', 'search_after', 'aggs', 'aggregations', 'terms', 'field', 'avg', 'sum', 'min', 'max', 'stats', 'cardinality', 'value_count', 'percentiles', 'histogram', 'date_histogram', 'calendar_interval', 'fixed_interval', 'interval', 'top_hits', 'filters', 'script', 'boost', 'analyzer', 'operator', 'mappings', 'properties', 'type', 'settings', 'number_of_shards', 'number_of_replicas', 'doc', 'doc_as_upsert', 'actions', 'add', 'remove', 'index', 'alias'];

function sources(schema: Schema | null): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const ns of schema?.namespaces ?? []) for (const r of [...ns.tables, ...(ns.aliases ?? []), ...(ns.data_streams ?? [])]) out.set(r.name, r.columns.map((c) => c.name).filter((c) => c !== '_id'));
  return out;
}

function consoleCompletion(schema: Schema | null) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const line = ctx.state.doc.lineAt(ctx.pos);
    const before = line.text.slice(0, ctx.pos - line.from);
    const known = sources(schema);
    const head = /^\s*(GET|POST|PUT|DELETE|HEAD|PATCH)\s+(\S*)$/i.exec(before);
    if (head) {
      const path = head[2] ?? '';
      const slash = path.lastIndexOf('/');
      const from = ctx.pos - (path.length - slash - 1);
      const inIndex = slash > 0 && indexOf(path) !== null;
      const options: Completion[] = inIndex
        ? ENDPOINTS.map((e) => ({ label: e, type: 'keyword' }))
        : [...[...known.keys()].map((n) => ({ label: n, type: 'class' })), ...ROOTS.map((r) => ({ label: r, type: 'keyword' }))];
      return { from, options, validFor: /^[\w.*?=\-/]*$/ };
    }
    const word = ctx.matchBefore(/"?[\w.@-]*/);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;
    if (/^\s*[A-Za-z]*$/.test(before) && !before.includes('"')) {
      return { from: line.from + (before.length - before.trimStart().length), options: METHODS.map((m) => ({ label: `${m} `, displayLabel: m, type: 'keyword' })) };
    }
    const doc = ctx.state.doc.toString();
    const cmd = consoleCommands(doc).find(([a, b]) => a <= ctx.pos && ctx.pos <= b + 1);
    const req = cmd ? parseRequest(doc.slice(cmd[0], cmd[1])) : null;
    const fields = (req && known.get(indexOf(req.path) ?? '')) ?? [];
    const quoted = word.text.startsWith('"');
    const wrap = (label: string): Completion => ({ label: quoted ? `"${label}"` : label, displayLabel: label, apply: `"${label}"` });
    return {
      from: word.from,
      options: [...fields.map((f) => ({ ...wrap(f), type: 'property', boost: 2 })), ...DSL.map((k) => ({ ...wrap(k), type: 'keyword' }))],
      validFor: /^"?[\w.@-]*$/,
    };
  };
}

function consoleProblems(text: string): Diagnostic[] {
  const out: Diagnostic[] = [];
  for (const [from, to] of consoleCommands(text)) {
    const part = text.slice(from, to);
    const r = parseRequest(part);
    if (!r || !r.body || ndjson(r.path)) continue;
    const bodyAt = from + part.indexOf(r.body);
    const p = checkJson(r.body.replace(/(^|\n)\s*(#|\/\/)[^\n]*/g, (s) => ' '.repeat(s.length)));
    if (p) out.push({ from: bodyAt + Math.min(p.at, r.body.length - 1), to: bodyAt + Math.min(p.at + 1, r.body.length), severity: 'error', message: m[p.message]() });
  }
  return out;
}

export function searchLanguage(schema: Schema | null, completes = true): Extension {
  return [
    consoleStream,
    completes ? consoleStream.data.of({ autocomplete: consoleCompletion(schema) }) : [],
    linter((view) => consoleProblems(view.state.doc.toString()), { delay: 300 }),
  ];
}

export interface KeyedRelation {
  name: string;
  columns: { name: string; type_name: string; primary_key: boolean }[];
  indexes: { name: string; columns: string[] }[];
}

const pretty = (v: unknown) => JSON.stringify(v, null, 2);

export function dynamoScan(table: string): string {
  return pretty({ Scan: { TableName: table } });
}

export function dynamoQuery(rel: KeyedRelation, index: string | null = null): string {
  const keys = index ? (rel.indexes.find((i) => i.name === index)?.columns ?? []) : rel.columns.filter((c) => c.primary_key).map((c) => c.name);
  const [pk, sk] = keys;
  if (!pk) return dynamoScan(rel.name);
  const typeOf = (name: string) => {
    const t = rel.columns.find((c) => c.name === name)?.type_name ?? 'S';
    return ['S', 'N', 'B'].includes(t) ? t : 'S';
  };
  const names: Record<string, string> = { '#pk': pk };
  const values: Record<string, Record<string, string>> = { ':pk': { [typeOf(pk)]: '' } };
  let condition = '#pk = :pk';
  if (sk) {
    names['#sk'] = sk;
    values[':sk'] = { [typeOf(sk)]: '' };
    condition += ' AND #sk >= :sk';
  }
  return pretty({ Query: { TableName: rel.name, ...(index ? { IndexName: index } : {}), KeyConditionExpression: condition, ExpressionAttributeNames: names, ExpressionAttributeValues: values } });
}

export type KeyOp = '' | '=' | '<' | '<=' | '>' | '>=' | 'between' | 'begins_with';
export type FilterOp = '=' | '<>' | '<' | '<=' | '>' | '>=' | 'begins_with' | 'contains' | 'exists' | 'not_exists';

export interface DynamoSpec {
  op: 'scan' | 'query';
  index: string | null;
  pk: string;
  skOp: KeyOp;
  sk: string;
  sk2: string;
  filters: { attr: string; op: FilterOp; value: string }[];
  limit: string;
  descending: boolean;
}

export function dynamoKeys(rel: KeyedRelation, index: string | null): [string | undefined, string | undefined] {
  const keys = index ? (rel.indexes.find((i) => i.name === index)?.columns ?? []) : rel.columns.filter((c) => c.primary_key).map((c) => c.name);
  return [keys[0], keys[1]];
}

export function dynamoType(rel: KeyedRelation, name: string): string {
  const t = rel.columns.find((c) => c.name === name)?.type_name.split(/\s/)[0] ?? 'S';
  return ['S', 'N', 'B', 'BOOL'].includes(t) ? t : 'S';
}

export function dynamoCall(rel: KeyedRelation, s: DynamoSpec): string {
  const names: Record<string, string> = {};
  const values: Record<string, Record<string, string | boolean>> = {};
  const name = (attr: string) => {
    const found = Object.entries(names).find(([, a]) => a === attr)?.[0];
    if (found) return found;
    const key = `#a${Object.keys(names).length}`;
    names[key] = attr;
    return key;
  };
  const value = (attr: string, v: string) => {
    const key = `:v${Object.keys(values).length}`;
    const t = dynamoType(rel, attr);
    values[key] = t === 'BOOL' ? { BOOL: v.trim() === 'true' } : { [t]: v };
    return key;
  };
  const test = (attr: string, op: KeyOp | FilterOp, v: string, v2 = ''): string => {
    const n = name(attr);
    if (op === 'exists') return `attribute_exists(${n})`;
    if (op === 'not_exists') return `attribute_not_exists(${n})`;
    if (op === 'begins_with' || op === 'contains') return `${op}(${n}, ${value(attr, v)})`;
    if (op === 'between') return `${n} BETWEEN ${value(attr, v)} AND ${value(attr, v2)}`;
    return `${n} ${op} ${value(attr, v)}`;
  };
  const [pk, sk] = dynamoKeys(rel, s.index);
  const call: Record<string, unknown> = { TableName: rel.name, ...(s.index ? { IndexName: s.index } : {}) };
  const query = s.op === 'query' && !!pk;
  if (query) {
    const key = [test(pk, '=', s.pk)];
    if (sk && s.skOp) key.push(test(sk, s.skOp, s.sk, s.sk2));
    call.KeyConditionExpression = key.join(' AND ');
  }
  const filter = s.filters.filter((f) => f.attr).map((f) => test(f.attr, f.op, f.value));
  if (filter.length) call.FilterExpression = filter.join(' AND ');
  if (Object.keys(names).length) call.ExpressionAttributeNames = names;
  if (Object.keys(values).length) call.ExpressionAttributeValues = values;
  const limit = Number.parseInt(s.limit, 10);
  if (limit > 0) call.Limit = limit;
  if (query && s.descending) call.ScanIndexForward = false;
  return pretty({ [query ? 'Query' : 'Scan']: call });
}
