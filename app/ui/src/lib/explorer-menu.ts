import { browseText, hasTool, isSql, relationalCaps, sqlOf } from './family';
import { openDynamoBuilder } from './dynamo-builder.svelte';
import { dynamoQuery, dynamoScan } from './nosql-lang';
import { ICON, TOOL_ICON } from './icons';
import { m } from '../paraglide/messages';
import type { Dialect, Maintenance } from './bindings';
import { copyText, keys, type MenuEntry } from './contextmenu.svelte';
import { objectOf, type ObjectRef } from './ddl';
import { isViewGroup, type ExNode, type NodePath, type Loaded } from './explorer';
import type { Schema, SessionInfo } from './api';
import { qualifiedName, selectSql } from './sqlname';
import { MAINTENANCE_LABEL, maintenanceOf, maintenanceSql } from './monitor';
import { flavorOf, variantOfSession } from './tools';
import { seedSearch } from './search';
import { seedGenerate } from './generate';
import { subjectOf } from './sqlgen';
import { sqlEntries } from './sqlgen.svelte';
import { openDesigner } from './design.svelte';

export interface MenuContext {
  connectionId: string;
  dialect: Dialect;
  info: SessionInfo | null;
  schema: (database: string) => Loaded<Schema>;
  databaseShown: (database: string) => boolean;
  schemaShown: (database: string, namespace: string) => boolean;
  isOpen: (key: string) => boolean;
  act: Actions;
}

export interface Actions {
  toggle: (n: ExNode) => void;
  activate: (n: ExNode) => void;
  refresh: (n: ExNode) => void;
  query: (database: string, sql: string) => void;
  openData: (path: NodePath) => void;
  er: (path: NodePath) => void;
  user: (key: string) => void;
  tool: (database: string | null, tool: 'activity' | 'monitor' | 'space' | 'users' | 'compare' | 'backup' | 'seclab' | 'search' | 'generate') => void;
  maintain: (database: string, action: Maintenance, table: { namespace: string; name: string } | null) => void;
  exportTable: (path: NodePath) => void;
  importTable: (path: NodePath) => void;
  hideDatabase: (database: string, shown: boolean) => void;
  hideSchema: (database: string, namespace: string, shown: boolean) => void;
  kill: (id: string, query: string | null, mode: 'cancel' | 'terminate') => void;
  ddl: (database: string, object: ObjectRef) => void;
  copyDdl: (database: string, object: ObjectRef) => void;
  columnInfo: (n: ExNode) => void;
}

function ddlEntries(ctx: MenuContext, n: ExNode): MenuEntry[] {
  const object = objectOf(n, ctx.dialect);
  const database = n.path.database ?? ctx.info?.server.database ?? '';
  if (!object) return [];
  const sql = sqlOn(ctx);
  return [
    { label: sql ? m.ddl_show() : m.ddl_show_command(), icon: ICON.ddl, run: () => ctx.act.ddl(database, object) },
    { label: sql ? m.tree_copy_create() : m.tree_copy_create_command(), run: () => ctx.act.copyDdl(database, object) },
  ];
}

function maintenanceEntries(ctx: MenuContext, database: string, table: { namespace: string; name: string } | null): MenuEntry[] {
  if (!ctx.info) return [];
  const flavor = flavorOf(sqlOf(ctx.info), ctx.info.server.product);
  return maintenanceOf(variantOfSession(ctx.info), table ? 'table' : 'database').map((action) => ({
    label: MAINTENANCE_LABEL[action](),
    icon: ICON.maintain,
    hint: maintenanceSql(flavor, action, null).replace(/^EXEC /, ''),
    run: () => ctx.act.maintain(database, action, table),
  }));
}

const capsOf = (ctx: MenuContext) => ctx.info?.caps ?? relationalCaps(ctx.dialect);
const sqlOn = (ctx: MenuContext) => isSql(capsOf(ctx));
const toolOn = (ctx: MenuContext, tool: Parameters<typeof hasTool>[1]) => hasTool(capsOf(ctx), tool);

