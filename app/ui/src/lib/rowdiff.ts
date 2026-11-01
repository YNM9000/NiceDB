import type { ColumnPlan_Serialize as ColumnPlan, RowDiff_Serialize as RowDiff, RowState, Schema_Serialize as Schema } from './bindings';
import { tablePartsFromSql } from './copy';
import type { CellView, GridEdit, RowView } from './grid';

export type SourceMode = 'query' | 'table';

export const STATES: readonly RowState[] = ['added', 'removed', 'changed', 'same'];

export const NO_ROWS: Record<RowState, number> = { added: 0, removed: 0, changed: 0, same: 0 };

export function tableOfSql(sql: string): { namespace: string | null; name: string } | null {
  const parts = tablePartsFromSql(sql);
  if (!parts) return null;
  const name = parts.at(-1) ?? '';
  return { namespace: parts.length > 1 ? (parts.at(-2) ?? null) : null, name };
}

export function primaryKey(schema: Schema | null | undefined, namespace: string | null, name: string): string[] {
  const spaces = schema?.namespaces ?? [];
  const ordered = namespace ? spaces.filter((n) => n.name === namespace) : spaces;
  for (const ns of ordered) {
    const t = ns.tables.find((x) => x.name === name) ?? ns.tables.find((x) => x.name.toLowerCase() === name.toLowerCase());
    if (t) return t.columns.filter((c) => c.primary_key).map((c) => c.name);
  }
  return [];
}

export function proposeKey(common: readonly string[], pk: readonly string[]): string[] {
  const lower = new Map(common.map((c) => [c.toLowerCase(), c]));
  const mapped = pk.map((k) => lower.get(k.toLowerCase()));
  if (pk.length && mapped.every((k) => k !== undefined)) return mapped as string[];
  const id = lower.get('id');
  return id ? [id] : [];
}

export function commonColumns(a: readonly { name: string }[], b: readonly { name: string }[]): string[] {
  const taken = new Set<number>();
  const out: string[] = [];
  for (const ca of a) {
    let i = b.findIndex((cb, j) => !taken.has(j) && cb.name === ca.name);
    if (i < 0) i = b.findIndex((cb, j) => !taken.has(j) && cb.name.toLowerCase() === ca.name.toLowerCase());
    if (i >= 0) {
      taken.add(i);
      out.push(ca.name);
    }
  }
  return out;
}

const VIEW: Record<RowState, RowView['state'] | null> = { added: 'new', removed: 'deleted', changed: 'edited', same: null };

export function diffEdit(rows: readonly RowDiff[], index: readonly number[]): GridEdit {
  const at = (r: number) => rows[index[r] ?? -1];
  return {
    readonly: true,
    cell(r, c): CellView | null {
      const d = at(r);
      const change = d?.state === 'changed' ? d.changes.find((x) => x.column === c) : undefined;
      return change && d ? { value: d.cells[c] ?? null, was: change.was } : null;
    },
    row(r): RowView | null {
      const d = at(r);
      const state = d ? VIEW[d.state] : null;
      return state ? { state, n: null, mark: null, note: null } : null;
    },
    applied: () => false,
    label: (r) => String(r + 1),
    start: () => false,
    commit: () => false,
    setNull: () => {},
    toggleDelete: () => {},
    focus: () => {},
  };
}

export function pickIndex(rows: readonly RowDiff[], show: Record<RowState, boolean>, from = 0): number[] {
  const out: number[] = [];
  for (let i = from; i < rows.length; i++) if (show[(rows[i] as RowDiff).state]) out.push(i);
  return out;
}

export function columnNotes(plan: ColumnPlan | null) {
  return {
    onlyA: plan?.only_a.map((c) => c.name) ?? [],
    onlyB: plan?.only_b.map((c) => c.name) ?? [],
    retyped: plan?.common.filter((p) => p.type_differs).map((p) => ({ name: p.name, a: p.a_type ?? '?', b: p.b_type ?? '?' })) ?? [],
  };
}

export const EDITOR_STATEMENTS = 20_000;
