import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { json } from '@codemirror/lang-json';
import { linter, type Diagnostic } from '@codemirror/lint';
import type { Extension } from '@codemirror/state';
import type { Schema } from './api';
import { m } from '../paraglide/messages';

export function splitCommands(text: string): [number, number][] {
  const out: [number, number][] = [];
  let depth = 0;
  let start = -1;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"' || c === "'") {
      i = stringEnd(text, i);
    } else if (c === '/' && text[i + 1] === '/') {
      const nl = text.indexOf('\n', i);
      i = nl < 0 ? text.length : nl;
    } else if (c === '{' || c === '[') {
      if (depth === 0) start = i;
      depth++;
    } else if ((c === '}' || c === ']') && depth > 0) {
      depth--;
      if (depth === 0 && start >= 0) {
        out.push([start, i + 1]);
        start = -1;
      }
    }
  }
  if (start >= 0) out.push([start, text.trimEnd().length]);
  return out;
}

function stringEnd(text: string, open: number): number {
  const q = text[open];
  for (let i = open + 1; i < text.length; i++) {
    if (text[i] === '\\') i++;
    else if (text[i] === q) return i;
  }
  return text.length;
}

export function uncomment(text: string): string {
  let out = '';
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      const end = stringEnd(text, i);
      out += text.slice(i, end + 1);
      i = end;
    } else if (c === '/' && text[i + 1] === '/') {
      const nl = text.indexOf('\n', i);
      const end = nl < 0 ? text.length : nl;
      out += ' '.repeat(end - i);
      i = end - 1;
    } else {
      out += c;
    }
  }
  return out;
}

export interface JsonProblem {
  at: number;
  message: 'json_expected_value' | 'json_expected_key' | 'json_expected_colon' | 'json_expected_comma' | 'json_unclosed_string' | 'json_trailing' | 'json_not_object';
}

export function checkJson(text: string): JsonProblem | null {
  let i = 0;
  const ws = () => {
    while (i < text.length && /\s/.test(text[i] ?? '')) i++;
  };
  const fail = (message: JsonProblem['message']): JsonProblem => ({ at: Math.min(i, text.length), message });
  const str = (): JsonProblem | null => {
    const end = stringEnd(text, i);
    if (end >= text.length) return fail('json_unclosed_string');
    i = end + 1;
    return null;
  };
  const value = (): JsonProblem | null => {
    ws();
    const c = text[i];
    if (c === '{') {
      i++;
      ws();
      if (text[i] === '}') return (i++, null);
      for (;;) {
        ws();
        if (text[i] !== '"') return fail('json_expected_key');
        const s = str();
        if (s) return s;
        ws();
        if (text[i] !== ':') return fail('json_expected_colon');
        i++;
        const v = value();
        if (v) return v;
        ws();
        if (text[i] === ',') {
          i++;
          continue;
        }
        if (text[i] === '}') return (i++, null);
        return fail('json_expected_comma');
      }
    }
    if (c === '[') {
      i++;
      ws();
      if (text[i] === ']') return (i++, null);
      for (;;) {
        const v = value();
        if (v) return v;
        ws();
        if (text[i] === ',') {
          i++;
          continue;
        }
        if (text[i] === ']') return (i++, null);
        return fail('json_expected_comma');
      }
    }
    if (c === '"') return str();
    const lit = /^(-?\d+(\.\d+)?([eE][+-]?\d+)?|true|false|null)/.exec(text.slice(i));
    if (!lit) return fail('json_expected_value');
    i += lit[0].length;
    return null;
  };
  ws();
  if (text[i] !== '{') return fail('json_not_object');
  const p = value();
  if (p) return p;
  ws();
  return i < text.length ? fail('json_trailing') : null;
}

export function problems(text: string): JsonProblem[] {
  const clean = uncomment(text);
  const out: JsonProblem[] = [];
  for (const [from, to] of splitCommands(clean)) {
    const p = checkJson(clean.slice(from, to));
    if (p) out.push({ at: from + p.at, message: p.message });
  }
  return out;
}

export function formatCommands(text: string): string {
  const ranges = splitCommands(text);
  let out = '';
  let last = 0;
  for (const [from, to] of ranges) {
    out += text.slice(last, from);
    const part = text.slice(from, to);
    try {
      out += part.includes('//') ? part : JSON.stringify(JSON.parse(part), null, 2);
    } catch {
      out += part;
    }
    last = to;
  }
  return out + text.slice(last);
}

