import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { StreamLanguage, type StringStream } from '@codemirror/language';
import type { Extension } from '@codemirror/state';

export type KeyLookup = (prefix: string) => Promise<string[]>;

export const COMMANDS: readonly [string, string][] = [
  ['GET', 'key'],
  ['SET', 'key value [EX s | PX ms | KEEPTTL] [NX | XX]'],
  ['MGET', 'key [key …]'],
  ['MSET', 'key value [key value …]'],
  ['GETRANGE', 'key start end'],
  ['SETRANGE', 'key offset value'],
  ['APPEND', 'key value'],
  ['STRLEN', 'key'],
  ['INCR', 'key'],
  ['INCRBY', 'key increment'],
  ['INCRBYFLOAT', 'key increment'],
  ['DECR', 'key'],
  ['DECRBY', 'key decrement'],
  ['GETDEL', 'key'],
  ['GETEX', 'key [EX s | PX ms | PERSIST]'],
  ['DEL', 'key [key …]'],
  ['UNLINK', 'key [key …]'],
  ['EXISTS', 'key [key …]'],
  ['TYPE', 'key'],
  ['TTL', 'key'],
  ['PTTL', 'key'],
  ['EXPIRE', 'key seconds'],
  ['PEXPIRE', 'key ms'],
  ['EXPIREAT', 'key unix-time'],
  ['PERSIST', 'key'],
  ['RENAME', 'key newkey'],
  ['RENAMENX', 'key newkey'],
  ['COPY', 'source destination [REPLACE]'],
  ['TOUCH', 'key [key …]'],
  ['SCAN', 'cursor [MATCH pattern] [COUNT n] [TYPE type]'],
  ['KEYS', 'pattern'],
  ['RANDOMKEY', ''],
  ['DBSIZE', ''],
  ['DUMP', 'key'],
  ['OBJECT', 'ENCODING | FREQ | IDLETIME | REFCOUNT key'],
  ['HGET', 'key field'],
  ['HSET', 'key field value [field value …]'],
  ['HSETNX', 'key field value'],
  ['HMGET', 'key field [field …]'],
  ['HGETALL', 'key'],
  ['HDEL', 'key field [field …]'],
  ['HEXISTS', 'key field'],
  ['HKEYS', 'key'],
  ['HVALS', 'key'],
  ['HLEN', 'key'],
  ['HINCRBY', 'key field increment'],
  ['HSCAN', 'key cursor [MATCH pattern] [COUNT n]'],
  ['LPUSH', 'key value [value …]'],
  ['RPUSH', 'key value [value …]'],
  ['LPOP', 'key [count]'],
  ['RPOP', 'key [count]'],
  ['LRANGE', 'key start stop'],
  ['LINDEX', 'key index'],
  ['LSET', 'key index value'],
  ['LLEN', 'key'],
  ['LREM', 'key count value'],
  ['LTRIM', 'key start stop'],
  ['LPOS', 'key value'],
  ['LMOVE', 'source destination LEFT|RIGHT LEFT|RIGHT'],
  ['BLPOP', 'key [key …] timeout'],
  ['BRPOP', 'key [key …] timeout'],
  ['SADD', 'key member [member …]'],
  ['SREM', 'key member [member …]'],
  ['SMEMBERS', 'key'],
  ['SISMEMBER', 'key member'],
  ['SCARD', 'key'],
  ['SINTER', 'key [key …]'],
  ['SUNION', 'key [key …]'],
  ['SDIFF', 'key [key …]'],
  ['SPOP', 'key [count]'],
  ['SRANDMEMBER', 'key [count]'],
  ['SSCAN', 'key cursor [MATCH pattern] [COUNT n]'],
  ['ZADD', 'key [NX | XX] [GT | LT] score member [score member …]'],
  ['ZREM', 'key member [member …]'],
  ['ZSCORE', 'key member'],
  ['ZINCRBY', 'key increment member'],
  ['ZCARD', 'key'],
  ['ZCOUNT', 'key min max'],
  ['ZRANGE', 'key start stop [BYSCORE | BYLEX] [REV] [LIMIT offset count] [WITHSCORES]'],
  ['ZREVRANGE', 'key start stop [WITHSCORES]'],
  ['ZRANGEBYSCORE', 'key min max [WITHSCORES] [LIMIT offset count]'],
  ['ZRANK', 'key member'],
  ['ZREVRANK', 'key member'],
  ['ZPOPMIN', 'key [count]'],
  ['ZPOPMAX', 'key [count]'],
  ['ZSCAN', 'key cursor [MATCH pattern] [COUNT n]'],
  ['XADD', 'key [MAXLEN ~ n] * field value [field value …]'],
  ['XRANGE', 'key start end [COUNT n]'],
  ['XREVRANGE', 'key end start [COUNT n]'],
  ['XLEN', 'key'],
  ['XDEL', 'key id [id …]'],
  ['XTRIM', 'key MAXLEN [~] n'],
  ['XREAD', '[COUNT n] STREAMS key [key …] id [id …]'],
  ['XINFO', 'STREAM | GROUPS | CONSUMERS key'],
  ['XGROUP', 'CREATE | DESTROY | SETID key group id'],
  ['XPENDING', 'key group'],
  ['XACK', 'key group id [id …]'],
  ['PFADD', 'key element [element …]'],
  ['PFCOUNT', 'key [key …]'],
  ['GEOADD', 'key longitude latitude member'],
  ['GEOSEARCH', 'key FROMMEMBER member BYRADIUS radius m|km'],
  ['SETBIT', 'key offset value'],
  ['GETBIT', 'key offset'],
  ['BITCOUNT', 'key [start end]'],
  ['PUBLISH', 'channel message'],
  ['PUBSUB', 'CHANNELS | NUMSUB | NUMPAT'],
  ['EVAL', 'script numkeys [key …] [arg …]'],
  ['EVALSHA', 'sha1 numkeys [key …] [arg …]'],
  ['FCALL', 'function numkeys [key …] [arg …]'],
  ['SCRIPT', 'EXISTS | FLUSH | LOAD'],
  ['FUNCTION', 'LIST | LOAD | DELETE | FLUSH'],
  ['INFO', '[section]'],
  ['PING', '[message]'],
  ['ECHO', 'message'],
  ['TIME', ''],
  ['CLIENT', 'LIST | INFO | ID | KILL | SETNAME | GETNAME'],
  ['CONFIG', 'GET | SET | RESETSTAT | REWRITE parameter'],
  ['SLOWLOG', 'GET [count] | LEN | RESET'],
  ['MEMORY', 'USAGE key | STATS | DOCTOR'],
  ['LATENCY', 'LATEST | DOCTOR | HISTORY event'],
  ['COMMAND', 'DOCS | COUNT | INFO | LIST'],
  ['ACL', 'WHOAMI | LIST | USERS | GETUSER | SETUSER | DELUSER'],
  ['MODULE', 'LIST'],
  ['ROLE', ''],
  ['LASTSAVE', ''],
  ['BGSAVE', ''],
  ['FLUSHDB', '[ASYNC | SYNC]'],
  ['FLUSHALL', '[ASYNC | SYNC]'],
  ['JSON.GET', 'key [path …]'],
  ['JSON.SET', 'key path value [NX | XX]'],
  ['JSON.DEL', 'key [path]'],
  ['JSON.TYPE', 'key [path]'],
  ['JSON.ARRAPPEND', 'key path value [value …]'],
  ['JSON.NUMINCRBY', 'key path number'],
  ['FT.SEARCH', 'index query'],
  ['FT._LIST', ''],
];

