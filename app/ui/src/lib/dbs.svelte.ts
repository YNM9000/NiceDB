import { api, type Visibility } from './api';
import { forgetExplorer } from './explorer-state.svelte';
import { emptyVisibility } from './visibility';

export const dbs = $state({
  visibility: {} as Record<string, Visibility>,
  showAll: {} as Record<string, boolean>,
});

export const visibilityOf = (connectionId: string): Visibility => dbs.visibility[connectionId] ?? emptyVisibility();

export async function loadVisibility(connectionId: string) {
  dbs.visibility[connectionId] = await api.visibilityGet(connectionId).catch(emptyVisibility);
}

export async function saveVisibility(connectionId: string, v: Visibility) {
  dbs.visibility[connectionId] = v;
  await api.visibilitySet(connectionId, $state.snapshot(v));
}

export function forget(connectionId: string) {
  delete dbs.visibility[connectionId];
  delete dbs.showAll[connectionId];
  forgetExplorer(connectionId);
}
