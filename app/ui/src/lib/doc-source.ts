import type { Schema } from './api';
import type { Capabilities } from './bindings';
import { cellText } from './ejson';
import { collectionAt } from './mql';
import { indexOf, parseRequest } from './nosql2-lang';

export interface DocSource {
  collection: string;
  keys: string[] | null;
}

const bare = (name: string) => name.replace(/^"(.*)"$/, '$1').replaceAll('""', '"');

function selectedTable(text: string, cql: boolean): string | null {
  const name = '("(?:[^"]|"")+"|[\\w]+)';
  const re = new RegExp(`^\\s*SELECT\\s+${cql ? '(?:JSON\\s+)?' : ''}\\*\\s+FROM\\s+${name}(?:\\.${name})?`, 'i');
  const hit = re.exec(text);
  if (!hit?.[1]) return null;
  return bare(cql && hit[2] ? hit[2] : hit[1]);
}

function primaryKey(schema: Schema | null, table: string): string[] | null {
  for (const ns of schema?.namespaces ?? []) {
    const r = ns.tables.find((t) => t.name === table);
    if (r) {
      const keys = r.columns.filter((c) => c.primary_key).map((c) => c.name);
      return keys.length ? keys : null;
    }
  }
  return null;
}

export function docSource(caps: Capabilities, text: string, schema: Schema | null): DocSource | null {
  switch (caps.language.kind) {
    case 'mql': {
      const head = /^\s*\{\s*"(\w+)"/.exec(text)?.[1];
      if (head !== 'find' || /"projection"\s*:/.test(text)) return null;
      const collection = collectionAt(text, 0);
      return collection ? { collection, keys: null } : null;
    }
    case 'search': {
      const r = parseRequest(text.trim());
      if (!r || !/(^|\/)_search(\?|$)/.test(r.path)) return null;
      if (/"_source"\s*:\s*(false|\[|\{|")/.test(r.body) || /"fields"\s*:/.test(r.body)) return null;
      const index = indexOf(r.path);
      return index ? { collection: index, keys: null } : null;
    }
    case 'cql':
    case 'partiql': {
      let table = selectedTable(text, caps.language.kind === 'cql');
      if (!table && caps.language.kind === 'partiql' && text.trim().startsWith('{')) {
        try {
          const call = JSON.parse(text) as Record<string, Record<string, unknown> | undefined>;
          const req = call.Scan ?? call.Query;
          if (req && typeof req.TableName === 'string' && !req.ProjectionExpression && !req.Select) table = req.TableName;
        } catch {
          table = null;
        }
      }
      if (!table) return null;
      const keys = primaryKey(schema, table);
      return keys ? { collection: table, keys } : null;
    }
    default:
      return null;
  }
}

export function keyOf(doc: unknown, src: DocSource | null): string | null {
  if (doc === null || typeof doc !== 'object') return null;
  const o = doc as Record<string, unknown>;
  if (!src?.keys) return '_id' in o ? JSON.stringify(o._id) : null;
  if (src.keys.some((k) => !(k in o))) return null;
  return JSON.stringify(Object.fromEntries(src.keys.map((k) => [k, o[k]])));
}

export function keyLabel(doc: unknown, src: DocSource | null): string | null {
  if (doc === null || typeof doc !== 'object') return null;
  const o = doc as Record<string, unknown>;
  if (!src?.keys) return '_id' in o ? cellText(o._id) : null;
  return src.keys.map((k) => cellText(o[k]) ?? '∅').join(' · ');
}

export function freshCopy(doc: Record<string, unknown>, src: DocSource | null): Record<string, unknown> {
  const drop = src?.keys ? [] : ['_id', '_index'];
  return Object.fromEntries(Object.entries(doc).filter(([k]) => !drop.includes(k)));
}
