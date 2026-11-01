import { m } from '../paraglide/messages';
import { appearance, updateAppearance } from './appearance.svelte';
import type { RunScope } from './bindings';
import { openPalette, bookmarkQuery, type Command } from './palette.svelte';
import { panel, togglePanel, toggleHistory, type PanelTab } from './panel.svelte';
import { currentBindings } from './prefs.svelte';
import type { ActionId } from './shortcuts';
import { type ToolKind } from './tools';
import { hasTool, viewsOf } from './family';
import { redo, undo } from './undo.svelte';
import { cancel, closeConnection, closeTab, databaseOf, newTab, refreshSchema, run, sessionOf, type Tab, type View } from './workspace.svelte';

export interface QueryEditor {
  runScope: (all: boolean) => RunScope;
  format: () => void;
  selected: () => string;
  content: () => string;
}

const bound = (id: ActionId) => () => currentBindings()[id] || null;

const showPanel = (tab: PanelTab) => () => !(panel.open && panel.tab === tab) && togglePanel(tab);

export function appCommands(hooks: { settings: () => void; newConnection: () => void }): Command[] {
  const app = m.pal_group_app;
  return [
    { id: 'palette.objects', label: m.pal_cmd_search_objects, group: app, keys: bound('palette'), run: () => openPalette('objects') },
    { id: 'palette.library', label: m.pal_cmd_search_library, group: app, run: () => openPalette('library') },
    { id: 'app.settings', label: m.pal_cmd_settings, group: app, run: hooks.settings },
    { id: 'app.new_connection', label: m.conn_new, group: app, run: hooks.newConnection },
    { id: 'app.undo', label: m.key_undo, group: app, keys: () => 'Mod+Z', run: () => undo() },
    { id: 'app.redo', label: m.key_redo, group: app, keys: () => 'Shift+Mod+Z', run: () => redo() },
    { id: 'app.sounds', label: () => (appearance.value.sounds ? m.pal_cmd_sounds_off() : m.pal_cmd_sounds_on()), group: app, run: () => updateAppearance({ sounds: !appearance.value.sounds }) },
    { id: 'app.sounds_detail', label: () => (appearance.value.sound_detail === 'reduced' ? m.pal_cmd_sounds_all() : m.pal_cmd_sounds_reduced()), group: app, when: () => appearance.value.sounds, run: () => updateAppearance({ sound_detail: appearance.value.sound_detail === 'reduced' ? 'all' : 'reduced' }) },
    { id: 'app.sound_timbre', label: () => (appearance.value.sound_timbre === 'ciotola' ? m.pal_cmd_timbre_marimba() : m.pal_cmd_timbre_ciotola()), group: app, when: () => appearance.value.sounds, run: () => updateAppearance({ sound_timbre: appearance.value.sound_timbre === 'ciotola' ? 'marimba' : 'ciotola' }) },
  ];
}

const VIEWS: [View, () => string][] = [
  ['query', m.view_query],
  ['data', m.view_data],
  ['schema', m.view_schema],
  ['tools', m.view_tools],
];

const TOOLS: [ToolKind, () => string][] = [
  ['activity', m.tool_activity],
  ['monitor', m.tool_monitor],
  ['space', m.tool_space],
  ['users', m.tool_users],
  ['compare', m.tool_compare],
  ['rows', m.tool_rows],
  ['search', m.tool_search],
  ['generate', m.tool_generate],
  ['backup', m.tool_backup],
  ['transfer', m.tool_transfer],
  ['seclab', m.tool_seclab],
  ['server', m.tool_server],
];

