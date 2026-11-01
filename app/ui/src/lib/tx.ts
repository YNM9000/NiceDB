import { m } from '../paraglide/messages';
import type { CommitMode, RowPreview, TxState } from './api';
import { num } from './format';

export const MODES: readonly CommitMode[] = ['auto', 'manual', 'smart'];

export const modeLabel = (mode: CommitMode) => ({ auto: m.tx_mode_auto, manual: m.tx_mode_manual, smart: m.tx_mode_smart })[mode]();

export const modeHint = (mode: CommitMode) => ({ auto: m.tx_mode_auto_hint, manual: m.tx_mode_manual_hint, smart: m.tx_mode_smart_hint })[mode]();

export const modeTag = (mode: CommitMode) => ({ auto: m.tx_mode_auto_tag, manual: m.tx_mode_manual_tag, smart: m.tx_mode_smart_tag })[mode]();

export const pending = (s: TxState | null | undefined) => !!s?.open && (s.writes > 0 || s.dry_run);

export function txSummary(s: TxState): string {
  if (s.failed) return m.tx_failed_short();
  if (s.writes === 0) return m.tx_no_writes();
  const writes = s.writes === 1 ? m.tx_writes_one() : m.tx_writes_other({ count: num(s.writes) });
  const rows = s.rows ?? 0;
  return rows ? `${writes} · ${rows === 1 ? m.tx_rows_one() : m.tx_rows_other({ count: num(rows) })}` : writes;
}

export interface PreviewCell {
  before: string | null;
  after: string | null;
  changed: boolean;
}

export function previewRows(p: RowPreview): PreviewCell[][] {
  const assigned = new Set(p.changed);
  return p.before.map((row, r) =>
    row.map((before, c) => {
      const after = p.kind === 'update' ? (p.after[r]?.[c] ?? null) : null;
      return { before, after, changed: p.kind === 'update' && assigned.has(c) && after !== before };
    }),
  );
}

export const changedCells = (p: RowPreview) => previewRows(p).reduce((n, row) => n + row.filter((c) => c.changed).length, 0);
