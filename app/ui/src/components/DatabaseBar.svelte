<script lang="ts">
  import { Check, ChevronDown, Database, RefreshCw, Settings2 } from '@lucide/svelte';
  import { dbs, saveVisibility, visibilityOf } from '../lib/dbs.svelte';
  import { databaseOf, useDatabase, workspace, type Tab } from '../lib/workspace.svelte';
  import { databaseShown, onlyUsed, schemaShown, setDatabase, setSchema, shownDatabases, emptyVisibility } from '../lib/visibility';
  import type { Schema } from '../lib/api';
  import { m } from '../paraglide/messages';
  import Popover from './Popover.svelte';

  interface Props {
    tab: Tab;
    schema: Schema | null;
    onrefresh: () => void;
  }
  let { tab, schema, onrefresh }: Props = $props();
  let listOpen = $state(false);
  let panelOpen = $state(false);
  let query = $state('');
  let target = $state<string | null>(null);
  let failure = $state<string | null>(null);

  const id = $derived(tab.connectionId);
  const vis = $derived(visibilityOf(id));
  const showAll = $derived(!!dbs.showAll[id]);
  const current = $derived(databaseOf(tab) ?? schema?.current_database ?? '');
  const all = $derived(schema?.databases ?? []);
  const listed = $derived(shownDatabases(vis, all, current, showAll));
  const multiple = $derived(all.length > 1);
  const configurable = $derived(multiple || (schema?.namespaces.length ?? 0) > 1);
  const q = $derived(query.trim().toLowerCase());
  const used = $derived(workspace.tabs.filter((t) => t.connectionId === id).map((t) => databaseOf(t) ?? ''));

  async function pick(name: string) {
    listOpen = false;
    if (name === current) return;
    target = name;
    failure = await useDatabase(tab, name);
    target = null;
  }

  const save = (next: ReturnType<typeof visibilityOf>) => void saveVisibility(id, next).catch(() => {});
</script>

<div class="dbs-bar">
  {#if multiple}
    <Popover label={m.dbs_switch()} bind:open={listOpen} class="dbs-trigger" span=".dbs-bar">
      {#snippet trigger()}
        <Database size={13} strokeWidth={1.7} aria-hidden="true" />
        <span class="dbs-name">{target ? m.dbs_switching({ database: target }) : current}</span>
        <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
      {/snippet}
      <ul class="dbs-list" role="listbox" aria-label={m.dbs_switch()}>
        {#each listed as name (name)}
          <li role="option" aria-selected={name === current}>
            <button type="button" class:hidden-db={!databaseShown(vis, name)} onclick={() => void pick(name)}>
              <span class="dbs-check">{#if name === current}<Check size={12} strokeWidth={2.2} aria-hidden="true" />{/if}</span>
              <span class="tree-name">{name}</span>
              {#if used.includes(name) && name !== current}<span class="tree-count">{m.dbs_in_use()}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </Popover>
  {:else}
    <h2 class="caps dbs-single"><Database size={13} strokeWidth={1.7} aria-hidden="true" /> {current}</h2>
  {/if}
  <span class="spacer"></span>
  {#if configurable}
    <Popover label={m.dbs_settings()} bind:open={panelOpen} class="icon-btn dbs-settings" span=".dbs-bar">
      {#snippet trigger()}<Settings2 size={13} strokeWidth={1.8} />{/snippet}
      <label class="field">
        <input type="search" placeholder={m.dbs_search()} bind:value={query} aria-label={m.dbs_search()} />
      </label>
      <div class="dbs-scroll">
        {#if multiple}
          {@const dbList = all.filter((d) => !q || d.toLowerCase().includes(q))}
          <h3 class="dbs-title">{m.dbs_databases()}</h3>
          {#each dbList as d (d)}
            <label class="check dbs-check-row">
              <input type="checkbox" checked={databaseShown(vis, d) || d === current} disabled={d === current} onchange={(e) => save(setDatabase(vis, d, e.currentTarget.checked))} />
              <span class="tree-name">{d}</span>
            </label>
          {/each}
          {#if !dbList.length}<p class="hint">{m.dbs_none()}</p>{/if}
        {/if}
        {#if schema}
          {@const nsList = schema.namespaces.filter((n) => !q || n.name.toLowerCase().includes(q))}
          <h3 class="dbs-title">{m.dbs_schemas({ database: current })}</h3>
          {#each nsList as n (n.name)}
            <label class="check dbs-check-row">
              <input type="checkbox" checked={schemaShown(vis, current, n.name)} onchange={(e) => save(setSchema(vis, current, n.name, e.currentTarget.checked))} />
              <span class="tree-name">{n.name}</span>
              <span class="tree-count">{n.tables.length + n.views.length}</span>
            </label>
          {/each}
          {#if !nsList.length}<p class="hint">{m.dbs_none()}</p>{/if}
        {/if}
      </div>
      <div class="dbs-foot">
        <button class="btn ghost" type="button" onclick={() => schema && save(onlyUsed(vis, schema, used))}>{m.dbs_only_used()}</button>
        <button class="btn ghost" type="button" onclick={() => save(emptyVisibility())}>{m.dbs_reset()}</button>
      </div>
    </Popover>
  {/if}
  <button class="icon-btn" type="button" aria-label={m.schema_refresh()} onclick={onrefresh}><RefreshCw size={13} strokeWidth={1.8} /></button>
</div>
{#if failure}<p class="hint schema-msg err" role="alert">{failure}</p>{/if}
