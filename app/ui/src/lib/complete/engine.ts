import type { Dialect } from '../bindings';
import type { Catalog, Link, RelEntry } from './catalog';
import { quoteName } from '../sqlname';
import { AFTER_RELATION, EXPRESSION, functionsOf, STATEMENT_START, typesOf, type Engine } from './dialects';
import { blockAtPos, parse, queryOf, refName, visibleRefs, type Block, type Parsed, type Ref } from './parse';
import { Resolver } from './resolve';
import { insideLiteral, isName, lower, scan, statementAt, statements, type Token } from './scan';

export type CandidateType = 'table' | 'view' | 'column' | 'function' | 'keyword' | 'schema' | 'database' | 'alias' | 'join' | 'cte' | 'type';

export interface Candidate {
  label: string;
  type: CandidateType;
  apply?: string;
  detail?: string;
  boost: number;
  call?: boolean;
  chain?: boolean;
  source?: RelEntry;
}

export type ContextKind = 'start' | 'relation' | 'join' | 'after-relation' | 'qualified' | 'expression' | 'on' | 'columns' | 'type' | 'database' | 'none';

export interface CompletionAt {
  from: number;
  to: number;
  context: ContextKind;
  options: Candidate[];
}

export interface Env {
  dialect: Dialect;
  engine: Engine;
  catalog: Catalog | null;
  upper: boolean;
  recency?: (c: Candidate) => number;
}

const RELATION_WORDS = new Set(['from', 'join', 'into', 'update', 'table', 'apply', 'using', 'straight_join']);
const EXPRESSION_WORDS = new Set(['select', 'where', 'and', 'or', 'not', 'having', 'by', 'set', 'when', 'then', 'else', 'case', 'returning', 'values', 'on', 'limit', 'offset', 'top', 'in', 'between', 'like', 'is', 'distinct', 'qualify']);
const NAMING_AFTER = new Set(['table', 'view', 'index', 'database', 'schema', 'function', 'procedure', 'trigger', 'sequence', 'type', 'exists', 'replace', 'temporary', 'temp', 'materialized']);

interface Here {
  tokens: Token[];
  parsed: Parsed;
  block: Block;
  at: number;
  prefix: string;
  quoted: Token | null;
  close: string;
}

