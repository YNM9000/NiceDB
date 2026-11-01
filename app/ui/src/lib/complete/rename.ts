import type { Dialect } from '../bindings';
import { parse, type Ref } from './parse';
import { isName, scan, statementAt, statements, type Token } from './scan';

export interface Occurrences {
  name: string;
  kind: 'alias' | 'cte';
  ranges: [number, number][];
  main: number;
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function aliasOccurrences(text: string, pos: number, dialect: Dialect): Occurrences | null {
  const st = statementAt(statements(text, dialect, scan(text, dialect)), pos);
  const tokens = st.tokens;
  const here = tokens.find((t) => isName(t) && t.from <= pos && pos <= t.to);
  if (!here) return null;
  const parsed = parse(tokens);
  const refs = parsed.blocks.flatMap((b) => b.refs);
  const dotAfter = (t: Token) => {
    const i = tokens.indexOf(t);
    const next = tokens[i + 1];
    const before = tokens[i - 1];
    return next?.k === 'punct' && next.text === '.' && !(before?.k === 'punct' && before.text === '.');
  };
  const name = here.text;
  const aliased = refs.filter((r) => r.alias && same(r.alias, name));
  const cte = parsed.ctes.find((c) => same(c.name, name));
  let toks: Token[] = [];
  let kind: Occurrences['kind'] = 'alias';
  if (aliased.some((r) => r.aliasTok === here) || (aliased.length && dotAfter(here))) {
    toks = [...aliased.map((r) => r.aliasTok as Token), ...tokens.filter((t) => isName(t) && same(t.text, name) && dotAfter(t) && !aliased.some((r) => r.nameToks.includes(t)))];
  } else if (cte && (cte.tok === here || refs.some((r) => isCteUse(r, name) && r.nameToks.includes(here)) || dotAfter(here))) {
    kind = 'cte';
    const uses = refs.filter((r) => isCteUse(r, name));
    const bare = uses.some((r) => !r.alias);
    toks = [cte.tok, ...uses.flatMap((r) => r.nameToks), ...(bare ? tokens.filter((t) => isName(t) && same(t.text, name) && dotAfter(t)) : [])];
  } else return null;
  const unique = [...new Set(toks)].sort((a, b) => a.from - b.from);
  const main = Math.max(0, unique.indexOf(here));
  return { name, kind, ranges: unique.map((t) => [t.from, t.to]), main };
}

const isCteUse = (r: Ref, name: string) => r.kind === 'relation' && r.parts.length === 1 && same(r.parts[0] ?? '', name);
