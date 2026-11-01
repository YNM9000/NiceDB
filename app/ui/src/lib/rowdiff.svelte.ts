import type { Column, ColumnPlan_Serialize as ColumnPlan, RowDiff_Serialize as RowDiff, RowState } from './bindings';
import type { GridData, GridEdit, GridRow } from './grid';
import { diffEdit, NO_ROWS, pickIndex } from './rowdiff';

export class DiffView implements GridData {
  readonly columns: Column[];
  readonly rows: GridRow[] = [];
  readonly index: number[] = [];
  readonly sql = '';
  readonly edit: GridEdit;
  count = $state(0);
  rev = $state(0);

  constructor(columns: Column[], all: readonly RowDiff[]) {
    this.columns = columns;
    this.edit = diffEdit(all, this.index);
  }

  take(all: readonly RowDiff[], picked: readonly number[]) {
    for (const i of picked) {
      this.index.push(i);
      this.rows.push((all[i] as RowDiff).cells);
    }
    this.count = this.rows.length;
    this.rev++;
  }
}

export class DiffSet {
  readonly all: RowDiff[] = [];
  plan = $state.raw<ColumnPlan | null>(null);
  show = $state<Record<RowState, boolean>>({ added: true, removed: true, changed: true, same: false });
  got = $state<Record<RowState, number>>({ ...NO_ROWS });
  view = $state.raw<DiffView | null>(null);

  setPlan(plan: ColumnPlan) {
    this.plan = plan;
    this.rebuild();
  }

  add(rows: readonly RowDiff[]) {
    const from = this.all.length;
    for (const r of rows) {
      this.all.push(r);
      this.got[r.state]++;
    }
    this.view?.take(this.all, pickIndex(this.all, this.show, from));
  }

  toggle(state: RowState) {
    this.show[state] = !this.show[state];
    this.rebuild();
  }

  private rebuild() {
    const plan = this.plan;
    if (!plan) return;
    const columns = plan.common.map((p) => ({ name: p.name, type_name: p.a_type ?? p.b_type }));
    const view = new DiffView(columns, this.all);
    view.take(this.all, pickIndex(this.all, this.show));
    this.view = view;
  }
}