export function complete(text: string, pos: number, env: Env, explicit = false): CompletionAt | null {
  const all = scan(text, env.dialect);
  if (insideLiteral(all, pos)) return null;
  const st = statementAt(statements(text, env.dialect, all), pos);
  const tokens = st.tokens;
  let at = tokens.findIndex((t) => t.to > pos || (t.to === pos && t.from < pos && isName(t)));
  if (at < 0) at = tokens.length;
  let typing = tokens[at];
  if (typing && !(typing.from < pos && isName(typing))) {
    if (typing.from < pos && typing.k !== 'punct' && typing.k !== 'op') return null;
    typing = undefined;
  }
  const quoted = typing?.k === 'qident' ? typing : null;
  const from = typing ? typing.from + (quoted ? 1 : 0) : pos;
  const prefix = text.slice(from, pos);
  const parsed = parse(tokens);
  const { block } = blockAtPos(parsed, typing ? typing.from : pos);
  const opener = quoted ? text[quoted.from] : '';
  const here: Here = { tokens, parsed, block, at, prefix, quoted, close: opener === '[' ? ']' : (opener ?? '"') };
  const prev = tokens[at - 1];
  const r = new Resolver(parsed, env.catalog);
  const typedCase = caseOf(tokens.slice(0, at));
  if (typedCase !== null) env = { ...env, upper: typedCase };

  const result = (context: ContextKind, options: Candidate[]): CompletionAt => ({ from, to: pos, context, options: finish(options, env, here) });

  if (prev?.k === 'punct' && prev.text === '.') {
    const chain: string[] = [];
    let k = at - 2;
    while (isName(tokens[k])) {
      chain.unshift((tokens[k] as Token).text);
      const dot = tokens[k - 1];
      if (dot?.k === 'punct' && dot.text === '.') k -= 2;
      else break;
    }
    if (!chain.length) return null;
    return result('qualified', qualified(chain, here, r, env));
  }
  if (!typing && !explicit && !auto(prev)) return null;
  if (prev?.k === 'op' && prev.text === '::') return result('type', types(env));
  if (block.columnsOf) return result('columns', columnsOfRef(block.columnsOf, r, env, 6, false));
  if (lower(prev) === 'as' && castCall(here)) return result('type', types(env));
  if (lower(prev) === 'as' || (lower(tokens[0]) === 'create' && NAMING_AFTER.has(lower(prev)))) return null;
  if (lower(prev) === 'use' && at === 1) return result('database', databases(env));
  if (!prev) return result('start', keywords(STATEMENT_START, 0));
  const clause = clauseOf(here);
  if (clause && RELATION_WORDS.has(clause.word)) {
    const expectsName = clause.index === at - 1 || (prev.k === 'punct' && prev.text === ',') || ['lateral', 'only'].includes(lower(prev));
    const isJoin = clause.word === 'join' || clause.word === 'apply';
    if (expectsName) return result(isJoin ? 'join' : 'relation', relations(here, r, env, isJoin));
    if (isName(prev) || (prev.k === 'punct' && prev.text === ')')) return result('after-relation', afterRelation(here, isJoin));
    return result('expression', expression(here, r, env, clause.word));
  }
  if (!clause) return result('start', keywords(STATEMENT_START, 0));
  if (clause.word === 'on' && clause.index === at - 1) return result('on', [...joinConditions(here, r, env), ...expression(here, r, env, 'on')]);
  if (EXPRESSION_WORDS.has(clause.word) || clause.word === 'on') return result(clause.word === 'on' ? 'on' : 'expression', expression(here, r, env, clause.word));
  return result('expression', expression(here, r, env, clause.word));
}

const CASE_WORDS = new Set(['select', 'from', 'where', 'join', 'on', 'and', 'or', 'group', 'order', 'by', 'insert', 'into', 'update', 'set', 'delete', 'with', 'as', 'left', 'inner', 'values', 'having', 'limit']);

function caseOf(tokens: Token[]): boolean | null {
  for (let i = tokens.length - 1; i >= 0; i--) {
    const t = tokens[i] as Token;
    if (t.k === 'word' && CASE_WORDS.has(t.text.toLowerCase())) return t.text === t.text.toUpperCase();
  }
  return null;
}

function auto(prev: Token | undefined): boolean {
  const w = lower(prev);
  return w === 'on' || w === 'join';
}

function castCall(here: Here): boolean {
  const b = here.block;
  if (b.query || b.open < 0) return false;
  const fn = lower(here.tokens[b.open - 1]);
  return fn === 'cast' || fn === 'try_cast' || fn === 'convert';
}

function clauseOf(here: Here): { word: string; index: number } | null {
  const { tokens, parsed } = here;
  let b: Block | null = here.block;
  let k = here.at - 1;
  while (b) {
    for (; k > b.open; k--) {
      if (parsed.blockAt[k] !== b.id) continue;
      const w = lower(tokens[k]);
      if (RELATION_WORDS.has(w) || EXPRESSION_WORDS.has(w) || w === 'with' || w === 'group' || w === 'order' || w === 'partition') {
        if (w === 'and' && between(tokens, k, b.id, parsed)) continue;
        return { word: w, index: k };
      }
    }
    if (!b.query && b.parent) {
      k = b.open - 1;
      b = b.parent;
      continue;
    }
    if (b.query && b.parent) return null;
    b = null;
  }
  return null;
}

function between(tokens: Token[], k: number, block: number, parsed: Parsed): boolean {
  for (let j = k - 1; j > k - 6 && j >= 0; j--) {
    if (parsed.blockAt[j] !== block) continue;
    const w = lower(tokens[j]);
    if (w === 'between') return true;
    if (w === 'and' || w === 'or' || w === 'where') return false;
  }
  return false;
}