const refresh = (ctx: MenuContext, n: ExNode): MenuEntry => ({ label: m.schema_refresh(), icon: ICON.refresh, run: () => ctx.act.refresh(n) });
const folding = (ctx: MenuContext, n: ExNode): MenuEntry => (n.expandable ? { label: n.open ? m.ex_collapse() : m.ex_expand(), run: () => ctx.act.toggle(n) } : false);

export function menuOf(ctx: MenuContext, n: ExNode): MenuEntry[] {
  const p = n.path;
  switch (n.kind) {
    case 'database':
      return databaseMenu(ctx, n);
    case 'databases':
      return [folding(ctx, n), refresh(ctx, n)];
    case 'schemas':
    case 'platform':
      return [folding(ctx, n), refresh(ctx, n)];
    case 'schema': {
      const database = p.database ?? '';
      const name = p.namespace ?? '';
      return [
        folding(ctx, n),
        'separator',
        { label: m.ex_query_here(), icon: ICON.newQuery, run: () => ctx.act.query(database, '') },
        sqlOn(ctx) && { label: m.dz_new_table(), icon: ICON.newTable, run: () => void openDesigner(ctx.connectionId, database, name, null) },
        toolOn(ctx, 'search') && { label: m.tool_search_dots(), icon: TOOL_ICON.search, run: () => (seedSearch(name), ctx.act.tool(database, 'search')) },
        { label: m.tree_copy_name(), icon: ICON.copy, hint: name, run: () => copyText(name) },
        'separator',
        { label: m.ex_hide_schema(), icon: ICON.hide, run: () => ctx.act.hideSchema(database, name, false) },
        refresh(ctx, n),
      ];
    }
    case 'group':
      return [
        folding(ctx, n),
        p.group === 'tables' && sqlOn(ctx) && { label: m.dz_new_table(), icon: ICON.newTable, run: () => void openDesigner(ctx.connectionId, p.database ?? '', p.namespace ?? '', null) },
        refresh(ctx, n),
      ];
    case 'relation':
      return relationMenu(ctx, n);
    case 'routine': {
      const full = `${qualifiedName(ctx.dialect, p.namespace ?? '', p.name ?? '', p.database)}${n.detail ?? ''}`;
      return [...ddlEntries(ctx, n), 'separator', { label: m.tree_copy_signature(), icon: ICON.copy, hint: full, run: () => copyText(full) }, { label: m.tree_copy_name(), run: () => copyText(p.name ?? '') }, 'separator', refresh(ctx, n)];
    }
    case 'object':
      return [...ddlEntries(ctx, n), 'separator', { label: m.tree_copy_name(), icon: ICON.copy, hint: p.name ?? '', run: () => copyText(p.name ?? '') }, { label: m.tree_copy_qualified(), run: () => copyText(qualifiedName(ctx.dialect, p.namespace ?? '', p.name ?? '', p.database)) }, 'separator', refresh(ctx, n)];
    case 'column':
      if (!sqlOn(ctx)) return [{ label: m.tree_copy_name(), icon: ICON.copy, hint: p.id ?? '', run: () => copyText(p.id ?? '') }, 'separator', refresh(ctx, n)];
      return [{ label: m.colinfo_open(), icon: ICON.columnInfo, shortcut: keys('Enter'), run: () => ctx.act.columnInfo(n) }, 'separator', { label: m.tree_copy_name(), icon: ICON.copy, hint: p.id ?? '', run: () => copyText(p.id ?? '') }, { label: m.tree_copy_qualified(), run: () => copyText(`${qualifiedName(ctx.dialect, p.namespace ?? '', p.name ?? '', p.database)}.${qualifiedName(ctx.dialect, '', p.id ?? '')}`) }, 'separator', refresh(ctx, n)];
    case 'index':
      return [...ddlEntries(ctx, n), 'separator', { label: m.tree_copy_name(), icon: ICON.copy, hint: p.id ?? '', run: () => copyText(p.id ?? '') }];
    case 'server':
      return serverMenu(ctx, n);
    case 'principal':
      return [
        { label: m.ex_permissions(), icon: ICON.permissions, run: () => ctx.act.user(p.name ?? '') },
        { label: m.tree_copy_name(), icon: ICON.copy, hint: p.name ?? '', run: () => copyText(p.name ?? '') },
      ];
    case 'session':
      return [
        { label: m.tool_kill_cancel(), icon: ICON.kill, run: () => ctx.act.kill(p.id ?? '', p.name, 'cancel') },
        { label: m.tool_kill_close(), icon: ICON.terminate, danger: true, run: () => ctx.act.kill(p.id ?? '', p.name, 'terminate') },
        'separator',
        { label: m.tool_copy_query(), icon: ICON.copy, disabled: !p.name, run: () => copyText(p.name ?? '') },
        { label: m.ex_open_activity(), icon: TOOL_ICON.activity, run: () => ctx.act.tool(null, 'activity') },
        { label: m.tool_monitor(), icon: TOOL_ICON.monitor, run: () => ctx.act.tool(null, 'monitor') },
      ];
    default:
      return [];
  }
}

