import type { Column } from './bindings';

export type GridRow = (string | null)[];

export interface GridData {
  columns: Column[];
  rows: GridRow[];
  count: number;
  sql: string;
  rev?: number;
}

export interface CellView {
  value: string | null;
  was?: string | null;
  hint?: string;
}

export interface RowView {
  state: 'edited' | 'new' | 'deleted';
  n: number | null;
  mark: 'conflict' | 'error' | null;
  note: string | null;
}

export interface GridEdit {
  readonly?: boolean;
  cell(r: number, c: number): CellView | null;
  row(r: number): RowView | null;
  applied(r: number, c: number): boolean;
  label(r: number): string;
  start(r: number, c: number): boolean;
  commit(r: number, c: number, value: string | null): boolean;
  setNull(r: number, c: number): void;
  toggleDelete(r0: number, r1: number): void;
  focus(r: number, c: number): void;
}

export interface SortKey {
  column: number;
  descending: boolean;
}

export const CELL_CHARS = 1000;

export const clipCell = (value: string) => (value.length > CELL_CHARS ? value.slice(0, CELL_CHARS) : value);

export const COLUMN_WINDOW_FROM = 40;

export function columnWindow(offsets: readonly number[], left: number, width: number, spare: number): { c0: number; c1: number } {
  const n = offsets.length - 1;
  const at = (x: number) => {
    let lo = 0;
    let hi = n;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if ((offsets[mid + 1] ?? 0) <= x) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  };
  return { c0: Math.min(n, at(left - spare)), c1: Math.min(n, at(left + width + spare) + 1) };
}