function keywords(list: readonly string[], boost: number): Candidate[] {
  return list.map((k) => ({ label: k, type: 'keyword', boost }));
}

const q = (name: string, env: Env) => quoteName(env.dialect, name);

function relEntry(rel: RelEntry, env: Env, boost: number, bare: boolean): Candidate {
  return {
    label: rel.name,
    type: rel.kind,
    apply: bare || rel.home ? q(rel.name, env) : `${q(rel.ns, env)}.${q(rel.name, env)}`,
    detail: rel.home || bare ? undefined : rel.ns,
    boost: boost + (rel.kind === 'table' ? 1 : 0) + (rel.home ? 1 : 0),
  };
}

function relations(here: Here, r: Resolver, env: Env, join: boolean): Candidate[] {
  const out: Candidate[] = here.parsed.ctes.map((c) => ({ label: c.name, type: 'cte', apply: q(c.name, env), boost: 8 }));
  const cat = env.catalog;
  if (!cat) return out;
  const linked = new Set<RelEntry>();
  if (join) {
    const scope = queryOf(here.block).refs;
    for (const ref of scope) {
      const rel = r.relation(ref);
      if (!rel) continue;
      for (const l of cat.linksOf(rel)) linked.add(l.child === rel ? l.parent : l.child);
    }
    out.push(...joins(here, r, env));
  }
  for (const rel of cat.relations()) out.push(relEntry(rel, env, linked.has(rel) ? 6 : 0, false));
  for (const ns of cat.namespaceNames()) if (!cat.isHome(ns)) out.push({ label: ns, type: 'schema', apply: `${q(ns, env)}.`, boost: -1, chain: true });
  if (env.dialect === 'mssql' || env.dialect === 'mysql') {
    for (const db of cat.databases) if (db.toLowerCase() !== cat.current.toLowerCase()) out.push({ label: db, type: 'database', apply: `${q(db, env)}.`, boost: -3, chain: true });
  }
  return out;
}

export function aliasFor(name: string, taken: Set<string>): string {
  const words = name.split(/[_\s]+|(?<=[a-z])(?=[A-Z])/).filter(Boolean);
  const base = (words.length > 1 ? words.map((w) => w[0]).join('') : name.slice(0, 1)).toLowerCase() || 't';
  let alias = base;
  for (let n = 2; taken.has(alias) || ['as', 'by', 'do', 'if', 'in', 'is', 'of', 'on', 'or', 'to'].includes(alias); n++) alias = `${base}${n}`;
  return alias;
}

function condition(l: Link, childQ: string, parentQ: string, env: Env): string {
  return l.fk.columns.map((c, i) => `${childQ}.${q(c, env)} = ${parentQ}.${q(l.fk.ref_columns[i] ?? c, env)}`).join(' AND ');
}

function joins(here: Here, r: Resolver, env: Env): Candidate[] {
  const cat = env.catalog;
  if (!cat) return [];
  const scope = queryOf(here.block).refs;
  const taken = new Set(scope.map((x) => (refName(x) ?? '').toLowerCase()));
  const out: Candidate[] = [];
  const seen = new Set<string>();
  for (const ref of scope) {
    const rel = r.relation(ref);
    const name = refName(ref);
    if (!rel || !name) continue;
    for (const l of cat.linksOf(rel)) {
      const other = l.child === rel ? l.parent : l.child;
      const alias = aliasFor(other.name, taken);
      const otherName = other.home ? q(other.name, env) : `${q(other.ns, env)}.${q(other.name, env)}`;
      const cond = l.child === rel ? condition(l, q(name, env), alias, env) : condition(l, alias, q(name, env), env);
      const on = env.upper ? 'ON' : 'on';
      const text = `${otherName} ${alias} ${on} ${cond}`;
      if (seen.has(text)) continue;
      seen.add(text);
      out.push({ label: text, type: 'join', apply: text, detail: l.fk.name, boost: 10 });
    }
  }
  return out;
}