const NAMES = new Set(COMMANDS.map(([n]) => n));

const SUBCOMMANDS: Record<string, string[]> = {
  CLIENT: ['LIST', 'INFO', 'ID', 'KILL', 'SETNAME', 'GETNAME', 'NO-EVICT', 'PAUSE', 'UNPAUSE'],
  CONFIG: ['GET', 'SET', 'RESETSTAT', 'REWRITE'],
  SLOWLOG: ['GET', 'LEN', 'RESET'],
  MEMORY: ['USAGE', 'STATS', 'DOCTOR', 'MALLOC-STATS', 'PURGE'],
  LATENCY: ['LATEST', 'DOCTOR', 'HISTORY', 'RESET'],
  OBJECT: ['ENCODING', 'FREQ', 'IDLETIME', 'REFCOUNT'],
  XINFO: ['STREAM', 'GROUPS', 'CONSUMERS'],
  XGROUP: ['CREATE', 'DESTROY', 'SETID', 'CREATECONSUMER', 'DELCONSUMER'],
  SCRIPT: ['EXISTS', 'FLUSH', 'LOAD', 'KILL'],
  FUNCTION: ['LIST', 'LOAD', 'DELETE', 'FLUSH', 'DUMP', 'RESTORE', 'STATS'],
  COMMAND: ['DOCS', 'COUNT', 'INFO', 'LIST', 'GETKEYS'],
  ACL: ['WHOAMI', 'LIST', 'USERS', 'GETUSER', 'SETUSER', 'DELUSER', 'CAT', 'LOG'],
  MODULE: ['LIST', 'LOAD', 'UNLOAD'],
  PUBSUB: ['CHANNELS', 'NUMSUB', 'NUMPAT'],
};

