import { ApiError, api, type Page } from './api';
import type { QuickFilter } from './bindings';
import type { TableInfo } from './data.svelte';

const chains = new Map<string, Promise<unknown>>();
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function patient<T>(job: () => Promise<T>): Promise<T> {
  for (let i = 0; ; i++) {
    try {
      return await job();
    } catch (e) {
      if (!(e instanceof ApiError && e.error.code === 'busy') || i >= 5) throw e;
      await sleep(200 * (i + 1));
    }
  }
}

export class Stale extends Error {}

export function queued<T>(sessionId: string, job: () => Promise<T>, stale?: () => boolean): Promise<T> {
  const next = (chains.get(sessionId) ?? Promise.resolve())
    .catch(() => undefined)
    .then(() => {
      if (stale?.()) throw new Stale();
      return patient(job);
    });
  chains.set(sessionId, next);
  return next;
}

export function rowsOf(sessionId: string, info: TableInfo, columns: string[], quick: QuickFilter[], search: string | null, limit: number, stale?: () => boolean, jobId?: string): Promise<Page> {
  const browse = {
      table: { namespace: info.namespace, name: info.name },
      columns,
      key: info.key.map((i) => info.columns[i]?.name ?? ''),
      filter: null,
      quick,
      order: [],
      limit,
      offset: 0,
      after: null,
      search,
    };
  const page = () => (jobId ? api.dataPick(sessionId, browse, jobId) : api.dataBrowse(sessionId, browse));
  return queued(sessionId, page, stale);
}
