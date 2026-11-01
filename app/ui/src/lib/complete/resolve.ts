import type { Catalog, RelEntry } from './catalog';
import { queryOf, type Block, type Cte, type Parsed, type Ref } from './parse';

export interface Col {
  name: string;
  type: string | null;
}

export interface Cols {
  list: Col[];
  complete: boolean;
  rel: RelEntry | null;
}

const UNKNOWN: Cols = { list: [], complete: false, rel: null };
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export class Resolver {
  private seen = new Set<Block | Cte>();
  private memo = new Map<Ref, Cols>();

  constructor(
    readonly parsed: Parsed,
    readonly catalog: Catalog | null,
  ) {}

  cte(name: string): Cte | undefined {
    return this.parsed.ctes.find((c) => same(c.name, name));
  }

  relation(ref: Ref): RelEntry | null {
    if (ref.kind !== 'relation' || !this.catalog) return null;
    if (ref.parts.length === 1 && this.cte(ref.parts[0] ?? '')) return null;
    return this.catalog.find(ref.parts)[0] ?? null;
  }

  columns(ref: Ref): Cols {
    const hit = this.memo.get(ref);
    if (hit) return hit;
    const out = this.compute(ref);
    this.memo.set(ref, out);
    return out;
  }

  private compute(ref: Ref): Cols {
    if (ref.columns) return { list: ref.columns.map((name) => ({ name, type: null })), complete: true, rel: null };
    if (ref.kind === 'subquery' && ref.sub) return this.select(ref.sub);
    if (ref.kind !== 'relation') return UNKNOWN;
    const cte = ref.parts.length === 1 ? this.cte(ref.parts[0] ?? '') : undefined;
    if (cte) return this.cteColumns(cte);
    const rel = this.relation(ref);
    if (!rel || !this.catalog) return UNKNOWN;
    return { list: this.catalog.columns(rel).map((c) => ({ name: c.name, type: c.type_name })), complete: true, rel };
  }

  cteColumns(cte: Cte): Cols {
    if (cte.columns) return { list: cte.columns.map((name) => ({ name, type: null })), complete: true, rel: null };
    if (!cte.body || this.seen.has(cte)) return UNKNOWN;
    this.seen.add(cte);
    const out = this.select(cte.body);
    this.seen.delete(cte);
    return out;
  }

  select(b: Block): Cols {
    const q = queryOf(b);
    if (this.seen.has(q)) return UNKNOWN;
    this.seen.add(q);
    const list: Col[] = [];
    let complete = q.items.length > 0;
    for (const item of q.items) {
      if (item.name) list.push({ name: item.name, type: null });
      else if (item.star) {
        const star = item.star;
        const refs = star.length ? q.refs.filter((r) => matchesRef(r, star.at(-1) ?? '')) : q.refs;
        for (const r of refs) {
          const c = this.columns(r);
          list.push(...c.list);
          complete &&= c.complete;
        }
        if (!refs.length) complete = false;
      } else complete = false;
    }
    this.seen.delete(q);
    return { list, complete, rel: null };
  }
}

export function matchesRef(r: Ref, name: string): boolean {
  if (r.alias) return same(r.alias, name);
  const last = r.parts.at(-1);
  return !!last && same(last, name);
}
