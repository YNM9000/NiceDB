import { m } from '../paraglide/messages';
import type { ConnectionProfile } from './api';
import { connections } from './connections.svelte';
import { orgUi, spaces } from './org.svelte';
import type { Command } from './palette.svelte';
import { isModel } from './profile-template';

export function profileCommands(fromModel: (p: ConnectionProfile) => void): Command[] {
  const app = m.pal_group_app;
  return [
    ...connections.list.filter(isModel).map((p) => ({ id: `conn.from_model.${p.id}`, label: () => m.pal_cmd_from_model({ name: p.name }), group: app, run: () => fromModel(p) })),
    { id: 'space.vars', label: m.pal_cmd_space_vars, group: app, when: () => spaces.active !== null, run: () => (orgUi.space = { id: spaces.active, preset: null, vars: true }) },
  ];
}