function databaseMenu(ctx: MenuContext, n: ExNode): MenuEntry[] {
  const database = n.path.database ?? '';
  const shown = ctx.databaseShown(database);
  return [
    { label: m.ex_open_db(), icon: ICON.openDatabase, hint: m.tree_click(), run: () => ctx.act.activate(n) },
    { label: m.ex_query_here(), icon: ICON.newQuery, run: () => ctx.act.query(database, '') },
    'separator',
    toolOn(ctx, 'search') && { label: m.tool_search_dots(), icon: TOOL_ICON.search, run: () => ctx.act.tool(database, 'search') },
    toolOn(ctx, 'compare') && { label: m.tool_compare(), icon: TOOL_ICON.compare, run: () => ctx.act.tool(database, 'compare') },
    toolOn(ctx, 'backup') && { label: m.tool_backup(), icon: TOOL_ICON.backup, run: () => ctx.act.tool(database, 'backup') },
    toolOn(ctx, 'seclab') && { label: m.tool_seclab(), icon: TOOL_ICON.seclab, run: () => ctx.act.tool(database, 'seclab') },
    toolOn(ctx, 'space') && { label: m.tool_space(), icon: TOOL_ICON.space, run: () => ctx.act.tool(database, 'space') },
    toolOn(ctx, 'monitor') && { label: m.tool_monitor(), icon: TOOL_ICON.monitor, run: () => ctx.act.tool(database, 'monitor') },
    'separator',
    ...(sqlOn(ctx) ? maintenanceEntries(ctx, database, null) : []),
    'separator',
    { label: m.tree_copy_name(), icon: ICON.copy, hint: database, run: () => copyText(database) },
    shown ? { label: m.ex_hide_db(), icon: ICON.hide, disabled: n.active, run: () => ctx.act.hideDatabase(database, false) } : { label: m.ex_show_db(), icon: ICON.show, run: () => ctx.act.hideDatabase(database, true) },
    folding(ctx, n),
    refresh(ctx, n),
  ];
}

function relationIn(ctx: MenuContext, database: string, ns: string, name: string) {
  const space = ctx.schema(database).data?.namespaces.find((x) => x.name === ns);
  return [...(space?.tables ?? []), ...(space?.views ?? []), ...(space?.materialized_views ?? []), ...(space?.dictionaries ?? []), ...(space?.aliases ?? []), ...(space?.data_streams ?? [])].find((r) => r.name === name) ?? null;
}