const KEYLESS = new Set(['INFO', 'PING', 'ECHO', 'TIME', 'DBSIZE', 'RANDOMKEY', 'FLUSHDB', 'FLUSHALL', 'CLIENT', 'CONFIG', 'SLOWLOG', 'LATENCY', 'COMMAND', 'ACL', 'MODULE', 'ROLE', 'LASTSAVE', 'BGSAVE', 'PUBLISH', 'PUBSUB', 'SCRIPT', 'FUNCTION', 'SCAN', 'KEYS', 'FT._LIST']);

export function splitLines(text: string): [number, number][] {
  const out: [number, number][] = [];
  let i = 0;
  while (i < text.length) {
    while (i < text.length && /\s/.test(text[i] ?? '')) i++;
    if (i >= text.length) break;
    const start = i;
    const comment = text[i] === '#' || text.startsWith('//', i);
    let quote: string | null = null;
    while (i < text.length) {
      const c = text[i];
      if (quote) {
        if (c === '\\' && (quote === '"' || text[i + 1] === "'")) i++;
        else if (c === quote) quote = null;
      } else if (c === '\n') break;
      else if (!comment && (c === '"' || c === "'") && (i === start || /\s/.test(text[i - 1] ?? ''))) quote = c;
      i++;
    }
    if (!comment) out.push([start, start + text.slice(start, i).trimEnd().length]);
  }
  return out;
}

export function words(line: string): string[] {
  const out: string[] = [];
  let i = 0;
  while (i < line.length) {
    while (i < line.length && /\s/.test(line[i] ?? '')) i++;
    if (i >= line.length) break;
    let w = '';
    const q = line[i] === '"' || line[i] === "'" ? line[i] : null;
    if (q) i++;
    while (i < line.length) {
      const c = line[i] ?? '';
      if (q) {
        if (c === '\\' && q === '"') {
          const e = line[i + 1] ?? '';
          w += ({ n: '\n', r: '\r', t: '\t' } as Record<string, string>)[e] ?? e;
          i += 2;
          continue;
        }
        if (c === '\\' && q === "'" && line[i + 1] === "'") {
          w += "'";
          i += 2;
          continue;
        }
        if (c === q) {
          i++;
          break;
        }
      } else if (/\s/.test(c)) break;
      w += c;
      i++;
    }
    out.push(w);
  }
  return out;
}

export function quoteArg(arg: string): string {
  if (arg && /^[\p{L}\p{N}:_\-.*/@+$#,]+$/u.test(arg)) return arg;
  return `"${arg.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n').replace(/\r/g, '\\r').replace(/\t/g, '\\t')}"`;
}

export const commandLine = (args: readonly string[]) => args.map(quoteArg).join(' ');

export function formatRedis(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const t = line.trim();
      if (!t || t.startsWith('#') || t.startsWith('//')) return t;
      const name = /^\S+/.exec(t)?.[0] ?? '';
      return name.toUpperCase() + t.slice(name.length).replace(/^\s+/, ' ');
    })
    .join('\n');
}

export const globEscape = (s: string) => s.replace(/[*?[\]\\]/g, (c) => `\\${c}`);

interface Mode {
  word: number;
  quote: string | null;
}

