import type { QueryProgress_Serialize as QueryProgress } from './bindings';

export function share(p: QueryProgress): number | null {
  if (!p.total_rows || p.total_rows <= 0) return null;
  return Math.min(1, Math.max(0, (p.read_rows ?? 0) / p.total_rows));
}

export function liveProgress<T extends { done: unknown; error: unknown; progress: QueryProgress | null }>(results: readonly T[]): T | null {
  for (let i = results.length - 1; i >= 0; i--) {
    const r = results[i];
    if (r && !r.done && !r.error && r.progress) return r;
  }
  return null;
}
