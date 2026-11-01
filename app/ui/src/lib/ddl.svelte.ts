import { m } from '../paraglide/messages';
import { api } from './api';
import type { Dialect } from './bindings';
import { copyText } from './contextmenu.svelte';
import type { ObjectRef } from './ddl';
import { queryOn } from './explorer.svelte';
import { showToast } from './toast.svelte';
import { ensureSession, sessionFor } from './workspace.svelte';

export interface DdlAsk {
  connectionId: string;
  database: string;
  dialect: Dialect | null;
  product: string;
  object: ObjectRef;
}

export const ddlUi = $state({ ask: null as DdlAsk | null });

export const showDdl = (ask: DdlAsk) => (ddlUi.ask = ask);

export async function loadDdl(connectionId: string, database: string, object: ObjectRef): Promise<string> {
  if (!(await ensureSession(connectionId, database))) throw new Error(m.dbs_open_failed());
  const s = sessionFor(connectionId, database);
  if (!s) throw new Error(m.dbs_open_failed());
  return api.objectDdl(s.info.session_id, object);
}

export async function copyDdl(connectionId: string, database: string, object: ObjectRef) {
  try {
    await copyText(await loadDdl(connectionId, database, object));
  } catch (e) {
    showToast(m.ddl_failed({ name: object.name, reason: e instanceof Error ? e.message : String(e) }), null, 'error');
  }
}

export const openDdl = (connectionId: string, database: string, sql: string) => queryOn(connectionId, database, sql, false);
