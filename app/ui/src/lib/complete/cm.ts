import { pickedCompletion, startCompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import { EditorSelection, Facet, StateEffect, StateField, type EditorState, type Extension } from '@codemirror/state';
import { Decoration, EditorView, hoverTooltip, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import { m } from '../../paraglide/messages';
import type { Schema } from '../api';
import type { Dialect } from '../bindings';
import { catalogOf } from './catalog';
import type { Engine } from './dialects';
import { complete, type Candidate, type CandidateType } from './engine';
import { inspect, type Finding } from './inspect';
import { aliasOccurrences } from './rename';
import { columnAt } from './column-at';
import { factsOf, type Describe } from '../colinfo';
import { cardDom } from '../colcard';

export const sqlDialect = Facet.define<Dialect, Dialect>({ combine: (v) => v[0] ?? 'postgres' });

const RECENT_KEY = 'nicedb.completion-recent';
const RECENT_MAX = 300;
let recent: Map<string, { n: number; at: number }> | null = null;

const recentKey = (c: { type?: string; label: string }) => `${c.type ?? ''}:${c.label.toLowerCase()}`;

function recentMap(): Map<string, { n: number; at: number }> {
  if (recent) return recent;
  recent = new Map();
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as [string, { n: number; at: number }][];
    if (Array.isArray(raw)) recent = new Map(raw);
  } catch {
    recent = new Map();
  }
  return recent;
}

export function recencyBoost(c: { type?: string; label: string }, now = Date.now()): number {
  const hit = recentMap().get(recentKey(c));
  if (!hit) return 0;
  const days = (now - hit.at) / 86_400_000;
  return Math.min(8, Math.round(Math.log2(1 + hit.n) * 2 + Math.max(0, 3 - days)));
}

export function remember(c: { type?: string; label: string }, now = Date.now()) {
  const map = recentMap();
  const k = recentKey(c);
  const hit = map.get(k);
  map.delete(k);
  map.set(k, { n: (hit?.n ?? 0) + 1, at: now });
  while (map.size > RECENT_MAX) map.delete(map.keys().next().value as string);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([...map]));
  } catch {
    return;
  }
}

type Kind = CandidateType | 'snippet';

const KIND_MARK: Record<Kind, string> = { table: 'T', view: 'V', column: 'C', function: 'ƒ', keyword: '', schema: 'S', database: 'D', alias: 'A', join: '⋈', cte: 'W', type: 'τ', snippet: '§' };

function kindName(t: Kind): string {
  const names: Record<Kind, () => string> = {
    table: m.cmp_kind_table,
    view: m.cmp_kind_view,
    column: m.cmp_kind_column,
    function: m.cmp_kind_function,
    keyword: m.cmp_kind_keyword,
    schema: m.cmp_kind_schema,
    database: m.cmp_kind_database,
    alias: m.cmp_kind_alias,
    join: m.cmp_kind_join,
    cte: m.cmp_kind_cte,
    type: m.cmp_kind_type,
    snippet: m.cmp_kind_snippet,
  };
  return names[t]();
}

const kindOf = (c: Completion): Kind => (c.type && c.type in KIND_MARK ? (c.type as Kind) : 'keyword');

const MAX_WINDOW = 40_000;

function windowOf(state: EditorState, pos: number): [number, number] {
  const len = state.doc.length;
  if (len <= MAX_WINDOW) return [0, len];
  let node = syntaxTree(state).resolveInner(pos, -1);
  for (let n: typeof node | null = node; n; n = n.parent) if (n.name === 'Statement') node = n;
  if (node.name === 'Statement' && node.to - node.from < MAX_WINDOW * 4) return [node.from, Math.min(len, node.to)];
  return [Math.max(0, pos - MAX_WINDOW), Math.min(len, pos + MAX_WINDOW / 4)];
}

function apply(c: Candidate) {
  const text = c.apply ?? c.label;
  if (!c.call && !c.chain) return text;
  return (view: EditorView, completion: Completion, from: number, to: number) => {
    const insert = c.call ? `${text}()` : text;
    const cursor = from + text.length + (c.call ? 1 : 0);
    view.dispatch({ changes: { from, to, insert }, selection: { anchor: cursor }, annotations: pickedCompletion.of(completion), userEvent: 'input.complete' });
    if (c.chain) queueMicrotask(() => startCompletion(view));
  };
}

function toCompletion(c: Candidate, env: SqlEnv): Completion {
  const catalog = c.source ? catalogOf(env.schema, env.searchPath ?? null) : null;
  const facts = catalog && c.source ? factsOf(catalog, c.source, c.label) : null;
  return {
    ...(facts ? { info: () => cardDom(facts, env.describe ?? null) } : {}),
    label: c.label,
    type: c.type,
    detail: c.type === 'view' ? [kindName('view').toLowerCase(), c.detail].filter(Boolean).join(' · ') : c.type === 'join' ? m.cmp_from_fk() : c.detail,
    boost: Math.max(-99, Math.min(99, c.boost * 4)),
    apply: apply(c),
  };
}

export interface SqlEnv {
  dialect: Dialect;
  engine: Engine;
  schema: Schema | null;
  upper: boolean;
  searchPath?: readonly string[] | null;
  describe?: Describe | null;
}

export function completionSource(env: SqlEnv) {
  return (ctx: CompletionContext): CompletionResult | null => {
    const [from, to] = windowOf(ctx.state, ctx.pos);
    const r = complete(ctx.state.sliceDoc(from, to), ctx.pos - from, { dialect: env.dialect, engine: env.engine, catalog: catalogOf(env.schema, env.searchPath ?? null), upper: env.upper, recency: recencyBoost }, ctx.explicit);
    if (!r || !r.options.length) return null;
    return { from: r.from + from, to: r.to + from, options: r.options.map((c) => toCompletion(c, env)), validFor: /^[\p{L}\p{N}_$]*$/u };
  };
}

