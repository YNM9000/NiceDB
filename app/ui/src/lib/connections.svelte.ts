import { m } from '../paraglide/messages';
import { api, ApiError, type ConnectionProfile, type Secrets } from './api';
import { askConfirm } from './guard.svelte';
import { record } from './undo.svelte';

export type EditMode = 'edit' | 'copy' | 'delete' | 'follow';

export const connections = $state({ list: [] as ConnectionProfile[], loaded: false });

export async function loadConnections() {
  connections.list = await api.connectionsList();
  connections.loaded = true;
}

const keep: Secrets = { password: null, ssh: null };

const ENV_LABEL = { local: m.env_local, development: m.env_development, staging: m.env_staging, production: m.env_production };

async function saveThroughGate(profile: ConnectionProfile, secrets: Secrets, name: string): Promise<boolean> {
  try {
    await api.connectionSave(profile, secrets);
    return true;
  } catch (e) {
    if (!(e instanceof ApiError) || e.error.code !== 'needs_confirmation') throw e;
    const typed = await askConfirm({
      title: m.conn_downgrade_title(),
      connection: name,
      production: true,
      lines: [m.conn_downgrade_line({ env: ENV_LABEL[profile.environment]() })],
      risks: [],
      note: m.conn_downgrade_note(),
      action: m.conn_downgrade_btn(),
    });
    if (typed === false) return false;
    await api.connectionSave(profile, secrets, typed);
    return true;
  }
}

export async function saveConnection(profile: ConnectionProfile, secrets: Secrets): Promise<boolean> {
  const before = connections.list.find((c) => c.id === profile.id);
  const previous = before ? $state.snapshot(before) : null;
  if (!(await saveThroughGate(profile, secrets, previous?.name ?? profile.name))) return false;
  await loadConnections();
  if (!previous || secrets.password !== null || secrets.ssh !== null) return true;
  const after = $state.snapshot(profile);
  const restore = async (p: ConnectionProfile) => {
    const now = connections.list.find((c) => c.id === p.id)?.name ?? p.name;
    await saveThroughGate(p, keep, now);
    await loadConnections();
  };
  record({
    label: previous.name === after.name ? m.undo_conn_edited() : m.undo_conn_renamed(),
    undo: () => restore(previous),
    redo: () => restore(after),
    valid: () => connections.list.some((c) => c.id === after.id),
  });
  return true;
}