function joinConditions(here: Here, r: Resolver, env: Env): Candidate[] {
  const cat = env.catalog;
  if (!cat) return [];
  const scope = queryOf(here.block).refs.filter((x) => (x.nameToks[0]?.from ?? x.aliasTok?.from ?? 0) < (here.tokens[here.at - 1]?.from ?? 0));
  const last = scope.at(-1);
  const lastRel = last ? r.relation(last) : null;
  const lastName = last ? refName(last) : null;
  if (!last || !lastRel || !lastName) return [];
  const out: Candidate[] = [];
  for (const ref of scope.slice(0, -1)) {
    const rel = r.relation(ref);
    const name = refName(ref);
    if (!rel || !name) continue;
    for (const l of cat.linksOf(lastRel)) {
      if (l.child === lastRel && l.parent === rel) out.push(cond(condition(l, q(lastName, env), q(name, env), env), l));
      else if (l.parent === lastRel && l.child === rel) out.push(cond(condition(l, q(name, env), q(lastName, env), env), l));
    }
  }
  return out;
  function cond(text: string, l: Link): Candidate {
    return { label: text, type: 'join', apply: text, detail: l.fk.name, boost: 10 };
  }
}

function afterRelation(here: Here, join: boolean): Candidate[] {
  const out = keywords(join ? ['on', 'using', ...AFTER_RELATION.filter((k) => k !== 'on')] : AFTER_RELATION, join ? 0 : -1);
  const scope = queryOf(here.block).refs;
  const last = scope.at(-1);
  const prev = here.tokens[here.at - 1];
  if (last && !last.alias && last.nameToks.at(-1) === prev && last.kind === 'relation') {
    const taken = new Set(scope.map((x) => (refName(x) ?? '').toLowerCase()));
    out.push({ label: aliasFor(last.parts.at(-1) ?? 't', taken), type: 'alias', boost: 2 });
  }
  return out;
}

function columnsOfRef(ref: Ref, r: Resolver, env: Env, boost: number, qualify: boolean): Candidate[] {
  const name = refName(ref);
  const cols = r.columns(ref);
  return cols.list.map((c, i) => ({
    label: c.name,
    type: 'column',
    apply: qualify && name ? `${q(name, env)}.${q(c.name, env)}` : q(c.name, env),
    detail: [qualify ? name : null, c.type].filter(Boolean).join(' · ') || undefined,
    boost: boost - Math.min(i, 40) / 100,
    ...(cols.rel ? { source: cols.rel } : {}),
  }));
}

function qualified(chain: string[], here: Here, r: Resolver, env: Env): Candidate[] {
  const cat = env.catalog;
  const name = chain.at(-1) ?? '';
  const out: Candidate[] = [];
  if (chain.length === 1) {
    for (const { ref } of visibleRefs(here.block)) {
      if ((ref.alias ?? ref.parts.at(-1) ?? '').toLowerCase() === name.toLowerCase()) out.push(...columnsOfRef(ref, r, env, 6, false));
    }
    if (out.length) return out;
    const cte = r.cte(name);
    if (cte) return r.cteColumns(cte).list.map((c) => ({ label: c.name, type: 'column', apply: q(c.name, env), detail: c.type ?? undefined, boost: 5 }));
  }
  if (!cat) return out;
  const ns = chain.length === 2 && cat.isDatabase(chain[0] ?? '') ? cat.namespace(name) : chain.length === 1 ? cat.namespace(name) : undefined;
  if (ns) {
    for (const rel of cat.relations()) if (rel.ns === ns.name) out.push(relEntry(rel, env, 2, true));
    for (const f of ns.functions) out.push({ label: f.name, type: 'function', apply: q(f.name, env), detail: f.signature, boost: 0, call: true });
    return out;
  }
  const rel = cat.find(chain)[0];
  if (rel) return cat.columns(rel).map((c) => ({ label: c.name, type: 'column', apply: q(c.name, env), detail: c.type_name, boost: 4, source: rel }));
  if (chain.length === 1 && cat.isDatabase(name)) {
    if (env.dialect === 'mysql') return cat.relations().map((x) => relEntry(x, env, 2, true));
    return cat.namespaceNames().map((n) => ({ label: n, type: 'schema', apply: `${q(n, env)}.`, boost: 2, chain: true }));
  }
  return out;
}

