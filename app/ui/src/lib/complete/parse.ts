import { CLAUSE_WORDS } from './dialects';
import { isName, lower, type Token } from './scan';

export interface Ref {
  kind: 'relation' | 'subquery' | 'function';
  parts: string[];
  nameToks: Token[];
  alias: string | null;
  aliasTok: Token | null;
  columns: string[] | null;
  sub: Block | null;
  block: Block;
}

export interface SelectItem {
  name: string | null;
  alias: boolean;
  star: string[] | null;
}

export interface Cte {
  name: string;
  tok: Token;
  columns: string[] | null;
  body: Block | null;
}

export interface Block {
  id: number;
  parent: Block | null;
  open: number;
  close: number;
  head: string;
  query: boolean;
  refs: Ref[];
  items: SelectItem[];
  ctes: Cte[];
  columnsOf: Ref | null;
  entries: Entry[];
}

export type Entry = { t: Token; i: number } | { b: Block; i: number };

export interface Parsed {
  tokens: Token[];
  root: Block;
  blocks: Block[];
  blockAt: number[];
  ctes: Cte[];
}

const QUERY_HEADS = new Set(['select', 'with', 'values', 'table', 'from']);
const SELECT_END = new Set(['from', 'into', 'where', 'group', 'order', 'having', 'limit', 'union', 'intersect', 'except', 'window', 'fetch', 'offset', 'qualify']);
const NOT_ALIAS = new Set([...CLAUSE_WORDS, 'left', 'right', 'inner', 'outer', 'full', 'cross', 'natural', 'straight_join', 'partition', 'semi', 'anti', 'asof', 'positional']);

const tokOf = (e: Entry | undefined): Token | undefined => (e && 't' in e ? e.t : undefined);
const blockOf = (e: Entry | undefined): Block | undefined => (e && 'b' in e ? e.b : undefined);
const wordOf = (e: Entry | undefined) => lower(tokOf(e));
const isPunct = (e: Entry | undefined, c: string) => {
  const t = tokOf(e);
  return !!t && t.k === 'punct' && t.text === c;
};

export function parse(tokens: Token[]): Parsed {
  const root: Block = { id: 0, parent: null, open: -1, close: tokens.length, head: '', query: true, refs: [], items: [], ctes: [], columnsOf: null, entries: [] };
  const blocks = [root];
  const blockAt: number[] = new Array<number>(tokens.length);
  const stack = [root];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i] as Token;
    const top = stack.at(-1) as Block;
    if (t.k === 'punct' && t.text === '(') {
      blockAt[i] = top.id;
      const child: Block = { id: blocks.length, parent: top, open: i, close: tokens.length, head: '', query: false, refs: [], items: [], ctes: [], columnsOf: null, entries: [] };
      blocks.push(child);
      top.entries.push({ b: child, i });
      stack.push(child);
    } else if (t.k === 'punct' && t.text === ')' && stack.length > 1) {
      top.close = i;
      stack.pop();
      blockAt[i] = (stack.at(-1) as Block).id;
    } else {
      blockAt[i] = top.id;
      top.entries.push({ t, i });
    }
  }
  for (const b of blocks) {
    b.head = wordOf(b.entries[0]);
    b.query = b === root || QUERY_HEADS.has(b.head);
  }
  const ctes: Cte[] = [];
  for (const b of blocks) parseBlock(b, ctes);
  return { tokens, root, blocks, blockAt, ctes };
}

function parseBlock(b: Block, ctes: Cte[]) {
  const e = b.entries;
  let j = 0;
  if (b.head === 'with') j = parseCtes(b, 1, ctes);
  while (j < e.length) {
    const w = wordOf(e[j]);
    const prev = wordOf(e[j - 1]);
    if (!b.query) j++;
    else if (w === 'select') j = parseSelect(b, j + 1);
    else if (w === 'from' && prev === 'distinct') j++;
    else if (w === 'from') j = parseRefs(b, j + 1, true, false);
    else if (w === 'join' || w === 'apply' || w === 'straight_join') j = parseRefs(b, j + 1, false, false);
    else if (w === 'into') j = parseRefs(b, j + 1, false, prev === 'insert' || prev === 'ignore' || b.head === 'insert' || b.head === 'replace');
    else if (w === 'update' && prev !== 'for' && prev !== 'on' && prev !== 'key' && prev !== 'do') j = parseRefs(b, j + 1, false, false);
    else if (w === 'using' && (b.head === 'merge' || b.head === 'delete')) j = parseRefs(b, j + 1, true, false);
    else if (w === 'table' && (prev === 'truncate' || b.head === 'table')) j = parseRefs(b, j + 1, false, false);
    else j++;
  }
}

function parseCtes(b: Block, j: number, all: Cte[]): number {
  const e = b.entries;
  if (wordOf(e[j]) === 'recursive') j++;
  for (;;) {
    const tok = tokOf(e[j]);
    if (!isName(tok)) return j;
    j++;
    let columns: string[] | null = null;
    const list = blockOf(e[j]);
    if (list && wordOf(e[j + 1]) === 'as') {
      columns = namesIn(list);
      j++;
    }
    if (wordOf(e[j]) !== 'as') return j;
    j++;
    if (wordOf(e[j]) === 'not') j++;
    if (wordOf(e[j]) === 'materialized') j++;
    const body = blockOf(e[j]) ?? null;
    if (body) j++;
    const cte = { name: tok.text, tok, columns, body };
    b.ctes.push(cte);
    all.push(cte);
    if (!isPunct(e[j], ',')) return j;
    j++;
  }
}

function namesIn(b: Block): string[] {
  return b.entries.flatMap((x) => {
    const t = tokOf(x);
    return isName(t) ? [t.text] : [];
  });
}

