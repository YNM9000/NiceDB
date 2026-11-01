export interface Keyed<T> {
  column: T;
  descending: boolean;
}

export type SortDir = 'asc' | 'desc' | 'none';

export function nextSort<T>(order: readonly Keyed<T>[], column: T, add: boolean, dir?: SortDir): Keyed<T>[] {
  const at = order.findIndex((o) => o.column === column);
  const cur = order[at];
  const want: SortDir = dir ?? (!cur ? 'asc' : !cur.descending ? 'desc' : 'none');
  const next = want === 'none' ? null : { column, descending: want === 'desc' };
  if (!add) return next ? [next] : [];
  if (!next) return order.filter((o) => o.column !== column);
  return at >= 0 ? order.map((o) => (o.column === column ? next : o)) : [...order, next];
}
