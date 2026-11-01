import type { Dialect } from '../bindings';
import type { Catalog, RelEntry } from './catalog';
import { parse, visibleRefs, type Ref } from './parse';
import { matchesRef, Resolver } from './resolve';
import { isName, scan, statementAt, statements } from './scan';

export interface ColumnHit {
  from: number;
  to: number;
  rel: RelEntry;
  name: string;
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function columnAt(text: string, pos: number, dialect: Dialect, catalog: Catalog | null): ColumnHit | null {
  if (!catalog) return null;
  const st = statementAt(statements(text, dialect, scan(text, dialect)), pos);
  const p = parse(st.tokens);
  const i = p.tokens.findIndex((t) => t.from <= pos && pos <= t.to && isName(t));
  const tok = p.tokens[i];
  if (!tok || !isName(tok)) return null;
  const next = p.tokens[i + 1];
  if (next?.k === 'punct' && (next.text === '(' || next.text === '.')) return null;
  const block = p.blocks[p.blockAt[i] ?? 0] ?? p.root;
  if (block.refs.some((r) => r.nameToks.includes(tok) || r.aliasTok === tok)) return null;
  const r = new Resolver(p, catalog);
  const prev = p.tokens[i - 1];
  let refs: Ref[];
  if (prev?.k === 'punct' && prev.text === '.' && prev.to === tok.from) {
    const chain: string[] = [];
    for (let k = i - 2; k >= 0; k -= 2) {
      const q = p.tokens[k];
      if (!isName(q)) break;
      chain.unshift(q.text);
      const dot = p.tokens[k - 1];
      if (!(dot?.k === 'punct' && dot.text === '.')) break;
    }
    const alias = chain.length === 1 ? visibleRefs(block).find(({ ref }) => matchesRef(ref, chain[0] ?? ''))?.ref : undefined;
    if (alias) refs = [alias];
    else {
      const rel = catalog.find(chain)[0];
      return rel && catalog.hasColumn(rel, tok.text) ? { from: tok.from, to: tok.to, rel, name: tok.text } : null;
    }
  } else refs = block.columnsOf ? [block.columnsOf] : visibleRefs(block).map((v) => v.ref);
  for (const ref of refs) {
    const cols = r.columns(ref);
    if (cols.rel && cols.list.some((c) => same(c.name, tok.text))) return { from: tok.from, to: tok.to, rel: cols.rel, name: tok.text };
  }
  return null;
}
