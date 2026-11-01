import type { Dialect } from '../bindings';
import type { Catalog } from './catalog';
import { NOT_COLUMNS } from './dialects';
import { parse, queryOf, refName, visibleRefs, type Block, type Parsed, type Ref } from './parse';
import { matchesRef, Resolver } from './resolve';
import { isName, lower, scan, statements, type Token } from './scan';

export interface Finding {
  from: number;
  to: number;
  kind: 'table' | 'column';
  name: string;
  table: string | null;
  elsewhere?: string;
}

const CHECKED = new Set(['select', 'with', 'insert', 'update', 'delete', 'merge']);
const SYSTEM = /^(pg_|sqlite_|duckdb_|sys|information_schema|dual$|mysql$|performance_schema)/i;
const COLUMN_CLAUSES = new Set(['select', 'where', 'and', 'or', 'not', 'on', 'by', 'having', 'set', 'when', 'then', 'else', 'case', 'in', 'between', 'like', 'is', 'returning', 'qualify']);
const STOP_CLAUSES = new Set(['from', 'join', 'into', 'update', 'using', 'table', 'apply', 'values', 'limit', 'offset', 'top', 'as', 'with', 'fetch', 'window', 'over', 'partition', 'straight_join', 'delete', 'insert', 'merge']);
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function createdIn(text: string, dialect: Dialect, tokens: Token[]): Set<string> {
  const out = new Set<string>();
  for (const st of statements(text, dialect, tokens)) {
    const t = st.tokens;
    for (let i = 0; i < t.length; i++) {
      const w = lower(t[i]);
      const make = w === 'table' || w === 'view' ? ['create', 'temporary', 'temp', 'replace', 'materialized', 'unlogged', 'global', 'local', 'or'].includes(lower(t[i - 1])) : w === 'into' && lower(t[0]) === 'select';
      if (!make) continue;
      let k = i + 1;
      if (lower(t[k]) === 'if') k += 3;
      while (isName(t[k]) && t[k + 1]?.text === '.' && isName(t[k + 2])) k += 2;
      const name = t[k];
      if (isName(name)) out.add(name.text.toLowerCase());
    }
  }
  return out;
}

export function inspect(text: string, dialect: Dialect, catalog: Catalog | null): Finding[] {
  if (!catalog || catalog.empty) return [];
  const all = scan(text, dialect);
  const created = createdIn(text, dialect, all);
  const out: Finding[] = [];
  for (const st of statements(text, dialect, all)) {
    const head = lower(st.tokens[0]);
    if (!CHECKED.has(head) || st.tokens.some((t) => ['pivot', 'unpivot', 'lateral', 'connect'].includes(lower(t)))) continue;
    const parsed = parse(st.tokens);
    const r = new Resolver(parsed, catalog);
    tables(parsed, r, catalog, created, out);
    columns(parsed, r, out);
  }
  return out;
}

