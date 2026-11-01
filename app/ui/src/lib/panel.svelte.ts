import type { ParamValue } from './bindings';
import { workspace } from './workspace.svelte';

export type PanelTab = 'notes' | 'scripts' | 'snippets';

export interface Saving {
  sql: string;
  connectionId: string;
  edit: string | null;
  params: ParamValue[];
}

export const panel = $state({ open: false, tab: 'notes' as PanelTab, saving: null as Saving | null });

function shut() {
  panel.open = false;
}

export function togglePanel(tab?: PanelTab) {
  if (panel.open && (!tab || tab === panel.tab)) return shut();
  if (tab) panel.tab = tab;
  panel.open = true;
  workspace.historyOpen = false;
}

export function toggleHistory() {
  workspace.historyOpen = !workspace.historyOpen;
  if (workspace.historyOpen) panel.open = false;
}

export const openSave = (sql: string, connectionId: string, edit: string | null = null, params: ParamValue[] = []) => (panel.saving = { sql, connectionId, edit, params });
