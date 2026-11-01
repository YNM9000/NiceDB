import type { RunEvent, RunRequest } from './api';
import type { SortKey, Unsortable } from './bindings';

type Run = (req: RunRequest, emit: (e: RunEvent) => void) => Promise<void>;

function unsortable(sql: string): Unsortable | null {
  const s = sql.trim().replace(/^\(+/, '').toLowerCase();
  if (!/^(select|with)\b/.test(s)) return 'not_select';
  if (/\binto\b|\bnextval\s*\(/.test(s) || (s.startsWith('with') && /\b(insert|update|delete)\b/.test(s))) return 'writes';
  return null;
}

function compare(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  const x = Number(a);
  const y = Number(b);
  return Number.isFinite(x) && Number.isFinite(y) && a.trim() !== '' && b.trim() !== '' ? x - y : a.localeCompare(b, 'it');
}

export function sortingRun(run: Run): Run {
  return async (req, emit) => {
    const keys: readonly SortKey[] = req.sort ?? [];
    if (!keys.length) return run(req, (e) => emit(e.type === 'statement' ? { ...e, unsortable: unsortable(e.sql) } : e));
    const rows: (string | null)[][] = [];
    let done: RunEvent | null = null;
    await run(req, (e) => {
      if (e.type === 'rows') rows.push(...e.rows);
      else if (e.type === 'done') done = e;
      else if (e.type === 'finished') {
        rows.sort((a, b) => {
          for (const k of keys) {
            const d = compare(a[k.column] ?? null, b[k.column] ?? null);
            if (d) return k.descending ? -d : d;
          }
          return 0;
        });
        for (let i = 0; i < rows.length; i += 4000) emit({ type: 'rows', index: 0, rows: rows.slice(i, i + 4000) });
        if (done) emit(done);
        emit(e);
      } else if (e.type === 'statement') emit({ ...e, unsortable: null });
      else emit(e);
    });
  };
}

export function previewScale(percent: number | undefined) {
  if (typeof document === 'undefined') return;
  const zoom = percent && percent !== 100 ? String(percent / 100) : '';
  document.documentElement.style.zoom = zoom;
  document.documentElement.style.setProperty('--nd-preview-zoom', zoom || '1');
}
