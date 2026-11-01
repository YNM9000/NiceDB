import { m } from '../paraglide/messages';
import { api, driverMessage, type ChangePreview, type Page, type Schema, type SessionInfo } from './api';
import type { Browse_Deserialize as Browse, ColumnInfo, Order, QuickFilter, Relation_Serialize as Relation } from './bindings';
import { Pending, rowKey, type Target } from './changeset';
import { changeRows } from './estimate';
import type { Crumb } from './fk';
import { askConfirm, settle, writesOpen } from './guard.svelte';
import { play } from './sound';
import { refreshTx } from './tx.svelte';

export const PAGE = 500;

export interface TableInfo {
  namespace: string;
  name: string;
  view: boolean;
  columns: ColumnInfo[];
  key: number[];
  estimated: number | null;
}

export function tableInfo(namespace: string, rel: Relation, view: boolean, rowid = false): TableInfo {
  const at = (name: string) => rel.columns.findIndex((c) => c.name === name);
  let key = rel.columns.flatMap((c, i) => (c.primary_key ? [i] : []));
  if (!key.length) {
    const unique = rel.indexes.find((ix) => ix.unique && ix.columns.length > 0 && ix.columns.every((n) => rel.columns[at(n)]?.nullable === false));
    key = unique ? unique.columns.map(at) : [];
  }
  if (rowid && !view && !key.length) {
    const columns = [...rel.columns, { name: 'ROWID', type_name: 'ROWID', nullable: false, primary_key: false }];
    return { namespace, name: rel.name, view, columns, key: [rel.columns.length], estimated: rel.estimated_rows ?? null };
  }
  return { namespace, name: rel.name, view, columns: rel.columns, key: view ? [] : key, estimated: rel.estimated_rows ?? null };
}

export function findTable(schema: Schema | null, namespace: string, name: string, rowid = false): TableInfo | null {
  const ns = schema?.namespaces.find((n) => n.name === namespace);
  const table = ns?.tables.find((r) => r.name === name);
  if (table) return tableInfo(namespace, table, false, rowid);
  const view = [...(ns?.views ?? []), ...(ns?.materialized_views ?? []), ...(ns?.dictionaries ?? []), ...(ns?.aliases ?? []), ...(ns?.data_streams ?? [])].find((r) => r.name === name);
  return view ? tableInfo(namespace, view, true) : null;
}

export const isBinaryType = (t: string) => /^(bytea|(tiny|medium|long)?blob|(var)?binary)\b/i.test(t.trim());

export const isHex = (v: string) => /^(\\x|0[xX])([0-9a-fA-F]{2})*$/.test(v);

export function readOnlyReason(t: TableInfo, engine: string | null = null): string | null {
  if (engine) return m.data_ro_engine({ engine });
  if (t.view) return m.data_ro_view();
  if (!t.key.length) return m.data_ro_nokey();
  return null;
}

export type Message = { kind: 'ok' | 'error'; text: string };

export class DataTab {
  info = $state.raw<TableInfo | null>(null);
  where = $state('');
  quick = $state.raw<QuickFilter[]>([]);
  order = $state.raw<Order[]>([]);
  offset = $state(0);
  page = $state.raw<Page | null>(null);
  total = $state<number | null>(null);
  estimated = $state(false);
  loading = $state(false);
  error = $state<string | null>(null);
  pending = $state.raw<Pending | null>(null);
  rev = $state(0);
  preview = $state.raw<ChangePreview | null>(null);
  showSql = $state(false);
  showAudit = $state(false);
  showRelated = $state(false);
  applying = $state(false);
  message = $state.raw<Message | null>(null);
  applied = $state.raw<Set<string>>(new Set());
  auditRev = $state(0);
  filterColumn = $state<string | null>(null);
  trail = $state.raw<Crumb[]>([]);
  ahead = $state.raw<Crumb[]>([]);
  private restored: SavedTrail | null = null;
  private seq = 0;
  private starts: ((string | null)[] | null)[] = [null];

  touch() {
    this.rev++;
  }

  get pages(): number | null {
    return this.total === null ? null : Math.max(1, Math.ceil(this.total / PAGE));
  }

