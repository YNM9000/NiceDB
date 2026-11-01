import { m } from '../paraglide/messages';
import type { FileFormat, Maintenance } from './bindings';
import { connections } from './connections.svelte';
import { copyText } from './contextmenu.svelte';
import { showDdl } from './ddl.svelte';
import { openDesigner } from './design.svelte';
import { hasTool, isSql, sqlOf } from './family';
import { seedGenerate } from './generate';
import { ICON, KIND_ICON, TOOL_ICON } from './icons';
import { MAINTENANCE_LABEL, maintenanceOf } from './monitor';
import { maintain } from './monitor.svelte';
import { createFolder, createSpace, currentSpace, duplicateSpace, orgUi, spaces, switchSpace } from './org.svelte';
import { connectionUrl } from './connurl';
import { paletteHost, type Choice, type Command } from './palette.svelte';
import { qualifiedName } from './sqlname';
import { variantOfSession } from './tools';
import { askExport, askImport, compareWith } from './tools.svelte';
import { closeConnection, closeOthers, closeTabs, duplicateTab, ensureSession, newTab, openConnection, openTable, renameTab, sessionOf, showInEr, useDatabase, workspace, type Tab } from './workspace.svelte';

const ENV = { local: m.env_local, development: m.env_development, staging: m.env_staging, production: m.env_production } as const;

export const EXPORT_FORMATS: { id: FileFormat; label: string }[] = [
  { id: 'csv', label: 'CSV' },
  { id: 'tsv', label: 'TSV' },
  { id: 'json', label: 'JSON' },
  { id: 'xlsx', label: 'Excel (XLSX)' },
  { id: 'parquet', label: 'Parquet' },
];

const connectionChoices = (keep: (id: string) => boolean = () => true): Choice[] =>
  connections.list.filter((c) => keep(c.id)).map((c) => ({ id: c.id, label: c.name, where: ENV[c.environment](), icon: KIND_ICON.connection }));

