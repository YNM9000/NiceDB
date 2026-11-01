import type { Change_Deserialize as Change, ChangeSet_Deserialize as ChangeSet, Field_Deserialize as Field, TableRef } from './bindings';

export type Cell = string | null;
export type Row = Cell[];

export interface Shape {
  table: TableRef;
  columns: string[];
  types?: string[];
  key: number[];
}

export type Target = { kind: 'update'; key: string; columns: string[] } | { kind: 'insert'; id: number } | { kind: 'delete'; key: string };

export type Mark = { kind: 'conflict' } | { kind: 'error'; message: string };

interface Edit {
  seq: number;
  original: Row;
  cells: Map<number, Cell>;
}

interface NewRow {
  seq: number;
  id: number;
  values: (Cell | undefined)[];
}

interface Deleted {
  seq: number;
  original: Row;
  edit: Edit | undefined;
}

type Step =
  | { kind: 'cell'; key: string; col: number; before: Cell | undefined; after: Cell; edit: Edit }
  | { kind: 'new-cell'; id: number; col: number; before: Cell | undefined; after: Cell }
  | { kind: 'insert'; id: number }
  | { kind: 'delete'; key: string; gone: Deleted }
  | { kind: 'drop-new'; row: NewRow; at: number };

interface Undone {
  step: Step;
  row?: NewRow;
  at?: number;
}

const markKey = (t: Target) => (t.kind === 'insert' ? `new:${t.id}` : t.key);

export class Pending {
  private edits = new Map<string, Edit>();
  private deleted = new Map<string, Deleted>();
  private inserted: NewRow[] = [];
  private steps: Step[] = [];
  private undone: Undone[] = [];
  private marks = new Map<string, Mark>();
  private seq = 0;
  private ids = 0;

  constructor(readonly shape: Shape) {}

  keyOf(row: Row): string {
    return JSON.stringify(this.shape.key.map((i) => row[i] ?? null));
  }

  get empty() {
    return this.edits.size === 0 && this.deleted.size === 0 && this.inserted.length === 0;
  }

  get counts() {
    let cells = 0;
    for (const e of this.edits.values()) cells += e.cells.size;
    return { cells, rows: this.edits.size, inserted: this.inserted.length, deleted: this.deleted.size, total: this.edits.size + this.inserted.length + this.deleted.size };
  }

  get newRows(): readonly NewRow[] {
    return this.inserted;
  }

  set(row: Row, col: number, value: Cell) {
    const key = this.keyOf(row);
    if (this.deleted.has(key)) return;
    let edit = this.edits.get(key);
    const before = edit?.cells.get(col);
    if ((before === undefined ? row[col] : before) === value) return;
    edit ??= { seq: ++this.seq, original: [...row], cells: new Map() };
    this.write(key, edit, col, value);
    this.marks.delete(key);
    this.did({ kind: 'cell', key, col, before, after: value, edit });
  }

  private did(step: Step) {
    this.steps.push(step);
    this.undone = [];
  }

  private write(key: string, edit: Edit, col: number, value: Cell | undefined) {
    if (value === undefined || value === edit.original[col]) edit.cells.delete(col);
    else edit.cells.set(col, value);
    if (edit.cells.size === 0) this.edits.delete(key);
    else if (!this.edits.has(key)) this.edits.set(key, edit);
  }

  setNew(id: number, col: number, value: Cell) {
    const row = this.inserted.find((r) => r.id === id);
    if (!row || row.values[col] === value) return;
    this.did({ kind: 'new-cell', id, col, before: row.values[col], after: value });
    row.values[col] = value;
    this.marks.delete(`new:${id}`);
  }

  insert(): number {
    const id = ++this.ids;
    this.inserted.unshift({ seq: ++this.seq, id, values: this.shape.columns.map(() => undefined) });
    this.did({ kind: 'insert', id });
    return id;
  }

  toggleDelete(row: Row) {
    const key = this.keyOf(row);
    if (this.restore(key)) return;
    const edit = this.edits.get(key);
    this.edits.delete(key);
    const gone: Deleted = { seq: ++this.seq, original: [...row], edit };
    this.deleted.set(key, gone);
    this.marks.delete(key);
    this.did({ kind: 'delete', key, gone });
  }

  private restore(key: string): boolean {
    const d = this.deleted.get(key);
    if (!d) return false;
    this.deleted.delete(key);
    if (d.edit) this.edits.set(key, d.edit);
    this.marks.delete(key);
    return true;
  }

  dropNew(id: number) {
    const at = this.inserted.findIndex((r) => r.id === id);
    const row = this.inserted[at];
    if (!row) return;
    this.inserted.splice(at, 1);
    this.marks.delete(`new:${id}`);
    this.did({ kind: 'drop-new', row, at });
  }

  revert(t: Target) {
    if (t.kind === 'update') this.edits.delete(t.key);
    else if (t.kind === 'delete') {
      const d = this.deleted.get(t.key);
      this.deleted.delete(t.key);
      if (d?.edit) this.edits.set(t.key, d.edit);
    } else this.inserted = this.inserted.filter((r) => r.id !== t.id);
    this.marks.delete(markKey(t));
    this.undone = [];
    this.steps = this.steps.filter((s) => (t.kind === 'insert' ? !('id' in s && s.id === t.id) && !(s.kind === 'drop-new' && s.row.id === t.id) : !('key' in s && s.key === t.key)));
  }

  get canRedo() {
    return this.undone.length > 0;
  }