function expression(here: Here, r: Resolver, env: Env, clause: string): Candidate[] {
  const out: Candidate[] = [];
  const visible = visibleRefs(here.block);
  const near = visible.filter((v) => v.near);
  const counts = new Map<string, number>();
  for (const { ref } of near) for (const c of r.columns(ref).list) counts.set(c.name.toLowerCase(), (counts.get(c.name.toLowerCase()) ?? 0) + 1);
  for (const { ref, near: isNear } of visible) {
    const name = refName(ref);
    const resolved = r.columns(ref);
    const cols = resolved.list;
    cols.forEach((c, i) => {
      const clash = (counts.get(c.name.toLowerCase()) ?? 0) > 1 || !isNear;
      out.push({
        label: c.name,
        type: 'column',
        apply: clash && name ? `${q(name, env)}.${q(c.name, env)}` : q(c.name, env),
        detail: [name, c.type].filter(Boolean).join(' · ') || undefined,
        boost: (isNear ? 5 : 2) - Math.min(i, 40) / 100,
        ...(resolved.rel ? { source: resolved.rel } : {}),
      });
    });
    if (name) out.push({ label: name, type: 'alias', apply: `${q(name, env)}.`, detail: ref.alias && ref.parts.length ? ref.parts.join('.') : undefined, boost: 3, chain: true });
  }
  if (clause === 'by' || clause === 'having' || clause === 'order' || clause === 'group') {
    for (const item of queryOf(here.block).items) if (item.name && !counts.has(item.name.toLowerCase())) out.push({ label: item.name, type: 'column', apply: q(item.name, env), boost: 6 });
  }
  if (!visible.length && env.catalog && here.prefix) {
    for (const rel of env.catalog.relations()) out.push({ ...relEntry(rel, env, -4, false), apply: `${relEntry(rel, env, 0, false).apply}.`, chain: true });
  }
  for (const f of env.catalog?.functions() ?? []) out.push({ label: f.name, type: 'function', apply: f.home ? q(f.name, env) : `${q(f.ns, env)}.${q(f.name, env)}`, detail: f.signature, boost: 0, call: true });
  for (const f of functionsOf(env.engine)) out.push({ label: f, type: 'function', boost: -1, call: true });
  if (here.prefix) out.push(...keywords(EXPRESSION, -2));
  return out;
}

function types(env: Env): Candidate[] {
  return typesOf(env.dialect).map((t) => ({ label: t, type: 'type', boost: 0 }));
}

function databases(env: Env): Candidate[] {
  return (env.catalog?.databases ?? []).map((d) => ({ label: d, type: 'database', apply: q(d, env), boost: 0 }));
}

function finish(options: Candidate[], env: Env, here: Here): Candidate[] {
  const typed = here.prefix.replace(/[^A-Za-z]/g, '');
  const upper = typed ? typed === typed.toUpperCase() && typed !== typed.toLowerCase() : env.upper;
  const seen = new Set<string>();
  const out: Candidate[] = [];
  for (const o of options) {
    let c = o;
    const keepsCase = env.engine === 'clickhouse' && o.type !== 'keyword';
    if (!keepsCase && (o.type === 'keyword' || (o.type === 'function' && !o.apply) || o.type === 'type')) c = { ...o, label: upper ? o.label.toUpperCase() : o.label.toLowerCase() };
    if (here.quoted) {
      if (c.type === 'keyword' || c.type === 'join') continue;
      c = { ...c, apply: c.label + (here.quoted.open ? here.close : ''), chain: false, call: false };
    }
    const id = `${c.type}\u0000${c.label}\u0000${c.apply ?? ''}`;
    if (seen.has(id)) continue;
    seen.add(id);
    const extra = env.recency?.(c) ?? 0;
    out.push(extra ? { ...c, boost: c.boost + extra } : c);
  }
  return out;
}