const lang = StreamLanguage.define<Mode>({
  name: 'redis',
  startState: () => ({ word: 0, quote: null }),
  token(stream: StringStream, state: Mode) {
    if (stream.sol() && !state.quote) state.word = 0;
    if (state.quote) {
      while (!stream.eol()) {
        const c = stream.next();
        if (c === '\\') stream.next();
        else if (c === state.quote) {
          state.quote = null;
          break;
        }
      }
      return 'string';
    }
    if (stream.eatSpace()) return null;
    if (state.word === 0 && (stream.match('#') || stream.match('//'))) {
      stream.skipToEnd();
      return 'comment';
    }
    const c = stream.peek();
    if (c === '"' || c === "'") {
      stream.next();
      state.quote = c;
      state.word++;
      while (!stream.eol()) {
        const n = stream.next();
        if (n === '\\') stream.next();
        else if (n === c) {
          state.quote = null;
          break;
        }
      }
      return 'string';
    }
    stream.match(/^\S+/);
    const w = stream.current();
    state.word++;
    if (state.word === 1) return NAMES.has(w.toUpperCase()) || w.includes('.') ? 'keyword' : 'variableName';
    if (/^-?\d+(\.\d+)?$/.test(w)) return 'number';
    return null;
  },
  languageData: { commentTokens: { line: '#' } },
});

function completer(keys: KeyLookup | null) {
  const commands: Completion[] = COMMANDS.map(([label, args]) => ({ label, detail: args, type: 'keyword' }));
  return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
    const line = ctx.state.doc.lineAt(ctx.pos);
    const before = line.text.slice(0, ctx.pos - line.from);
    if (/^\s*(#|\/\/)/.test(before)) return null;
    const parts = before.trimStart().split(/\s+/);
    const current = parts.at(-1) ?? '';
    if (current.startsWith('"') || current.startsWith("'")) return null;
    const from = ctx.pos - current.length;
    if (parts.length <= 1) {
      if (!current && !ctx.explicit) return null;
      return { from, options: commands, validFor: /^[\w.]*$/ };
    }
    const name = (parts[0] ?? '').toUpperCase();
    const subs = SUBCOMMANDS[name];
    if (parts.length === 2 && subs) return { from, options: subs.map((label) => ({ label, type: 'keyword' })), validFor: /^[\w-]*$/ };
    if (!keys || KEYLESS.has(name) || (!current && !ctx.explicit)) return null;
    const found = await keys(current).catch(() => []);
    if (ctx.aborted || !found.length) return null;
    return { from, options: found.map((label) => ({ label, type: 'variable' })), filter: false };
  };
}

export function redisLanguage(keys: KeyLookup | null, completes = true): Extension {
  return [lang, completes ? lang.data.of({ autocomplete: completer(keys) }) : []];
}

const READS = new Set(['GET', 'MGET', 'GETRANGE', 'STRLEN', 'EXISTS', 'TYPE', 'TTL', 'PTTL', 'DBSIZE', 'SCAN', 'KEYS', 'HGET', 'HMGET', 'HGETALL', 'HKEYS', 'HVALS', 'HLEN', 'HEXISTS', 'HSCAN', 'LRANGE', 'LINDEX', 'LLEN', 'LPOS', 'SMEMBERS', 'SISMEMBER', 'SCARD', 'SSCAN', 'SINTER', 'SUNION', 'SDIFF', 'SRANDMEMBER', 'ZRANGE', 'ZREVRANGE', 'ZRANGEBYSCORE', 'ZSCORE', 'ZRANK', 'ZREVRANK', 'ZCARD', 'ZCOUNT', 'ZSCAN', 'XRANGE', 'XREVRANGE', 'XLEN', 'XINFO', 'XREAD', 'XPENDING', 'INFO', 'PING', 'ECHO', 'TIME', 'RANDOMKEY', 'PFCOUNT', 'GETBIT', 'BITCOUNT', 'JSON.GET', 'JSON.TYPE', 'ROLE', 'LASTSAVE', 'DUMP', 'OBJECT']);

export function readsOnly(line: string): boolean {
  const [name = '', sub = ''] = words(line).map((w) => w.toUpperCase());
  if (READS.has(name)) return true;
  if (name === 'CONFIG') return sub === 'GET';
  if (name === 'CLIENT') return ['LIST', 'INFO', 'ID', 'GETNAME'].includes(sub);
  if (name === 'SLOWLOG') return sub === 'GET' || sub === 'LEN';
  if (name === 'MEMORY') return sub === 'USAGE' || sub === 'STATS' || sub === 'DOCTOR';
  return false;
}

export function sentinelMaster(label: string): { name: string; address: string } | null {
  const m = /·\s*(\S+)\s+(\S+:\d+)\s*$/.exec(label);
  return m?.[1] && m[2] ? { name: m[1], address: m[2] } : null;
}
