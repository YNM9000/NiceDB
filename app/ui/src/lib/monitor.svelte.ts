import { sqlOf } from './family';
import { m } from '../paraglide/messages';
import { api } from './api';
import type { Maintenance } from './bindings';
import { tabOn } from './explorer.svelte';
import { DEFAULT_INTERVAL, explainSql, hasPlaceholders, INTERVALS, MAINTENANCE_LABEL, maintenanceSql } from './monitor';
import { prefs, updateSection } from './prefs.svelte';
import { explain } from './plan.svelte';
import { qualifiedName } from './sqlname';
import { showToast } from './toast.svelte';
import { duration, flavorOf, type Flavor } from './tools';
import { confirmToolWrite, errorText } from './tools.svelte';
import { newTab, sessionFor, workspace } from './workspace.svelte';

export const monitorUi = {
  get interval(): number {
    const ms = prefs.value.results.monitor_ms;
    return (INTERVALS as readonly number[]).includes(ms) ? ms : DEFAULT_INTERVAL;
  },
  set interval(ms: number) {
    void updateSection('results', { monitor_ms: ms });
  },
};

export function explainQuery(connectionId: string, raw: string, flavor: Flavor | null = null) {
  const sql = flavor ? explainSql(flavor, raw) : raw;
  const tab = newTab(connectionId, sql);
  if (hasPlaceholders(sql, flavor)) {
    showToast(m.mon_explain_params());
    return;
  }
  workspace.split = Math.min(workspace.split, 0.24);
  void explain(tab, { kind: 'statement', cursor: 0 }, 'estimate');
}

export async function maintain(connectionId: string, database: string | null, action: Maintenance, table: { namespace: string; name: string } | null) {
  if (database && !sessionFor(connectionId, database)) await tabOn(connectionId, database);
  const s = sessionFor(connectionId, database);
  if (!s) return;
  const flavor = flavorOf(sqlOf(s.info), s.info.server.product);
  const name = table ? qualifiedName(sqlOf(s.info), table.namespace, table.name) : null;
  const sql = maintenanceSql(flavor, action, name);
  const label = MAINTENANCE_LABEL[action]();
  const target = table?.name ?? database ?? s.info.server.database ?? '';
  try {
    const typed = await confirmToolWrite(s.info, m.mon_maintain_title({ action: label, target }), [sql], label, m.mon_maintain_note());
    if (typed === false) return;
    showToast(m.mon_maintain_running({ sql }), null, 'info', 60_000);
    const done = await api.serverMaintain({ session_id: s.info.session_id, action, table, confirm: typed });
    const said = done.messages.filter((x) => !/^status: ok$/i.test(x));
    const tail = said.length ? ` ${said.join(' · ')}` : '';
    showToast(`${m.mon_maintain_done({ sql: done.sql, time: duration(done.elapsed_ms) })}${tail}`);
  } catch (e) {
    showToast(errorText(e), null, 'error');
  }
}
