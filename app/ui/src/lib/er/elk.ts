import type { ELK as Elk, ElkNode } from 'elkjs/lib/elk-api';
import { applyElk, toElk } from './layout';
import type { ErGraph } from './model';

let instance: Promise<Elk> | null = null;

function elk(): Promise<Elk> {
  instance ??= Promise.all([import('elkjs/lib/elk-api.js'), import('elkjs/lib/elk-worker.min.js?url')]).then(
    ([api, url]) => new api.default({ workerFactory: () => new Worker(url.default) }),
  );
  return instance;
}

export async function autoLayout(g: ErGraph, aspect?: number): Promise<void> {
  const result: ElkNode = await (await elk()).layout(toElk(g, aspect));
  applyElk(g, result);
}