export function moreCommands(ctx: { tab: () => Tab | null; editorText: () => string | null }): Command[] {
  const conn = m.pal_group_connection;
  const withTab = (f: (t: Tab) => unknown) => () => {
    const t = ctx.tab();
    if (t) void f(t);
  };
  const session = () => {
    const t = ctx.tab();
    return t ? sessionOf(t) : null;
  };
  const shown = () => {
    const t = ctx.tab();
    const info = t?.view === 'data' ? t.data.info : null;
    return t && info ? { t, info } : null;
  };
  const sql = () => {
    const s = session();
    return !!s && isSql(s.info.caps);
  };
  const tableName = () => shown()?.info.name ?? '';
  const onTable = (f: (t: Tab, ns: string, name: string, view: boolean) => unknown) => () => {
    const x = shown();
    if (x) void f(x.t, x.info.namespace, x.info.name, x.info.view);
  };
  return [
    {
      id: 'connection.connect_to',
      label: m.pal_cmd_connect_to,
      group: conn,
      icon: ICON.connect,
      when: () => connections.list.length > 0,
      run: () => {},
      ask: { prompt: m.pal_ask_connection, choices: () => connectionChoices(), run: (id) => openConnection(id) },
    },
    {
      id: 'connection.new_tab_on',
      label: m.pal_cmd_new_tab_on,
      group: conn,
      icon: ICON.newTab,
      when: () => connections.list.length > 0,
      run: () => {},
      ask: { prompt: m.pal_ask_connection, choices: () => connectionChoices(), run: async (id) => (await ensureSession(id, null)) && newTab(id) },
    },
    {
      id: 'connection.disconnect_from',
      label: m.pal_cmd_disconnect_from,
      group: conn,
      icon: ICON.disconnect,
      when: () => Object.keys(workspace.sessions).length > 0,
      run: () => {},
      ask: { prompt: m.pal_ask_open_connection, choices: () => connectionChoices((id) => !!workspace.sessions[id]), run: (id) => closeConnection(id) },
    },
    ...(['edit', 'copy', 'delete'] as const).map(
      (mode): Command => ({
        id: `connection.${mode}`,
        label: { edit: m.pal_cmd_conn_edit, copy: m.pal_cmd_conn_copy, delete: m.pal_cmd_conn_delete }[mode],
        group: conn,
        icon: { edit: ICON.edit, copy: ICON.duplicate, delete: ICON.remove }[mode],
        when: () => connections.list.length > 0 && !!paletteHost.connectionDialog,
        run: () => {},
        ask: {
          prompt: m.pal_ask_connection,
          choices: () => connectionChoices(),
          run: (id) => {
            const p = connections.list.find((c) => c.id === id);
            if (p) paletteHost.connectionDialog?.(p, mode);
          },
        },
      }),
    ),
    {
      id: 'space.switch',
      label: m.pal_cmd_space_switch,
      group: m.pal_group_app,
      icon: ICON.space,
      run: () => {},
      ask: {
        prompt: m.pal_ask_space,
        choices: () => [{ id: '', label: m.org_space_all(), icon: ICON.spaces }, ...spaces.list.map((w) => ({ id: w.id, label: w.name, icon: ICON.space }))],
        run: (id) => switchSpace(id || null),
      },
    },
    {
      id: 'space.new',
      label: m.pal_cmd_space_new,
      group: m.pal_group_app,
      icon: ICON.spaces,
      run: () => {},
      ask: { prompt: m.pal_ask_space_name, run: (name) => createSpace(name, [], []) },
    },
    {
      id: 'connection.copy_url',
      label: m.pal_cmd_conn_copy_url,
      group: conn,
      icon: ICON.copyUrl,
      when: () => connections.list.length > 0,
      run: () => {},
      ask: {
        prompt: m.pal_ask_connection,
        choices: () => connectionChoices(),
        run: (id) => {
          const p = connections.list.find((c) => c.id === id);
          if (p) void copyText(connectionUrl(p));
        },
      },
    },
    {
      id: 'org.folder_new',
      label: m.pal_cmd_folder_new,
      group: m.pal_group_app,
      icon: ICON.folderNew,
      run: async () => {
        const f = await createFolder(null);
        orgUi.renaming = f.id;
      },
    },
    {
      id: 'org.export',
      label: m.pal_cmd_org_export,
      group: m.pal_group_app,
      icon: ICON.export,
      run: () => {
        const w = currentSpace();
        orgUi.exporting = w ? { folders: [...w.folders], connections: [...w.connections] } : { folders: [], connections: [] };
      },
    },
    { id: 'org.import', label: m.pal_cmd_org_import, group: m.pal_group_app, icon: ICON.import, run: () => (orgUi.importing = true) },
    ...(
      [
        ['edit', m.pal_cmd_space_edit, ICON.rename, (id: string) => (orgUi.space = { id, preset: null })],
        ['duplicate', m.pal_cmd_space_duplicate, ICON.duplicate, (id: string) => duplicateSpace(id)],
        ['delete', m.pal_cmd_space_delete, ICON.remove, (id: string) => (orgUi.space = { id, preset: null, remove: true })],
      ] as const
    ).map(
      ([what, label, icon, act]): Command => ({
        id: `space.${what}`,
        label,
        group: m.pal_group_app,
        icon,
        when: () => spaces.list.length > 0,
        run: () => {},
        ask: { prompt: m.pal_ask_space, choices: () => spaces.list.map((w) => ({ id: w.id, label: w.name, where: w.id === spaces.active ? m.pal_current() : '', icon: ICON.space })), run: (id) => act(id) },
      }),
    ),
    {
      id: 'tab.close_right',
      label: m.pal_cmd_tab_close_right,
      group: conn,
      icon: ICON.closeTab,
      when: () => {
        const t = ctx.tab();
        return !!t && workspace.tabs.indexOf(t) < workspace.tabs.length - 1;
      },
      run: withTab((t) => closeTabs(workspace.tabs.slice(workspace.tabs.indexOf(t) + 1))),
    },
    { id: 'tab.duplicate', label: m.tab_duplicate, group: conn, icon: ICON.duplicate, when: () => !!ctx.tab(), run: withTab(duplicateTab) },
    { id: 'tab.close_others', label: m.tab_close_others, group: conn, icon: ICON.closeTab, when: () => workspace.tabs.length > 1, run: withTab(closeOthers) },
    {
      id: 'tab.rename',
      label: m.pal_cmd_tab_rename,
      group: conn,
      icon: ICON.rename,
      when: () => !!ctx.tab(),
      run: () => {},
      ask: { prompt: () => ctx.tab()?.title ?? m.pal_ask_tab_name(), run: (title) => ctx.tab() && renameTab(ctx.tab() as Tab, title) },
    },
    {
      id: 'schema.use_database',
      label: m.pal_cmd_use_database,
      group: conn,
      icon: ICON.openDatabase,
      when: () => (session()?.schema?.databases.length ?? 0) > 1,
      run: () => {},
      ask: {
        prompt: m.pal_ask_database,
        choices: () => (session()?.schema?.databases ?? []).map((d) => ({ id: d, label: d, where: d === session()?.database ? m.pal_current() : '', icon: KIND_ICON.database })),
        run: (db) => ctx.tab() && useDatabase(ctx.tab() as Tab, db),
      },
    },
    {
      id: 'schema.open_table',
      label: m.pal_cmd_open_table,
      group: conn,
      icon: ICON.openData,
      when: () => !!session()?.schema && sql(),
      run: () => {},
      ask: {
        prompt: m.pal_ask_table,
        choices: () =>
          (session()?.schema?.namespaces ?? []).flatMap((n) => [
            ...n.tables.map((r) => ({ id: `${n.name}\u0000${r.name}`, label: r.name, where: n.name, icon: KIND_ICON.table })),
            ...n.views.map((r) => ({ id: `${n.name}\u0000${r.name}`, label: r.name, where: n.name, icon: KIND_ICON.view })),
          ]),
        run: (key) => {
          const [ns = '', name = ''] = key.split('\u0000');
          const t = ctx.tab();
          if (t) openTable(t, ns, name);
        },
      },
    },
    {
      id: 'schema.maintain_db',
      label: m.pal_cmd_maintain_db,
      group: m.pal_group_tools,
      icon: ICON.maintain,
      when: () => {
        const s = session();
        return !!s && sql() && maintenanceOf(variantOfSession(s.info), 'database').length > 0;
      },
      run: () => {},
      ask: {
        prompt: m.pal_ask_maintenance,
        choices: () => {
          const s = session();
          if (!s) return [];
          return maintenanceOf(variantOfSession(s.info), 'database').map((a) => ({ id: a, label: MAINTENANCE_LABEL[a](), icon: ICON.maintain }));
        },
        run: (action) => {
          const t = ctx.tab();
          const s = session();
          if (t && s) void maintain(t.connectionId, s.database, action as Maintenance, null);
        },
      },
    },
    {
      id: 'tool.compare_with',
      label: m.pal_cmd_compare_with,
      group: m.pal_group_tools,
      icon: TOOL_ICON.compare,
      when: () => {
        const s = session();
        return !!s?.info.caps && hasTool(s.info.caps, 'compare') && connections.list.length > 1;
      },
      run: () => {},
      ask: {
        prompt: m.pal_ask_compare_target,
        choices: () => {
          const t = ctx.tab();
          const me = connections.list.find((c) => c.id === t?.connectionId);
          const same = (id: string) => connections.list.find((c) => c.id === id)?.target.driver === me?.target.driver;
          return connectionChoices((id) => id !== t?.connectionId).sort((a, b) => Number(same(b.id)) - Number(same(a.id)));
        },
        run: (id) => ctx.tab() && compareWith(ctx.tab() as Tab, id),
      },
    },
    {
      id: 'query.open_in',
      label: m.pal_cmd_open_in,
      group: m.pal_group_query,
      icon: ICON.openIn,
      when: () => ctx.tab()?.view === 'query' && !!ctx.editorText()?.trim(),
      run: () => {},
      ask: {
        prompt: m.pal_ask_connection,
        choices: () => connectionChoices((id) => id !== ctx.tab()?.connectionId),
        run: async (id) => {
          const text = ctx.editorText() ?? '';
          if (await ensureSession(id, null)) newTab(id, text);
        },
      },
    },
    {
      id: 'data.export_to',
      label: () => m.pal_cmd_export_to({ table: tableName() }),
      group: m.pal_group_tools,
      icon: ICON.export,
      when: () => !!shown(),
      run: () => {},
      ask: {
        prompt: m.pal_ask_format,
        choices: () => EXPORT_FORMATS.map((f) => ({ id: f.id, label: f.label, icon: ICON.export })),
        run: (format) => {
          const x = shown();
          if (x) askExport({ connectionId: x.t.connectionId, database: x.t.database, source: { kind: 'table', namespace: x.info.namespace, name: x.info.name }, label: x.info.name, expected: null, format: format as FileFormat });
        },
      },
    },
    { id: 'data.import', label: () => m.pal_cmd_import_into({ table: tableName() }), group: m.pal_group_tools, icon: ICON.import, when: () => !!shown() && !shown()?.info.view, run: onTable((t, ns, name) => askImport({ connectionId: t.connectionId, database: t.database, table: { namespace: ns, name } })) },
    {
      id: 'data.ddl',
      label: () => m.pal_cmd_ddl_of({ table: tableName() }),
      group: m.pal_group_view,
      icon: ICON.ddl,
      when: () => !!shown() && !!session()?.info.caps.ddl,
      run: onTable((t, ns, name, view) => {
        const s = sessionOf(t);
        if (s) showDdl({ connectionId: t.connectionId, database: s.database, dialect: sqlOf(s.info), product: s.info.server.product, object: { kind: view ? 'view' : 'table', namespace: ns, name, table: null, signature: null } });
      }),
    },
    { id: 'data.er', label: () => m.pal_cmd_er_of({ table: tableName() }), group: m.pal_group_view, icon: ICON.er, when: () => !!shown() && !!session()?.info.caps.er, run: onTable((t, ns, name) => showInEr(t, ns, name)) },
    { id: 'data.design', label: () => m.pal_cmd_design_of({ table: tableName() }), group: m.pal_group_view, icon: ICON.design, when: () => !!shown() && !shown()?.info.view && sql(), run: onTable((t, ns, name) => openDesigner(t.connectionId, sessionOf(t)?.database ?? '', ns, name)) },
    {
      id: 'data.copy_name',
      label: () => m.pal_cmd_copy_name_of({ table: tableName() }),
      group: m.pal_group_view,
      icon: ICON.copy,
      when: () => !!shown(),
      run: onTable((t, ns, name) => {
        const s = sessionOf(t);
        if (s) void copyText(qualifiedName(sqlOf(s.info), ns, name, s.database));
      }),
    },
    {
      id: 'data.generate_for',
      label: m.pal_cmd_generate_for,
      group: m.pal_group_tools,
      icon: TOOL_ICON.generate,
      when: () => {
        const s = session();
        return !!s?.info.caps && hasTool(s.info.caps, 'generate') && s.info.environment !== 'production';
      },
      run: () => {},
      ask: {
        prompt: m.pal_ask_table,
        choices: () => (session()?.schema?.namespaces ?? []).flatMap((n) => n.tables.map((r) => ({ id: `${n.name}\u0000${r.name}`, label: r.name, where: n.name, icon: KIND_ICON.table }))),
        run: (key) => {
          const [ns = '', name = ''] = key.split('\u0000');
          const t = ctx.tab();
          if (!t) return;
          seedGenerate(ns, name);
          t.view = 'tools';
          t.tool = 'generate';
        },
      },
    },
  ];
}
