import type { KeyInfo_Serialize as KeyInfo, KeyWrite_Deserialize as KeyWrite } from './bindings';
import { commandLine } from './redis';

export const SEPARATOR = ':';

export const KINDS = ['string', 'hash', 'list', 'set', 'zset', 'stream', 'ReJSON-RL'] as const;
export type Kind = (typeof KINDS)[number];

export const isKnownKind = (k: string): k is Kind => (KINDS as readonly string[]).includes(k);

export interface KeyRow {
  kind: 'folder' | 'key';
  path: string;
  label: string;
  depth: number;
  count: number;
  info: KeyInfo | null;
  open: boolean;
}

interface Node {
  folders: Map<string, Node>;
  keys: KeyInfo[];
  count: number;
}

const node = (): Node => ({ folders: new Map(), keys: [], count: 0 });

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

export function keyTree(keys: readonly KeyInfo[], open: ReadonlySet<string>, sep = SEPARATOR): KeyRow[] {
  const root = node();
  for (const k of keys) {
    const parts = k.key.split(sep);
    let at = root;
    at.count++;
    for (let i = 0; i < parts.length - 1; i++) {
      const name = parts[i] ?? '';
      let next = at.folders.get(name);
      if (!next) {
        next = node();
        at.folders.set(name, next);
      }
      next.count++;
      at = next;
    }
    at.keys.push(k);
  }
  const out: KeyRow[] = [];
  const walk = (n: Node, prefix: string, depth: number) => {
    const folders = [...n.folders.entries()].sort((a, b) => collator.compare(a[0], b[0]));
    for (const [name, child] of folders) {
      const path = `${prefix}${name}${sep}`;
      const isOpen = open.has(path);
      out.push({ kind: 'folder', path, label: name, depth, count: child.count, info: null, open: isOpen });
      if (isOpen) walk(child, path, depth + 1);
    }
    const leaves = [...n.keys].sort((a, b) => collator.compare(a.key, b.key));
    for (const k of leaves) out.push({ kind: 'key', path: k.key, label: k.key.slice(prefix.length) || k.key, depth, count: 0, info: k, open: false });
  };
  walk(root, '', 0);
  return out;
}

export function foldersOf(key: string, sep = SEPARATOR): string[] {
  const parts = key.split(sep);
  return parts.slice(0, -1).map((_, i) => `${parts.slice(0, i + 1).join(sep)}${sep}`);
}

export function mergeKeys(have: readonly KeyInfo[], page: readonly KeyInfo[]): KeyInfo[] {
  const at = new Map(have.map((k, i) => [k.key, i]));
  const out = [...have];
  for (const k of page) {
    const i = at.get(k.key);
    if (i === undefined) {
      at.set(k.key, out.length);
      out.push(k);
    } else out[i] = k;
  }
  return out;
}

const args = (...a: (string | number)[]) => a.map(String);

export function writeCommands(w: KeyWrite): string[][] {
  switch (w.op) {
    case 'set_string':
      return [args('SET', w.key, w.value, 'KEEPTTL')];
    case 'hash_set':
      return [args('HSET', w.key, w.field, w.value)];
    case 'hash_delete':
      return [args('HDEL', w.key, ...w.fields)];
    case 'list_set':
      return [args('LSET', w.key, w.index ?? 0, w.value)];
    case 'list_push':
      return [args(w.head ? 'LPUSH' : 'RPUSH', w.key, ...w.values)];
    case 'list_remove':
      return [args('LREM', w.key, 1, w.value)];
    case 'set_add':
      return [args('SADD', w.key, ...w.members)];
    case 'set_remove':
      return [args('SREM', w.key, ...w.members)];
    case 'zset_add':
      return [args('ZADD', w.key, w.score ?? 0, w.member)];
    case 'zset_remove':
      return [args('ZREM', w.key, ...w.members)];
    case 'stream_add':
      return [args('XADD', w.key, '*', ...w.fields.flatMap((f) => [f.name, f.value]))];
    case 'stream_delete':
      return [args('XDEL', w.key, ...w.ids)];
    case 'json_set':
      return [args('JSON.SET', w.key, '$', w.value)];
    case 'expire':
      return [w.ttl_ms === null || w.ttl_ms === undefined ? args('PERSIST', w.key) : args('PEXPIRE', w.key, w.ttl_ms)];
    case 'rename':
      return [args('RENAMENX', w.key, w.to)];
    case 'delete':
      return [args('UNLINK', ...w.keys)];
    case 'create': {
      const { name, value } = w.first;
      const first =
        w.kind === 'hash'
          ? args('HSET', w.key, name, value)
          : w.kind === 'list'
            ? args('RPUSH', w.key, value)
            : w.kind === 'set'
              ? args('SADD', w.key, name)
              : w.kind === 'zset'
                ? args('ZADD', w.key, value, name)
                : w.kind === 'stream'
                  ? args('XADD', w.key, '*', name, value)
                  : w.kind === 'ReJSON-RL'
                    ? args('JSON.SET', w.key, '$', value, 'NX')
                    : args('SET', w.key, value, 'NX');
      return w.ttl_ms ? [first, args('PEXPIRE', w.key, w.ttl_ms)] : [first];
    }
  }
}

export const describeWrite = (w: KeyWrite): string[] => writeCommands(w).map((c) => commandLine(c.length > 8 ? [...c.slice(0, 8), '…'] : c));

export function parseTtl(text: string): number | null {
  const m = /^\s*(\d+(?:[.,]\d+)?)\s*(ms|s|m|min|h|d|g)?\s*$/i.exec(text);
  if (!m) return null;
  const n = Number((m[1] ?? '').replace(',', '.'));
  const unit = (m[2] ?? 's').toLowerCase();
  const factor: Record<string, number> = { ms: 1, s: 1000, m: 60_000, min: 60_000, h: 3_600_000, d: 86_400_000, g: 86_400_000 };
  const ms = n * (factor[unit] ?? 1000);
  return Number.isFinite(ms) && ms >= 1 ? Math.round(ms) : null;
}

export function shortTtl(ms: number | null | undefined, day = 'g'): string {
  if (ms === null || ms === undefined) return '';
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.round(s / 60)} min`;
  if (s < 86_400) return `${Math.round(s / 3600)} h`;
  return `${Math.round(s / 86_400)} ${day}`;
}

export function asJson(text: string): unknown {
  const t = text.trim();
  if (!(t.startsWith('{') || t.startsWith('['))) return undefined;
  try {
    return JSON.parse(t) as unknown;
  } catch {
    return undefined;
  }
}