function parseRefs(b: Block, j: number, list: boolean, into: boolean): number {
  for (;;) {
    const next = parseRef(b, j, into);
    if (next === null) return j;
    j = next;
    if (!list || !isPunct(b.entries[j], ',')) return j;
    j++;
  }
}

function parseRef(b: Block, j: number, into: boolean): number | null {
  const e = b.entries;
  while (['lateral', 'only', 'ignore'].includes(wordOf(e[j]))) j++;
  const sub = blockOf(e[j]);
  const ref: Ref = { kind: 'relation', parts: [], nameToks: [], alias: null, aliasTok: null, columns: null, sub: null, block: b };
  const literal = tokOf(e[j]);
  if (sub) {
    ref.kind = 'subquery';
    ref.sub = sub;
    j++;
  } else if (literal?.k === 'string' || literal?.k === 'param') {
    ref.kind = 'function';
    j++;
  } else {
    const first = tokOf(e[j]);
    if (!isName(first) || (first.k === 'word' && NOT_ALIAS.has(first.text.toLowerCase()))) return null;
    ref.parts.push(first.text);
    ref.nameToks.push(first);
    j++;
    while (isPunct(e[j], '.')) {
      const t = tokOf(e[j + 1]);
      if (!isName(t)) {
        j++;
        break;
      }
      ref.parts.push(t.text);
      ref.nameToks.push(t);
      j += 2;
    }
    const args = blockOf(e[j]);
    if (args && into) {
      args.columnsOf = ref;
      j++;
    } else if (args) {
      ref.kind = 'function';
      j++;
    }
  }
  b.refs.push(ref);
  if (into) return j;
  if (wordOf(e[j]) === 'as') j++;
  const alias = tokOf(e[j]);
  if (isName(alias) && (alias.k === 'qident' || !NOT_ALIAS.has(alias.text.toLowerCase()))) {
    ref.alias = alias.text;
    ref.aliasTok = alias;
    j++;
    const cols = blockOf(e[j]);
    if (cols && !cols.query) {
      ref.columns = namesIn(cols);
      j++;
    }
  }
  if (wordOf(e[j]) === 'with' && blockOf(e[j + 1])) j += 2;
  return j;
}

function parseSelect(b: Block, j: number): number {
  const e = b.entries;
  while (['distinct', 'all'].includes(wordOf(e[j]))) j++;
  if (wordOf(e[j]) === 'on' && blockOf(e[j + 1])) j += 2;
  if (wordOf(e[j]) === 'top') {
    j += 2;
    if (wordOf(e[j]) === 'percent') j++;
    if (wordOf(e[j]) === 'with' && wordOf(e[j + 1]) === 'ties') j += 2;
  }
  let item: Entry[] = [];
  const flush = () => {
    if (item.length) b.items.push(itemOf(item));
    item = [];
  };
  while (j < e.length && !SELECT_END.has(wordOf(e[j]))) {
    if (isPunct(e[j], ',')) flush();
    else item.push(e[j] as Entry);
    j++;
  }
  flush();
  return j;
}

function itemOf(item: Entry[]): SelectItem {
  const last = tokOf(item.at(-1));
  const prev = item.at(-2);
  if (last && last.k === 'op' && last.text === '*') {
    if (item.length === 1) return { name: null, alias: false, star: [] };
    if (isPunct(prev, '.')) return { name: null, alias: false, star: item.slice(0, -2).flatMap((x) => (isName(tokOf(x)) ? [(tokOf(x) as Token).text] : [])) };
  }
  const first = tokOf(item[0]);
  const second = tokOf(item[1]);
  if (isName(first) && second?.k === 'op' && second.text === '=') return { name: first.text, alias: true, star: null };
  if (!isName(last)) return { name: null, alias: false, star: null };
  if (last.k === 'word' && ['end', 'null', 'true', 'false'].includes(last.text.toLowerCase()) && item.length > 1 && wordOf(prev) !== 'as') return { name: null, alias: false, star: null };
  if (item.length === 1 || isPunct(prev, '.')) return { name: last.text, alias: false, star: null };
  if (wordOf(prev) === 'as' || blockOf(prev) || isName(tokOf(prev)) || ['number', 'string', 'param'].includes(tokOf(prev)?.k ?? '')) return { name: last.text, alias: true, star: null };
  return { name: null, alias: false, star: null };
}

export function blockAtPos(p: Parsed, pos: number): { block: Block; index: number } {
  let index = 0;
  while (index < p.tokens.length && (p.tokens[index] as Token).to <= pos) index++;
  const inside = p.tokens[index];
  if (inside && inside.from < pos) return { block: p.blocks[p.blockAt[index] ?? 0] ?? p.root, index };
  const prev = p.tokens[index - 1];
  if (!prev) return { block: p.root, index };
  if (prev.k === 'punct' && prev.text === '(') {
    const child = p.blocks.find((b) => b.open === index - 1);
    if (child) return { block: child, index };
  }
  return { block: p.blocks[p.blockAt[index - 1] ?? 0] ?? p.root, index };
}

export function queryOf(b: Block): Block {
  let x: Block | null = b;
  while (x && !x.query) x = x.parent;
  return x ?? b;
}

export function visibleRefs(b: Block): { ref: Ref; near: boolean }[] {
  const out: { ref: Ref; near: boolean }[] = [];
  let q: Block | null = queryOf(b);
  let near = true;
  while (q) {
    for (const ref of q.refs) out.push({ ref, near });
    near = false;
    q = q.parent ? queryOf(q.parent) : null;
  }
  return out;
}

export const refName = (r: Ref) => r.alias ?? r.parts.at(-1) ?? null;