  async open(session: SessionInfo, info: TableInfo) {
    if (this.info && this.info.namespace === info.namespace && this.info.name === info.name) return;
    if (this.blocked()) return;
    const r = this.restored;
    this.restored = null;
    const back = r && r.here.namespace === info.namespace && r.here.name === info.name ? r : null;
    this.trail = back?.trail ?? [];
    this.ahead = back?.ahead ?? [];
    this.show(info, back?.here ?? { where: '', quick: [], order: [], offset: 0 });
    await this.load(session);
  }

  saved(): string | null {
    const here = this.here();
    if (!here) return null;
    const plain = !this.trail.length && !this.ahead.length && !here.where && !here.quick.length && !here.order.length && !here.offset;
    return plain ? null : JSON.stringify({ here, trail: this.trail, ahead: this.ahead } satisfies SavedTrail);
  }

  restore(json: string) {
    this.restored = readTrail(json);
  }

  async follow(session: SessionInfo, info: TableInfo, quick: QuickFilter[], focus?: Crumb['focus']): Promise<boolean> {
    if (this.blocked()) return false;
    const now = this.here();
    if (now) this.trail = [...this.trail, focus ? { ...now, focus } : now];
    this.ahead = [];
    this.show(info, { where: '', quick, order: [], offset: 0 });
    await this.load(session);
    return true;
  }

  async back(session: SessionInfo, info: TableInfo, at: number): Promise<Crumb['focus']> {
    const crumb = this.trail[at];
    if (!crumb || this.blocked()) return;
    const now = this.here();
    this.ahead = [...this.ahead, ...(now ? [now] : []), ...this.trail.slice(at + 1).reverse()];
    this.trail = this.trail.slice(0, at);
    this.show(info, crumb);
    await this.load(session);
    return crumb.focus;
  }

  async forward(session: SessionInfo, info: TableInfo) {
    const crumb = this.ahead[this.ahead.length - 1];
    if (!crumb || this.blocked()) return;
    const now = this.here();
    this.ahead = this.ahead.slice(0, -1);
    if (now) this.trail = [...this.trail, now];
    this.show(info, crumb);
    await this.load(session);
  }

  private here(): Crumb | null {
    const now = this.info;
    return now ? { namespace: now.namespace, name: now.name, where: this.where, quick: this.quick, order: this.order, offset: this.offset } : null;
  }

  private blocked() {
    if (!this.pending || this.pending.empty) return false;
    this.message = { kind: 'error', text: m.data_switch_blocked() };
    return true;
  }

  private show(info: TableInfo, view: Pick<Crumb, 'where' | 'quick' | 'order' | 'offset'>) {
    this.info = info;
    this.where = view.where;
    this.quick = view.quick;
    this.order = view.order;
    this.offset = view.offset;
    this.starts = [null];
    this.total = null;
    this.page = null;
    this.message = null;
    this.preview = null;
    this.pending = new Pending({ table: { namespace: info.namespace, name: info.name }, columns: info.columns.map((c) => c.name), types: info.columns.map((c) => c.type_name), key: info.key });
    this.touch();
  }

  whole(): Browse | null {
    const info = this.info;
    if (!info) return null;
    return {
      table: { namespace: info.namespace, name: info.name },
      columns: info.columns.map((c) => c.name),
      key: info.key.map((i) => info.columns[i]?.name ?? ''),
      filter: this.where.trim() || null,
      quick: this.quick,
      order: [],
      limit: 0,
      offset: 0,
    };
  }

  async load(session: SessionInfo) {
    const base = this.whole();
    if (!base) return;
    const seq = ++this.seq;
    this.loading = true;
    this.error = null;
    try {
      const page = await api.dataBrowse(session.session_id, {
        ...base,
        order: this.order,
        limit: PAGE,
        offset: this.offset,
        after: this.starts[Math.round(this.offset / PAGE)] ?? null,
      });
      if (seq !== this.seq) return;
      this.page = page;
      if (page.total !== null) {
        this.total = page.total;
        this.estimated = page.estimated;
      }
      else if (this.offset === 0) this.total = null;
    } catch (e) {
      if (seq === this.seq) this.error = e instanceof Error ? e.message : String(e);
    } finally {
      if (seq === this.seq) this.loading = false;
    }
  }

  async reload(session: SessionInfo, from = 0) {
    const at = Math.round(from / PAGE);
    if (from === 0) this.starts = [null];
    else if (at === Math.round(this.offset / PAGE) + 1) this.starts[at] = this.page?.next ?? null;
    this.offset = from;
    if (from === 0) this.total = null;
    await this.load(session);
  }

