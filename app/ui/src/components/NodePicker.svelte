<script lang="ts">
  import { Folder } from '@lucide/svelte';
  import { connections } from '../lib/connections.svelte';
  import { driverOf } from '../lib/drivers';
  import { markIcon } from '../lib/folder-look';
  import { org } from '../lib/org.svelte';
  import { buildRows, covered, type Row } from '../lib/tree';
  import { m } from '../paraglide/messages';
  import DriverMark from './DriverMark.svelte';

  let { folders = $bindable(), conns = $bindable(), label }: { folders: string[]; conns: string[]; label: string } = $props();

  const rows = $derived(buildRows(org.folders.map((f) => ({ ...f, collapsed: false })), connections.list));
  const inside = $derived(covered(org.folders, connections.list, folders));

  const checked = (r: Row) => (r.kind === 'folder' ? folders.includes(r.folder.id) || inside.folders.has(r.folder.id) : conns.includes(r.conn.id) || inside.connections.has(r.conn.id));
  const implied = (r: Row) => (r.kind === 'folder' ? inside.folders.has(r.folder.id) : inside.connections.has(r.conn.id));

  function toggle(r: Row) {
    if (implied(r)) return;
    if (r.kind === 'connection') {
      conns = conns.includes(r.conn.id) ? conns.filter((x) => x !== r.conn.id) : [...conns, r.conn.id];
      return;
    }
    const id = r.folder.id;
    if (folders.includes(id)) {
      folders = folders.filter((x) => x !== id);
      return;
    }
    const next = [...folders, id];
    const below = covered(org.folders, connections.list, next);
    folders = next.filter((x) => !below.folders.has(x));
    conns = conns.filter((x) => !below.connections.has(x));
  }

  const allIds = $derived({ folders: org.folders.filter((f) => !f.parent).map((f) => f.id), conns: connections.list.filter((c) => !c.folder).map((c) => c.id) });
  const everything = $derived(allIds.folders.every((f) => folders.includes(f)) && allIds.conns.every((c) => conns.includes(c)));

  function toggleAll() {
    if (everything) {
      folders = [];
      conns = [];
    } else {
      folders = [...allIds.folders];
      conns = [...allIds.conns];
    }
  }
</script>

<div class="node-picker" role="group" aria-label={label}>
  <label class="check pick-all">
    <input type="checkbox" checked={everything} onchange={toggleAll} />
    <span>{m.org_pick_all()}</span>
  </label>
  <div class="pick-rows">
    {#each rows as r (r.kind === 'folder' ? `f:${r.folder.id}` : `c:${r.conn.id}`)}
      <label class="check pick-row" class:implied={implied(r)} style:--depth={r.depth}>
        <input type="checkbox" checked={checked(r)} disabled={implied(r)} onchange={() => toggle(r)} />
        {#if r.kind === 'folder'}
          {@const Mark = markIcon(r.folder.mark)}
          <span class="pick-icon" data-tint={r.folder.tint ?? undefined}>{#if Mark}<Mark size={14} strokeWidth={1.7} />{:else}<Folder size={14} strokeWidth={1.7} />{/if}</span>
          <span class="pick-name">{r.folder.name}</span>
          <span class="pick-count">{r.count}</span>
        {:else}
          {@const conn = r.conn}
          <span class="pick-icon"><DriverMark driver={driverOf(conn.target)} size={14} /></span>
          <span class="pick-name">{conn.name}</span>
        {/if}
      </label>
    {/each}
  </div>
</div>