  undo(): boolean {
    for (;;) {
      const step = this.steps.pop();
      if (!step) return false;
      const done = this.undoStep(step);
      if (done) {
        this.undone.push(done);
        return true;
      }
    }
  }

  redo(): boolean {
    for (;;) {
      const u = this.undone.pop();
      if (!u) return false;
      if (this.redoStep(u)) {
        this.steps.push(u.step);
        return true;
      }
    }
  }

  private undoStep(step: Step): Undone | null {
    switch (step.kind) {
      case 'cell': {
        const current = this.edits.get(step.key);
        if (this.deleted.has(step.key) || (current && current !== step.edit)) return null;
        this.write(step.key, step.edit, step.col, step.before);
        return { step };
      }
      case 'new-cell': {
        const row = this.inserted.find((r) => r.id === step.id);
        if (!row) return null;
        row.values[step.col] = step.before;
        return { step };
      }
      case 'insert': {
        const at = this.inserted.findIndex((r) => r.id === step.id);
        const row = this.inserted[at];
        if (!row) return null;
        this.inserted.splice(at, 1);
        return { step, row, at };
      }
      case 'delete':
        return this.restore(step.key) ? { step } : null;
      case 'drop-new':
        this.inserted.splice(Math.min(step.at, this.inserted.length), 0, step.row);
        return { step };
    }
  }

  private redoStep(u: Undone): boolean {
    const step = u.step;
    switch (step.kind) {
      case 'cell': {
        const current = this.edits.get(step.key);
        if (this.deleted.has(step.key) || (current && current !== step.edit)) return false;
        this.write(step.key, step.edit, step.col, step.after);
        this.marks.delete(step.key);
        return true;
      }
      case 'new-cell': {
        const row = this.inserted.find((r) => r.id === step.id);
        if (!row) return false;
        row.values[step.col] = step.after;
        return true;
      }
      case 'insert': {
        if (!u.row || this.inserted.some((r) => r.id === step.id)) return false;
        this.inserted.splice(Math.min(u.at ?? 0, this.inserted.length), 0, u.row);
        return true;
      }
      case 'delete': {
        if (this.deleted.has(step.key)) return false;
        const edit = this.edits.get(step.key);
        this.edits.delete(step.key);
        this.deleted.set(step.key, { ...step.gone, edit });
        this.marks.delete(step.key);
        return true;
      }
      case 'drop-new': {
        const at = this.inserted.findIndex((r) => r.id === step.row.id);
        if (at < 0) return false;
        this.inserted.splice(at, 1);
        this.marks.delete(`new:${step.row.id}`);
        return true;
      }
    }
  }

  cell(row: Row, col: number): { value: Cell; was: Cell } | null {
    const edit = this.edits.get(this.keyOf(row));
    if (!edit?.cells.has(col)) return null;
    return { value: edit.cells.get(col) ?? null, was: edit.original[col] ?? null };
  }

  rowState(row: Row): 'edited' | 'deleted' | null {
    const key = this.keyOf(row);
    if (this.deleted.has(key)) return 'deleted';
    return this.edits.has(key) ? 'edited' : null;
  }

  mark(t: Target): Mark | undefined {
    return this.marks.get(markKey(t));
  }

  markRow(row: Row): Mark | undefined {
    return this.marks.get(this.keyOf(row));
  }

  markNew(id: number): Mark | undefined {
    return this.marks.get(`new:${id}`);
  }

  setMark(t: Target, m: Mark) {
    this.marks.clear();
    this.marks.set(markKey(t), m);
  }

  private fields(cols: Iterable<number>, from: (c: number) => Cell): Field[] {
    return [...cols].map((c) => ({ column: this.shape.columns[c] ?? '', value: from(c) }));
  }

  build(): { set: ChangeSet; targets: Target[] } {
    type Rev = { seq: number; change: Change; target: Target };
    const revs: Rev[] = [];
    const key = this.shape.key;
    for (const [k, e] of this.edits) {
      const cols = [...e.cells.keys()].sort((a, b) => a - b);
      revs.push({
        seq: e.seq,
        change: {
          kind: 'update',
          key: this.fields(key, (c) => e.original[c] ?? null),
          original: this.fields(cols, (c) => e.original[c] ?? null),
          set: this.fields(cols, (c) => e.cells.get(c) ?? null),
        },
        target: { kind: 'update', key: k, columns: cols.map((c) => this.shape.columns[c] ?? '') },
      });
    }
    for (const r of this.inserted) {
      const cols = r.values.flatMap((v, i) => (v === undefined ? [] : [i]));
      revs.push({ seq: r.seq, change: { kind: 'insert', values: this.fields(cols, (c) => r.values[c] ?? null) }, target: { kind: 'insert', id: r.id } });
    }
    for (const [k, d] of this.deleted) {
      const all = this.shape.columns.map((_, i) => i);
      revs.push({
        seq: d.seq,
        change: { kind: 'delete', key: this.fields(key, (c) => d.original[c] ?? null), original: this.fields(all, (c) => d.original[c] ?? null) },
        target: { kind: 'delete', key: k },
      });
    }
    revs.sort((a, b) => a.seq - b.seq);
    const columns = this.shape.columns.map((name, i) => ({ name, type_name: this.shape.types?.[i] ?? '' }));
    return { set: { table: this.shape.table, columns, changes: revs.map((r) => r.change) }, targets: revs.map((r) => r.target) };
  }

  numbers(): Map<string, number> {
    const out = new Map<string, number>();
    this.build().targets.forEach((t, i) => out.set(markKey(t), i + 1));
    return out;
  }
}

export const rowKey = markKey;
