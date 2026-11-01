import { sqlOf } from './family';
import { ClipboardCopy } from '@lucide/svelte';
import { ICON } from './icons';
import { m } from '../paraglide/messages';
import { api } from './api';
import type { Dialect, IdentityColumn } from './bindings';
import { copyText, type MenuEntry } from './contextmenu.svelte';
import type { ObjectRef } from './ddl';
import { loadedSchema, queryOn } from './explorer.svelte';
import type { Command, PaletteAction } from './palette.svelte';
import { generatorsOf, requestOf, subjectOf, type GenSubject, type Generator } from './sqlgen';
import { showToast } from './toast.svelte';
import { ensureSession, sessionFor, type Tab } from './workspace.svelte';

export interface DepsAsk {
  connectionId: string;
  database: string;
  dialect: Dialect;
  object: ObjectRef;
}

export const depsUi = $state({ ask: null as DepsAsk | null, trail: [] as DepsAsk[] });

export function showDeps(ask: DepsAsk, from: DepsAsk | null = null) {
  depsUi.trail = from ? [...depsUi.trail, from] : [];
  depsUi.ask = ask;
}

export function depsBack() {
  const prev = depsUi.trail.at(-1);
  if (!prev) return;
  depsUi.trail = depsUi.trail.slice(0, -1);
  depsUi.ask = prev;
}

export async function loadDeps(ask: DepsAsk) {
  if (!(await ensureSession(ask.connectionId, ask.database))) throw new Error(m.dbs_open_failed());
  const s = sessionFor(ask.connectionId, ask.database);
  if (!s) throw new Error(m.dbs_open_failed());
  return api.objectDependencies(s.info.session_id, ask.object);
}

async function identityOf(connectionId: string, s: GenSubject): Promise<IdentityColumn[]> {
  try {
    if (!(await ensureSession(connectionId, s.database))) return [];
    const session = sessionFor(connectionId, s.database);
    return session ? await api.tableIdentity(session.info.session_id, s.namespace, s.name) : [];
  } catch {
    return [];
  }
}

export async function generate(connectionId: string, s: GenSubject, generator: Generator, to: 'tab' | 'clipboard') {
  const identity = !s.view && (generator === 'insert' || generator === 'upsert') ? await identityOf(connectionId, s) : [];
  const sql = await api.sqlGenerate(s.dialect, requestOf(s, generator, identity));
  if (sql === null) return;
  if (to === 'clipboard') {
    await copyText(sql);
    showToast(m.gen_copied());
  } else {
    await queryOn(connectionId, s.database, sql, false);
  }
}

export function sqlEntries(connectionId: string, s: GenSubject): MenuEntry[] {
  const items = (to: 'tab' | 'clipboard'): MenuEntry[] =>
    generatorsOf(s).flatMap((g, i, all) => [
      (g.generator === 'copy' || g.generator === 'drop') && all[i - 1]?.generator !== 'copy' ? 'separator' : null,
      { label: g.label, hint: g.hint, disabled: g.disabled, danger: g.danger, run: () => generate(connectionId, s, g.generator, to) },
    ]);
  const object: ObjectRef = { kind: s.view ? 'view' : 'table', namespace: s.namespace, name: s.name, table: null, signature: null };
  return [
    { label: m.gen_menu(), icon: ICON.generateSql, items: [...items('tab'), 'separator', { label: m.gen_to_clipboard(), icon: ClipboardCopy, items: items('clipboard') }] },
    { label: m.deps_show(), icon: ICON.deps, run: () => showDeps({ connectionId, database: s.database, dialect: s.dialect, object }) },
  ];
}

export function sqlgenActions(connectionId: string, s: GenSubject): PaletteAction[] {
  const object: ObjectRef = { kind: s.view ? 'view' : 'table', namespace: s.namespace, name: s.name, table: null, signature: null };
  return [
    ...generatorsOf(s)
      .filter((g) => !g.disabled && g.generator !== 'copy' && g.generator !== 'drop')
      .map((g) => ({ id: `gen-${g.generator}`, label: m.gen_action({ what: g.label }), icon: ICON.generateSql, run: () => generate(connectionId, s, g.generator, 'tab') })),
    { id: 'deps', label: m.deps_show(), icon: ICON.deps, run: () => showDeps({ connectionId, database: s.database, dialect: s.dialect, object }) },
  ];
}

export function subjectIn(connectionId: string, dialect: Dialect, database: string, namespace: string, name: string, view: boolean): GenSubject {
  const ns = loadedSchema(connectionId, database).data?.namespaces.find((n) => n.name === namespace);
  const rel = [...(ns?.tables ?? []), ...(ns?.views ?? [])].find((r) => r.name === name) ?? null;
  return subjectOf(dialect, database, namespace, rel, name, view);
}

export function sqlgenCommands(tab: () => Tab | null): Command[] {
  const group = m.pal_group_sqlgen;
  const subject = (): { cid: string; s: GenSubject } | null => {
    const t = tab();
    const info = t?.view === 'data' ? t.data.info : null;
    const session = t ? sessionFor(t.connectionId, t.database) : null;
    if (!t || !info || !session) return null;
    const rel = { name: info.name, columns: info.columns, indexes: [], foreign_keys: [], estimated_rows: null };
    const s = subjectOf(sqlOf(session.info), session.info.server.database, info.namespace, rel, info.name, info.view);
    return { cid: t.connectionId, s: { ...s, key: info.key.flatMap((i) => info.columns[i]?.name ?? []) } };
  };
  const gen = (generator: Generator, label: () => string): Command => ({
    id: `schema.gen_${generator}`,
    label,
    group,
    when: () => {
      const x = subject();
      return !!x && !!generatorsOf(x.s).find((g) => g.generator === generator && !g.disabled);
    },
    run: () => {
      const x = subject();
      if (x) void generate(x.cid, x.s, generator, 'tab');
    },
  });
  return [
    gen('select', m.gen_cmd_select),
    gen('insert', m.gen_cmd_insert),
    gen('update', m.gen_cmd_update),
    gen('delete', m.gen_cmd_delete),
    gen('upsert', m.gen_cmd_upsert),
    {
      id: 'schema.deps',
      label: m.gen_cmd_deps,
      group,
      when: () => !!subject(),
      run: () => {
        const x = subject();
        if (x) showDeps({ connectionId: x.cid, database: x.s.database, dialect: x.s.dialect, object: { kind: x.s.view ? 'view' : 'table', namespace: x.s.namespace, name: x.s.name, table: null, signature: null } });
      },
    },
  ];
}