  async refreshPreview(session: SessionInfo) {
    const p = this.pending;
    if (!p || p.empty) {
      this.preview = null;
      return;
    }
    this.preview = await api.dataPreview(session.session_id, p.build().set);
  }

  async apply(session: SessionInfo, connectionName: string) {
    const p = this.pending;
    const info = this.info;
    if (!p || p.empty || !info || this.applying) return;
    const { set, targets } = p.build();
    let confirm: string | null = null;
    if (session.environment === 'production') {
      if (!writesOpen(session.session_id)) {
        this.message = { kind: 'error', text: m.data_ro_locked() };
        return;
      }
      const [preview, counts] = await Promise.all([api.dataPreview(session.session_id, set), api.dataEstimate(session.session_id, set).catch(() => set.changes.map(() => null))]);
      const estimate = changeRows(counts, set.changes.map((ch) => ch.kind));
      const c = p.counts;
      const lines = [
        ...(c.cells ? [c.cells === 1 ? m.confirm_change_cell({ table: info.name }) : m.confirm_change_cells({ count: c.cells, table: info.name })] : []),
        ...(c.inserted ? [c.inserted === 1 ? m.data_new_one() : m.data_new_other({ count: c.inserted })] : []),
        ...(c.deleted ? [c.deleted === 1 ? m.data_del_one() : m.data_del_other({ count: c.deleted })] : []),
      ];
      const typed = await askConfirm({
        title: c.total === 1 ? m.confirm_apply_one() : m.confirm_apply_other({ count: c.total }),
        connection: connectionName,
        production: true,
        lines,
        risks: [],
        rows: estimate.rows,
        stale: estimate.stale,
        note: preview.statements.length ? m.confirm_tx() : '',
        action: m.confirm_apply_btn(),
      });
      if (typed === false) return;
      confirm = typed;
    }
    this.applying = true;
    this.message = null;
    try {
      const out = await api.dataApply(session.session_id, set, confirm);
      if (out.kind === 'applied') {
        const done = new Set<string>();
        for (const t of targets) if (t.kind === 'update') for (const col of t.columns) done.add(`${t.key}|${col}`);
        this.pending = new Pending(p.shape);
        this.preview = null;
        const n = out.rows ?? set.changes.length;
        play('applied');
        const held = out.held ? (n === 1 ? m.data_applied_held_one() : m.data_applied_held_other({ count: n })) : null;
        this.message = { kind: 'ok', text: held ?? (n === 1 ? m.data_applied_one() : m.data_applied_other({ count: n })) };
        if (out.held) void refreshTx(session.session_id);
        this.auditRev++;
        await this.reload(session, this.offset);
        this.applied = done;
        setTimeout(() => {
          if (this.applied === done) this.applied = new Set();
        }, 1800);
      } else {
        const t: Target | undefined = targets[out.index];
        if (t) p.setMark(t, out.kind === 'conflict' ? { kind: 'conflict' } : { kind: 'error', message: driverMessage(out.error) });
        play('error');
        this.message = { kind: 'error', text: out.kind === 'conflict' ? m.data_conflict({ n: out.index + 1 }) : m.data_failed({ n: out.index + 1, message: driverMessage(out.error) }) };
        this.auditRev++;
      }
    } catch (e) {
      this.message = { kind: 'error', text: e instanceof Error ? e.message : String(e) };
    } finally {
      this.applying = false;
      this.touch();
      if (session.environment === 'production') void settle(session.session_id);
    }
  }
}

export { rowKey };

interface SavedTrail {
  here: Crumb;
  trail: Crumb[];
  ahead: Crumb[];
}

const isCrumb = (c: unknown): c is Crumb => {
  if (!c || typeof c !== 'object') return false;
  const x = c as Record<string, unknown>;
  return typeof x.namespace === 'string' && typeof x.name === 'string' && typeof x.where === 'string' && Array.isArray(x.quick) && Array.isArray(x.order) && typeof x.offset === 'number';
};

export function readTrail(json: string): SavedTrail | null {
  try {
    const v: unknown = JSON.parse(json);
    if (!v || typeof v !== 'object') return null;
    const { here, trail, ahead } = v as Record<string, unknown>;
    if (!isCrumb(here) || !Array.isArray(trail) || !Array.isArray(ahead) || !trail.every(isCrumb) || !ahead.every(isCrumb)) return null;
    return { here, trail, ahead };
  } catch {
    return null;
  }
}