export function columnHover(env: SqlEnv): Extension {
  return hoverTooltip(
    (view, pos) => {
      const [from, to] = windowOf(view.state, pos);
      const catalog = catalogOf(env.schema, env.searchPath ?? null);
      const hit = columnAt(view.state.sliceDoc(from, to), pos - from, env.dialect, catalog);
      const facts = hit && catalog ? factsOf(catalog, hit.rel, hit.name) : null;
      if (!hit || !facts) return null;
      return { pos: hit.from + from, end: hit.to + from, create: () => cardDom(facts, env.describe ?? null) };
    },
    { hoverTime: 450 },
  );
}

export const recency = EditorView.updateListener.of((u) => {
  for (const tr of u.transactions) {
    const picked = tr.annotation(pickedCompletion);
    if (picked) remember({ type: picked.type, label: picked.label });
  }
});

export const kindColumn = {
  render(c: Completion): Node | null {
    const kind = kindOf(c);
    const span = document.createElement('span');
    span.className = `cm-kind cm-kind-${kind}`;
    span.textContent = KIND_MARK[kind];
    span.dataset.tip = kindName(kind);
    span.setAttribute('aria-hidden', 'true');
    return span;
  },
  position: 20,
};

const setFindings = StateEffect.define<Finding[]>();

const findings = StateField.define<{ list: Finding[]; deco: DecorationSet }>({
  create: () => ({ list: [], deco: Decoration.none }),
  update(v, tr) {
    let { list, deco } = v;
    if (tr.docChanged) {
      deco = deco.map(tr.changes);
      list = list.map((f) => ({ ...f, from: tr.changes.mapPos(f.from, 1), to: tr.changes.mapPos(f.to, -1) })).filter((f) => f.to > f.from);
    }
    for (const e of tr.effects) {
      if (!e.is(setFindings)) continue;
      list = e.value;
      const len = tr.state.doc.length;
      deco = Decoration.set(
        list.filter((f) => f.to <= len && f.to > f.from).map((f) => Decoration.mark({ class: 'cm-unknown', attributes: { 'data-unknown': f.kind } }).range(f.from, f.to)),
        true,
      );
    }
    return { list, deco };
  },
  provide: (f) => EditorView.decorations.from(f, (v) => v.deco),
});

export function findingText(f: Finding): string {
  if (f.kind === 'table') return f.elsewhere ? m.cmp_outside_path({ name: f.name, schema: f.elsewhere }) : m.cmp_unknown_table({ name: f.name });
  return f.table ? m.cmp_unknown_column_of({ name: f.name, table: f.table }) : m.cmp_unknown_column({ name: f.name });
}

const MAX_INSPECTED = 200_000;
const DELAY_MS = 350;

export function inspections(dialect: Dialect, schema: Schema | null, on: boolean, searchPath: readonly string[] | null = null): Extension {
  if (!on || !schema) return [];
  const plugin = ViewPlugin.fromClass(
    class {
      timer: ReturnType<typeof setTimeout> | null = null;
      constructor(readonly view: EditorView) {
        this.schedule(0);
      }
      update(u: ViewUpdate) {
        if (u.docChanged) this.schedule(DELAY_MS);
      }
      schedule(ms: number) {
        if (this.timer) clearTimeout(this.timer);
        this.timer = setTimeout(() => {
          this.timer = null;
          const doc = this.view.state.doc;
          const list = doc.length > MAX_INSPECTED ? [] : inspect(doc.toString(), dialect, catalogOf(schema, searchPath));
          this.view.dispatch({ effects: setFindings.of(list) });
        }, ms);
      }
      destroy() {
        if (this.timer) clearTimeout(this.timer);
      }
    },
  );
  const tip = hoverTooltip((view, pos) => {
    const f = view.state.field(findings).list.find((x) => pos >= x.from && pos <= x.to);
    if (!f) return null;
    return {
      pos: f.from,
      end: f.to,
      create: () => {
        const dom = document.createElement('div');
        dom.className = 'cm-unknown-tip';
        const head = document.createElement('div');
        head.className = 'cm-unknown-head';
        head.textContent = findingText(f);
        const note = document.createElement('div');
        note.className = 'cm-unknown-note';
        note.textContent = m.cmp_unknown_note();
        dom.append(head, note);
        return { dom };
      },
    };
  });
  return [findings, plugin, tip];
}

export function renameAlias(view: EditorView): boolean {
  const sel = view.state.selection.main;
  const occ = aliasOccurrences(view.state.doc.toString(), sel.head, view.state.facet(sqlDialect));
  if (!occ) return false;
  view.dispatch({
    selection: EditorSelection.create(
      occ.ranges.map(([a, b]) => EditorSelection.range(a, b)),
      occ.main,
    ),
    scrollIntoView: true,
    userEvent: 'select',
  });
  return true;
}

export function canRenameAlias(state: EditorState): boolean {
  return !!aliasOccurrences(state.doc.toString(), state.selection.main.head, state.facet(sqlDialect));
}

export function addCursor(forward: boolean) {
  return (view: EditorView): boolean => {
    const ranges = view.state.selection.ranges;
    const extra = ranges.map((r) => view.moveVertically(EditorSelection.cursor(r.head, r.assoc), forward));
    if (extra.every((r, i) => r.head === ranges[i]?.head)) return false;
    view.dispatch({ selection: EditorSelection.create([...ranges, ...extra], ranges.length + extra.length - 1), scrollIntoView: true, userEvent: 'select' });
    return true;
  };
}
