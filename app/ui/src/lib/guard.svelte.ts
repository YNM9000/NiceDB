import { m } from '../paraglide/messages';
import { api, type RiskFound, type WriteAccess } from './api';
import type { UnlockMode } from './bindings';
import { prefs } from './prefs.svelte';
import { asksForConfirmation } from './prodsafety';
import { play } from './sound';

interface Access {
  guarded: boolean;
  open: boolean;
  mode: UnlockMode;
  until: number | null;
  closing?: boolean;
  note?: string | null;
}

export const guard = $state({ access: {} as Record<string, Access>, now: Date.now() });

let ticker: ReturnType<typeof setInterval> | null = null;

function set(sessionId: string, a: WriteAccess) {
  const note = guard.access[sessionId]?.note ?? null;
  guard.access[sessionId] = { guarded: a.guarded, open: a.open, mode: a.mode, until: a.open_ms === null ? null : Date.now() + a.open_ms, note };
  guard.now = Date.now();
  tick();
}

function tick() {
  const open = Object.values(guard.access).some((a) => a.until !== null);
  if (open && !ticker) ticker = setInterval(expire, 1000);
  if (!open && ticker) {
    clearInterval(ticker);
    ticker = null;
  }
}

function expire() {
  guard.now = Date.now();
  for (const [id, a] of Object.entries(guard.access)) {
    if (a.until !== null && a.until <= guard.now) {
      a.until = null;
      a.open = false;
      a.closing = true;
      void lock(id).catch(() => {});
    }
  }
  tick();
}

export const remaining = (sessionId: string) => {
  const until = guard.access[sessionId]?.until;
  return until ? Math.max(0, until - guard.now) : 0;
};

export const writesOpen = (sessionId: string) => {
  const a = guard.access[sessionId];
  return !!a?.open && (a.until === null || a.until > guard.now);
};

export const alwaysOpen = (sessionId: string) => !!guard.access[sessionId]?.guarded && guard.access[sessionId]?.mode === 'open' && writesOpen(sessionId);

export const clock = (ms: number) => {
  const s = Math.ceil(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};

export async function refreshAccess(sessionId: string) {
  set(sessionId, await api.writeAccess(sessionId));
}

export const refreshAll = () => Promise.all(Object.keys(guard.access).map((id) => refreshAccess(id).catch(() => {})));

const NOTE_MS = 12_000;

export async function settle(sessionId: string, relocked = false) {
  const was = guard.access[sessionId]?.closing ?? false;
  await refreshAccess(sessionId).catch(() => {});
  const a = guard.access[sessionId];
  if (!a || (!relocked && !(was && a.until === null))) return;
  const note = m.guard_relocked();
  a.note = note;
  setTimeout(() => {
    const now = guard.access[sessionId];
    if (now?.note === note) now.note = null;
  }, NOTE_MS);
}

export async function unlock(sessionId: string) {
  set(sessionId, await api.writesUnlock(sessionId));
  play('unlock');
}

export async function lock(sessionId: string) {
  set(sessionId, await api.writesLock(sessionId));
  play('lock');
}

export interface ConfirmRequest {
  title: string;
  connection: string;
  production: boolean;
  lines: string[];
  risks: RiskFound[];
  rows?: string | null;
  stale?: string | null;
  note: string;
  action: string;
}

export const confirmState = $state({ req: null as (ConfirmRequest & { resolve: (name: string | null | false) => void }) | null });

export function askConfirm(req: ConfirmRequest): Promise<string | null | false> {
  if (!asksForConfirmation(prefs.value.safety.confirm_mode, req.production)) return Promise.resolve(null);
  confirmState.req?.resolve(false);
  play('warn');
  return new Promise((resolve) => {
    confirmState.req = {
      ...req,
      resolve: (v) => {
        confirmState.req = null;
        resolve(v);
      },
    };
  });
}