function tables(p: Parsed, r: Resolver, catalog: Catalog, created: Set<string>, out: Finding[]) {
  for (const b of p.blocks) {
    for (const ref of b.refs) {
      if (ref.kind !== 'relation') continue;
      const name = ref.parts.at(-1) ?? '';
      const tok = ref.nameToks.at(-1);
      if (!tok || !name || /^[#@]/.test(name) || created.has(name.toLowerCase())) continue;
      if (ref.parts.length === 1 && (r.cte(name) || SYSTEM.test(name))) continue;
      const qualifier = ref.parts.slice(0, -1);
      if (qualifier.length && !catalog.knowsQualifier(qualifier)) continue;
      if (catalog.find(ref.parts).length || (qualifier.length === 0 && catalog.find([name]).length)) continue;
      const elsewhere = qualifier.length === 0 && catalog.strict ? catalog.schemasOf(name)[0] : undefined;
      out.push({ from: tok.from, to: tok.to, kind: 'table', name, table: null, ...(elsewhere ? { elsewhere } : {}) });
    }
  }
}

function declared(p: Parsed): Set<Token> {
  const s = new Set<Token>();
  for (const b of p.blocks) {
    for (const ref of b.refs) {
      for (const t of ref.nameToks) s.add(t);
      if (ref.aliasTok) s.add(ref.aliasTok);
    }
  }
  for (const c of p.ctes) s.add(c.tok);
  return s;
}

function clauseAt(p: Parsed, i: number): string {
  let b: Block | null = p.blocks[p.blockAt[i] ?? 0] ?? p.root;
  let k = i - 1;
  while (b) {
    for (; k > b.open; k--) {
      if (p.blockAt[k] !== b.id) continue;
      const w = lower(p.tokens[k]);
      if (COLUMN_CLAUSES.has(w) || STOP_CLAUSES.has(w)) return w;
    }
    if (b.query || !b.parent) return '';
    k = b.open - 1;
    b = b.parent;
  }
  return '';
}

function columns(p: Parsed, r: Resolver, out: Finding[]) {
  const skip = declared(p);
  const t = p.tokens;
  for (let i = 0; i < t.length; i++) {
    const tok = t[i];
    if (!isName(tok) || skip.has(tok)) continue;
    const next = t[i + 1];
    if (next && next.k === 'punct' && (next.text === '(' || next.text === '.')) continue;
    const block = p.blocks[p.blockAt[i] ?? 0] ?? p.root;
    const prev = t[i - 1];
    if (prev?.k === 'punct' && prev.text === '.' && prev.to === tok.from) {
      qualified(p, r, i, block, out);
      continue;
    }
    if (block.columnsOf) {
      const cols = r.columns(block.columnsOf);
      if (cols.complete && !cols.list.some((c) => same(c.name, tok.text))) out.push({ from: tok.from, to: tok.to, kind: 'column', name: tok.text, table: tableName(block.columnsOf) });
      continue;
    }
    if (tok.k === 'word' && NOT_COLUMNS.has(tok.text.toLowerCase())) continue;
    if (prev && ((isName(prev) && !(prev.k === 'word' && NOT_COLUMNS.has(prev.text.toLowerCase()))) || prev.k === 'number' || prev.k === 'string' || (prev.k === 'op' && prev.text === '::') || (prev.k === 'punct' && prev.text === ')'))) continue;
    if (!COLUMN_CLAUSES.has(clauseAt(p, i))) continue;
    const visible = visibleRefs(block);
    if (!visible.length || visible.some(({ ref }) => !r.columns(ref).complete)) continue;
    const q = queryOf(block);
    const known = (name: string) =>
      visible.some(({ ref }) => r.columns(ref).list.some((c) => same(c.name, name)) || matchesRef(ref, name)) ||
      p.blocks.some((b) => b.items.some((x) => x.alias && x.name !== null && same(x.name, name))) ||
      p.ctes.some((c) => same(c.name, name));
    if (known(tok.text)) continue;
    out.push({ from: tok.from, to: tok.to, kind: 'column', name: tok.text, table: q.refs.length === 1 && q.refs[0] ? tableName(q.refs[0]) : null });
  }
}

function qualified(p: Parsed, r: Resolver, i: number, block: Block, out: Finding[]) {
  const tok = p.tokens[i] as Token;
  const qual = p.tokens[i - 2];
  if (!isName(qual)) return;
  const before = p.tokens[i - 3];
  if (before?.k === 'punct' && before.text === '.') return;
  const ref = visibleRefs(block).find(({ ref }) => matchesRef(ref, qual.text))?.ref;
  if (!ref) return;
  const cols = r.columns(ref);
  if (!cols.complete || cols.list.some((c) => same(c.name, tok.text))) return;
  out.push({ from: tok.from, to: tok.to, kind: 'column', name: tok.text, table: tableName(ref) });
}

const tableName = (ref: Ref) => (ref.kind === 'relation' ? (ref.parts.at(-1) ?? null) : refName(ref));