function relationMenu(ctx: MenuContext, n: ExNode): MenuEntry[] {
  const p = n.path;
  const ns = p.namespace ?? '';
  const name = p.name ?? '';
  const database = p.database ?? '';
  const view = isViewGroup(p.group);
  const [showDdl, copyCreate] = ddlEntries(ctx, n);
  const caps = capsOf(ctx);
  if (!isSql(caps)) {
    const find = browseText(caps, name, () => '');
    const lang = caps.language.kind;
    const rel = lang === 'partiql' ? relationIn(ctx, database, ns, name) : null;
    return [
      { label: caps.words.table === 'key' ? m.tree_open_keys() : lang === 'cql' || lang === 'partiql' ? m.tree_open_data() : m.tree_open_documents(), icon: ICON.openData, hint: m.tree_click(), run: () => ctx.act.openData(p) },
      { label: m.tree_new_query_native(), icon: ICON.newQuery, run: () => ctx.act.query(database, find) },
      rel && { label: m.ddb_builder(), icon: ICON.newQuery, run: () => openDynamoBuilder({ connectionId: ctx.connectionId, database, rel, index: null }) },
      lang === 'partiql' && { label: m.ddb_scan(), icon: ICON.newQuery, run: () => ctx.act.query(database, dynamoScan(name)) },
      rel && { label: m.ddb_query_key(), icon: ICON.newQuery, run: () => ctx.act.query(database, dynamoQuery(rel)) },
      ...(rel?.indexes ?? []).map((ix) => ({ label: m.ddb_query_index({ index: ix.name }), icon: ICON.newQuery, run: () => ctx.act.query(database, dynamoQuery(rel as NonNullable<typeof rel>, ix.name)) })),
      caps.er && { label: m.tree_show_er(), icon: ICON.er, run: () => ctx.act.er(p) },
      caps.ddl && showDdl,
      'separator',
      { label: m.tree_copy_name(), icon: ICON.copy, hint: name, run: () => copyText(name) },
      { label: lang === 'mql' ? m.tree_copy_command() : m.tree_copy_browse(), run: () => copyText(find) },
      caps.ddl && copyCreate,
      'separator',
      { label: n.open ? m.tree_collapse() : m.tree_expand(), run: () => ctx.act.toggle(n) },
      refresh(ctx, n),
    ];
  }
  const select = selectSql(ctx.dialect, ns, name, database);
  return [
    { label: m.tree_open_data(), icon: ICON.openData, hint: m.tree_click(), run: () => ctx.act.openData(p) },
    { label: m.tree_new_query(), icon: ICON.newQuery, hint: m.tree_dblclick(), run: () => ctx.act.query(database, select) },
    caps.er && { label: m.tree_show_er(), icon: ICON.er, run: () => ctx.act.er(p) },
    showDdl,
    !view && caps.editable && { label: m.dz_edit_table_dots(), icon: ICON.design, run: () => void openDesigner(ctx.connectionId, database, ns, name) },
    ...sqlEntries(ctx.connectionId, subjectOf(ctx.dialect, database, ns, relationIn(ctx, database, ns, name), name, view)),
    'separator',
    { label: m.tree_copy_name(), icon: ICON.copy, hint: name, run: () => copyText(qualifiedName(ctx.dialect, ns, name, database)) },
    { label: m.tree_copy_select(), run: () => copyText(select) },
    copyCreate,
    'separator',
    { label: m.tool_export_data_dots(), icon: ICON.export, run: () => ctx.act.exportTable(p) },
    !view && hasTool(caps, 'transfer') && { label: m.tool_import_data_dots(), icon: ICON.import, run: () => ctx.act.importTable(p) },
    !view && hasTool(caps, 'generate') && { label: m.tool_generate_dots(), icon: TOOL_ICON.generate, run: () => (seedGenerate(ns, name), ctx.act.tool(database, 'generate')) },
    'separator',
    ...(view ? [] : maintenanceEntries(ctx, database, { namespace: ns, name })),
    'separator',
    { label: n.open ? m.tree_collapse() : m.tree_expand(), run: () => ctx.act.toggle(n) },
    refresh(ctx, n),
  ];
}

function serverMenu(ctx: MenuContext, n: ExNode): MenuEntry[] {
  const users = n.path.server === 'users';
  return [
    folding(ctx, n),
    { label: users ? m.ex_open_users() : m.ex_open_activity(), icon: users ? TOOL_ICON.users : TOOL_ICON.activity, run: () => ctx.act.tool(null, users ? 'users' : 'activity') },
    !users && { label: m.tool_monitor(), icon: TOOL_ICON.monitor, run: () => ctx.act.tool(null, 'monitor') },
    refresh(ctx, n),
  ];
}
