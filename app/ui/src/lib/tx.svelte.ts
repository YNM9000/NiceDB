import { m } from '../paraglide/messages';
import { api, ApiError, inTauri, type CommitMode, type SessionInfo, type TxState } from './api';
import { connections } from './connections.svelte';
import { askConfirm } from './guard.svelte';
import { play } from './sound';
import { showToast } from './toast.svelte';
import { pending, txSummary } from './tx';

export type CloseReason = 'tab' | 'space' | 'app';

export interface Waiting {
  sessionId: string;
  connection: string;
  database: string;
  state: TxState;
}

interface CloseAsk {
  reason: CloseReason;
  list: Waiting[];
  resolve: (choice: 'commit' | 'rollback' | false) => void;
}

export const txs = $state({
  state: {} as Record<string, TxState & { since: number | null }>,
  busy: {} as Record<string, boolean>,
  last: {} as Record<string, 'committed' | 'rolled_back'>,
  now: Date.now(),
  ask: null as CloseAsk | null,
});

const sessions = new Map<string, SessionInfo>();
let ticker: ReturnType<typeof setInterval> | null = null;

function tick() {
  const open = Object.values(txs.state).some((s) => s.open);
  if (open && !ticker) ticker = setInterval(() => (txs.now = Date.now()), 1000);
  if (!open && ticker) {
    clearInterval(ticker);
    ticker = null;
  }
}

export function setTx(sessionId: string, s: TxState) {
  const was = txs.state[sessionId];
  const since = s.open ? (was?.open && was.since !== null ? was.since : Date.now() - (s.open_ms ?? 0)) : null;
  if (s.open) delete txs.last[sessionId];
  txs.state[sessionId] = { ...s, since };
  txs.now = Date.now();
  tick();
}

export const txOf = (sessionId: string | null | undefined) => (sessionId ? (txs.state[sessionId] ?? null) : null);

export const openFor = (sessionId: string) => {
  const s = txs.state[sessionId];
  return s?.open && s.since !== null ? Math.max(0, txs.now - s.since) : 0;
};

export function trackSession(info: SessionInfo) {
  sessions.set(info.session_id, info);
}

export async function refreshTx(sessionId: string) {
  try {
    setTx(sessionId, await api.txState(sessionId));
  } catch {
    forgetTx(sessionId);
  }
}

export function forgetTx(sessionId: string) {
  delete txs.state[sessionId];
  sessions.delete(sessionId);
  tick();
}

export function txGone(sessionId: string) {
  const s = txs.state[sessionId];
  if (s) setTx(sessionId, { ...s, open: false, writes: 0, rows: 0, open_ms: null, failed: false, dry_run: false });
}

export async function setMode(sessionId: string, mode: CommitMode) {
  setTx(sessionId, await api.txModeSet(sessionId, mode));
}

const nameOf = (info: SessionInfo | undefined) => connections.list.find((c) => c.id === info?.connection_id)?.name ?? '';

async function act(sessionId: string, end: 'committed' | 'rolled_back', work: () => Promise<TxState>): Promise<boolean> {
  if (txs.busy[sessionId]) return false;
  txs.busy[sessionId] = true;
  try {
    const was = txs.state[sessionId]?.open;
    const now = await work();
    setTx(sessionId, now);
    if (was && !now.open) txs.last[sessionId] = end;
    return true;
  } catch (e) {
    void refreshTx(sessionId);
    const read = e instanceof ApiError && e.error.code === 'read_only';
    showToast(read ? m.tx_commit_locked() : m.tx_close_failed({ message: e instanceof Error ? e.message : String(e) }), null, 'error');
    return false;
  } finally {
    delete txs.busy[sessionId];
  }
}

export function commitTx(sessionId: string): Promise<boolean> {
  return act(sessionId, 'committed', async () => {
    try {
      const out = await api.txCommit(sessionId, null);
      play('applied');
      return out;
    } catch (e) {
      if (!(e instanceof ApiError) || e.error.code !== 'needs_confirmation') throw e;
      const info = sessions.get(sessionId);
      const s = txs.state[sessionId];
      const typed = await askConfirm({
        title: m.tx_commit_title(),
        connection: nameOf(info),
        production: true,
        lines: s ? [txSummary(s)] : [],
        risks: [],
        note: m.tx_commit_note(),
        action: m.tx_commit_btn(),
      });
      if (typed === false) return api.txState(sessionId);
      const out = await api.txCommit(sessionId, typed);
      play('applied');
      return out;
    }
  });
}

export function rollbackTx(sessionId: string): Promise<boolean> {
  return act(sessionId, 'rolled_back', async () => {
    const out = await api.txRollback(sessionId);
    return out;
  });
}

export function waiting(ids: Iterable<string>, describe: (sessionId: string) => { connection: string; database: string }): Waiting[] {
  return [...new Set(ids)].flatMap((sessionId) => {
    const state = txs.state[sessionId];
    return state && pending(state) ? [{ sessionId, state, ...describe(sessionId) }] : [];
  });
}

export async function settleBeforeClosing(list: Waiting[], reason: CloseReason): Promise<boolean> {
  if (!list.length) return true;
  txs.ask?.resolve(false);
  const choice = await new Promise<'commit' | 'rollback' | false>((resolve) => {
    txs.ask = {
      reason,
      list,
      resolve: (v) => {
        txs.ask = null;
        resolve(v);
      },
    };
  });
  if (!choice) return false;
  for (const w of list) {
    const ok = choice === 'commit' ? await commitTx(w.sessionId) : await rollbackTx(w.sessionId);
    if (!ok || txs.state[w.sessionId]?.open) return false;
  }
  return true;
}

export function guardWindowClose(list: () => Waiting[]) {
  if (!inTauri) return;
  void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
    const win = getCurrentWindow();
    void win.onCloseRequested(async (e) => {
      const open = list();
      if (!open.length) return;
      e.preventDefault();
      if (await settleBeforeClosing(open, 'app')) await win.destroy();
    });
  });
}