const COMMANDS = ['find', 'aggregate', 'count', 'distinct', 'insert', 'update', 'delete', 'findAndModify', 'createIndexes', 'dropIndexes', 'listIndexes', 'listCollections', 'collStats', 'dbStats', 'explain', 'create', 'drop'];
const KEYS = ['filter', 'projection', 'sort', 'limit', 'skip', 'hint', 'pipeline', 'documents', 'updates', 'deletes', 'q', 'u', 'multi', 'upsert', 'key', 'query', 'indexes', 'verbosity', 'collation', 'allowDiskUse'];
const OPERATORS = ['$eq', '$ne', '$gt', '$gte', '$lt', '$lte', '$in', '$nin', '$and', '$or', '$nor', '$not', '$exists', '$type', '$regex', '$elemMatch', '$size', '$all', '$expr', '$set', '$unset', '$inc', '$push', '$pull', '$addToSet'];
const STAGES = ['$match', '$project', '$group', '$sort', '$limit', '$skip', '$unwind', '$lookup', '$addFields', '$set', '$count', '$facet', '$bucket', '$sortByCount', '$replaceRoot', '$sample', '$out', '$merge'];
const ACCUMULATORS = ['$sum', '$avg', '$min', '$max', '$first', '$last', '$push', '$addToSet', '$multiply', '$subtract', '$add', '$divide', '$cond', '$ifNull', '$concat', '$toString', '$dateToString'];

export function collectionAt(text: string, pos: number): string | null {
  const range = splitCommands(text).find(([a, b]) => a <= pos && pos <= b);
  const part = range ? text.slice(range[0], range[1]) : text;
  const hit = /"(find|aggregate|count|distinct|insert|update|delete|findAndModify)"\s*:\s*"([^"]+)"/.exec(part);
  return hit?.[2] ?? null;
}

export function fieldsOf(schema: Schema | null, collection: string | null): string[] {
  const out = new Set<string>();
  for (const ns of schema?.namespaces ?? []) {
    for (const r of [...ns.tables, ...ns.views]) {
      if (collection && r.name !== collection) continue;
      for (const c of r.columns) out.add(c.name);
    }
  }
  return [...out];
}

function collections(schema: Schema | null): string[] {
  return (schema?.namespaces ?? []).flatMap((ns) => [...ns.tables, ...ns.views].map((r) => r.name));
}

export function mqlCompletion(schema: Schema | null) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const word = ctx.matchBefore(/"?[\w$.]*/);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;
    const quoted = word.text.startsWith('"');
    const typed = quoted ? word.text.slice(1) : word.text;
    const text = ctx.state.doc.toString();
    const before = text.slice(0, word.from).trimEnd();
    const asValue = before.endsWith(':');
    const coll = collectionAt(text, ctx.pos);
    const fields = fieldsOf(schema, coll);
    const wrap = (label: string, type: string, detail?: string): Completion => ({ label, type, detail, apply: quoted ? label : `"${label}"` });
    let options: Completion[];
    if (typed.startsWith('$')) {
      options = [...STAGES, ...OPERATORS, ...ACCUMULATORS].map((o) => wrap(o, 'keyword')).concat(fields.map((f) => wrap(`$${f}`, 'column')));
    } else if (asValue && /"(find|aggregate|count|distinct|insert|update|delete|findAndModify|create|drop|listIndexes|collStats)"\s*:$/.test(before)) {
      options = collections(schema).map((c) => wrap(c, 'table'));
    } else {
      options = [...fields.map((f) => wrap(f, 'column')), ...COMMANDS.map((c) => wrap(c, 'function')), ...KEYS.map((k) => wrap(k, 'keyword'))];
    }
    return { from: quoted ? word.from + 1 : word.from, options, validFor: /^[\w$.]*$/ };
  };
}

export function mqlLanguage(schema: Schema | null, completes = true): Extension {
  const lang = json();
  return [
    lang,
    completes ? lang.language.data.of({ autocomplete: mqlCompletion(schema) }) : [],
    linter(
      (view): Diagnostic[] =>
        problems(view.state.doc.toString()).map((p) => {
          const len = view.state.doc.length;
          const from = Math.min(p.at, len);
          return { from, to: Math.min(from + 1, len), severity: 'error', message: m[p.message]() };
        }),
      { delay: 300 },
    ),
  ];
}
