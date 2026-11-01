import { m } from '../paraglide/messages';
import { api, type SessionInfo } from './api';
import type { DocumentWrite, Outcome } from './bindings';
import { connections } from './connections.svelte';
import { askConfirm, writesOpen } from './guard.svelte';
import { play } from './sound';

export const docEdit = $state({ open: null as null | { title: string; text: string; id: string | null; index: number | null }, busy: false, error: null as string | null });

export type Done = { ok: true; outcome: Outcome } | { ok: false; message: string };

export async function writeDocuments(session: SessionInfo, write: DocumentWrite, lines: string[]): Promise<Done | null> {
  let confirm: string | null = null;
  if (session.environment === 'production') {
    if (!writesOpen(session.session_id)) return { ok: false, message: m.data_ro_locked() };
    const typed = await askConfirm({
      title: m.doc_confirm_title({ collection: write.collection }),
      connection: connections.list.find((c) => c.id === session.connection_id)?.name ?? '',
      production: true,
      lines,
      risks: [],
      note: '',
      action: m.confirm_apply_btn(),
    });
    if (typed === false) return null;
    confirm = typed;
  }
  try {
    const outcome = await api.documentsWrite(session.session_id, write, confirm);
    play('applied');
    return { ok: true, outcome };
  } catch (e) {
    return { ok: false, message: e instanceof Error ? e.message : String(e) };
  }
}
