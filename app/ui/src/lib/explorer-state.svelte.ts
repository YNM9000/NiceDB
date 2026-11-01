import type { Activity, Principals } from './api';
import { idle, type Loaded } from './explorer';

interface ServerState {
  users: Loaded<Principals>;
  sessions: Loaded<Activity>;
}

export const explorer = $state({
  open: {} as Record<string, boolean>,
  seeded: {} as Record<string, boolean>,
  wanted: {} as Record<string, string[]>,
  failures: {} as Record<string, string>,
  server: {} as Record<string, ServerState>,
  counts: {} as Record<string, Record<string, number> | null>,
  filter: '',
});

export const usersFocus = $state({ connectionId: '', key: '' });

const NONE: ServerState = { users: idle(), sessions: idle() };

export const serverOf = (connectionId: string): ServerState => explorer.server[connectionId] ?? NONE;

export function writableServer(connectionId: string): ServerState {
  if (!explorer.server[connectionId]) explorer.server[connectionId] = { users: idle(), sessions: idle() };
  return explorer.server[connectionId] as ServerState;
}

export const isWanted = (connectionId: string, database: string) => explorer.wanted[connectionId]?.includes(database) ?? false;

export function want(connectionId: string, database: string, on: boolean) {
  const list = (explorer.wanted[connectionId] ?? []).filter((d) => d !== database);
  explorer.wanted[connectionId] = on ? [...list, database] : list;
}

export function forgetExplorer(connectionId: string) {
  const prefix = `${connectionId}\u001f`;
  for (const k of Object.keys(explorer.open)) if (k.startsWith(prefix)) delete explorer.open[k];
  for (const k of Object.keys(explorer.failures)) if (k.startsWith(prefix)) delete explorer.failures[k];
  for (const k of Object.keys(explorer.seeded)) if (k.startsWith(prefix)) delete explorer.seeded[k];
  delete explorer.wanted[connectionId];
  delete explorer.server[connectionId];
  delete explorer.counts[connectionId];
}