export function workspaceCommands(ctx: { tab: () => Tab | null; editor: () => QueryEditor | null; save: () => void; explain: (t: Tab) => void }): Command[] {
  const q = m.pal_group_query;
  const inQuery = () => ctx.tab()?.view === 'query' && !!ctx.editor();
  const withTab = (f: (t: Tab) => unknown) => () => {
    const t = ctx.tab();
    if (t) void f(t);
  };
  const withEditor = (f: (t: Tab, e: QueryEditor) => unknown) => () => {
    const t = ctx.tab();
    const e = ctx.editor();
    if (t && e) void f(t, e);
  };
  const caps = () => {
    const t = ctx.tab();
    return (t ? sessionOf(t) : null)?.info.caps ?? null;
  };
  return [
    { id: 'query.run', label: m.key_run_statement, group: q, keys: bound('run_statement'), when: () => inQuery() && !ctx.tab()?.running, run: withEditor((t, e) => run(t, e.runScope(false))) },
    { id: 'query.run_all', label: m.key_run_script, group: q, keys: bound('run_script'), when: () => inQuery() && !ctx.tab()?.running, run: withEditor((t, e) => run(t, e.runScope(true))) },
    { id: 'query.dry_run', label: m.key_dry_run, group: q, keys: bound('dry_run'), when: () => inQuery() && !ctx.tab()?.running && !!caps()?.transactions, run: withEditor((t, e) => run(t, e.runScope(false), true)) },
    { id: 'query.cancel', label: m.key_cancel, group: q, keys: bound('cancel'), when: () => !!ctx.tab()?.running, run: withTab(cancel) },
    { id: 'query.explain', label: m.plan_explain, group: q, when: () => inQuery() && !!caps()?.explain, run: withTab(ctx.explain) },
    { id: 'query.explain_pipeline', label: m.pal_cmd_explain_pipeline, group: q, when: () => { const l = caps()?.language; return inQuery() && l?.kind === 'sql' && l.dialect === 'clickhouse'; }, run: withEditor((t, e) => newTab(t.connectionId, `EXPLAIN PIPELINE\n${(e.selected() || e.content()).trim().replace(/;\s*$/, '')}`)) },
    { id: 'query.format', label: m.key_format, group: q, keys: bound('format'), when: inQuery, run: withEditor((_, e) => e.format()) },
    { id: 'query.save', label: m.key_save_script, group: q, keys: bound('save_script'), when: inQuery, run: ctx.save },
    { id: 'query.bookmark', label: m.pal_cmd_bookmark_query, group: q, when: inQuery, run: withEditor((t, e) => bookmarkQuery(t.connectionId, databaseOf(t), e.selected() || e.content())) },
    { id: 'panel.notes', label: m.key_panel_notes, group: m.pal_group_panels, keys: bound('panel_notes'), run: () => togglePanel('notes') },
    { id: 'panel.scripts', label: m.pal_cmd_scripts, group: m.pal_group_panels, run: showPanel('scripts') },
    { id: 'panel.snippets', label: m.pal_cmd_snippets, group: m.pal_group_panels, run: showPanel('snippets') },
    { id: 'panel.history', label: m.key_panel_history, group: m.pal_group_panels, keys: bound('panel_history'), when: inQuery, run: toggleHistory },
    { id: 'tab.new', label: m.tab_new, group: m.pal_group_connection, keys: bound('new_tab'), run: withTab((t) => newTab(t.connectionId)) },
    { id: 'tab.close', label: m.tab_close, group: m.pal_group_connection, run: withTab((t) => closeTab(t.id)) },
    { id: 'schema.refresh', label: m.schema_refresh, group: m.pal_group_connection, run: withTab((t) => refreshSchema(t.connectionId, t.database)) },
    { id: 'connection.close', label: m.conn_disconnect, group: m.pal_group_connection, run: withTab((t) => closeConnection(t.connectionId)) },
    ...VIEWS.map(([view, name]): Command => ({ id: `view.${view}`, label: () => m.pal_cmd_view({ view: name() }), group: m.pal_group_view, when: () => {
        const c = caps();
        return !!ctx.tab() && ctx.tab()?.view !== view && (!c || viewsOf(c).includes(view));
      }, run: withTab((t) => (t.view = view)) })),
    ...TOOLS.map(([tool, name]): Command => ({
      id: `tool.${tool}`,
      label: () => m.pal_cmd_tool({ tool: name() }),
      group: m.pal_group_tools,
      when: () => {
        const c = caps();
        return !!c && hasTool(c, tool);
      },
      run: withTab((t) => {
        t.view = 'tools';
        t.tool = tool;
      }),
    })),
  ];
}
