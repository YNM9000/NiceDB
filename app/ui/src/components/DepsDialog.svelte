<script lang="ts">
  import { ArrowLeft, ArrowRight, Eye, FileCode, Hash, Sigma, Table2, TableProperties, Zap } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { showDdl } from '../lib/ddl.svelte';
  import { openRelation } from '../lib/explorer.svelte';
  import { groupDeps, readFromText, viaLabel, type Dependency } from '../lib/sqlgen';
  import { depsBack, depsUi, loadDeps, showDeps, type DepsAsk } from '../lib/sqlgen.svelte';
  import { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';

  let { ask, onclose }: { ask: DepsAsk; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  let deps = $state<Dependency[] | null>(null);
  let failure = $state<string | null>(null);

  const ICON = { table: Table2, view: Eye, routine: Sigma, trigger: Zap, sequence: Hash } as const;
  const KIND = { table: m.ddl_kind_table, view: m.ddl_kind_view, routine: m.ddl_kind_routine, trigger: m.ddl_kind_trigger, sequence: m.ddl_kind_sequence } as const;
  const view = $derived(ask.object.kind === 'view');
  const groups = $derived(deps ? groupDeps(deps, view) : []);
  const where = $derived([view ? m.ddl_kind_view() : m.ddl_kind_table(), ask.object.namespace].filter(Boolean).join(' · '));

  onMount(() => {
    dialog.showModal();
    loadDeps(ask).then(
      (d) => (deps = d),
      (e: unknown) => (failure = e instanceof Error ? e.message : String(e)),
    );
  });

  function close() {
    dialog.close();
    depsUi.trail = [];
    onclose();
  }

  const followable = (d: Dependency) => d.kind === 'table' || d.kind === 'view';
  const target = (d: Dependency): DepsAsk => ({ ...ask, object: { kind: d.kind === 'view' ? 'view' : 'table', namespace: d.namespace, name: d.name, table: null, signature: null } });

  function follow(d: Dependency) {
    if (followable(d)) showDeps(target(d), ask);
  }

  function product() {
    return sessionFor(ask.connectionId, ask.database)?.info.server.product ?? '';
  }

  function openData(d: Dependency) {
    const { connectionId, database } = ask;
    close();
    void openRelation(connectionId, database, d.namespace, d.name);
  }

  function ddl(d: Dependency) {
    const next = { connectionId: ask.connectionId, database: ask.database, dialect: ask.dialect, product: product(), object: { kind: d.kind, namespace: d.namespace, name: d.name, table: null, signature: null } };
    close();
    showDdl(next);
  }
</script>

<dialog class="sheet-dialog tool-dialog deps-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="deps-title">
  <form method="dialog" onsubmit={(e) => (e.preventDefault(), close())}>
    <header class="dialog-head deps-head">
      {#if depsUi.trail.length}
        <button class="icon-btn deps-back" type="button" onclick={depsBack} data-tip={m.deps_back()} aria-label={m.deps_back()}>
          <ArrowLeft size={15} strokeWidth={1.8} aria-hidden="true" />
        </button>
      {/if}
      <div class="deps-titles">
        <h2 id="deps-title" class="display">{ask.object.name}</h2>
        <p class="hint">{where}</p>
      </div>
    </header>
    <div class="dialog-body deps-body">
      {#if failure}
        <p class="test-result" role="alert">{failure}</p>
      {:else if deps === null}
        <p class="hint deps-wait" role="status"><span class="conn-state busy" aria-hidden="true"></span> {m.ddl_loading()}</p>
      {:else if deps.length === 0}
        <p class="deps-empty">{m.deps_empty({ name: ask.object.name })}</p>
      {:else}
        {#each groups as g, gi (gi)}
          <section class="deps-sec" aria-labelledby="deps-sec-{gi}">
            <h3 id="deps-sec-{gi}" class="deps-title">{g.title} <span class="deps-count">{g.items.length}</span></h3>
            {#if g.items.length === 0}
              <p class="deps-none">{m.deps_none()}</p>
            {:else}
              <ul class="deps-list" aria-label={m.deps_list()}>
                {#each g.items as d (`${d.via}:${d.kind}:${d.namespace}.${d.name}:${d.detail ?? ''}`)}
                  {@const Icon = ICON[d.kind]}
                  <li class="deps-item">
                    <button class="deps-go" type="button" class:follow={followable(d)} onclick={() => follow(d)} data-tip={followable(d) ? m.deps_open({ name: d.name }) : undefined}>
                      <Icon class="deps-icon" size={14} strokeWidth={1.7} aria-hidden="true" />
                      <span class="deps-name">
                        {#if d.namespace && d.namespace !== ask.object.namespace}<span class="deps-ns">{d.namespace}.</span>{/if}{d.name}
                      </span>
                      <span class="deps-kind">{KIND[d.kind]()}</span>
                      <span class="deps-via">{viaLabel(d)}</span>
                      {#if followable(d)}<ArrowRight class="deps-arrow" size={13} strokeWidth={1.8} aria-hidden="true" />{/if}
                    </button>
                    <span class="deps-acts">
                      {#if followable(d)}
                        <button class="icon-btn" type="button" onclick={() => openData(d)} data-tip={m.tree_open_data()} aria-label={`${m.tree_open_data()}: ${d.name}`}><TableProperties size={13} strokeWidth={1.8} aria-hidden="true" /></button>
                      {/if}
                      {#if d.kind !== 'trigger'}
                        <button class="icon-btn" type="button" onclick={() => ddl(d)} data-tip={m.ddl_show()} aria-label={`${m.ddl_show()}: ${d.name}`}><FileCode size={13} strokeWidth={1.8} aria-hidden="true" /></button>
                      {/if}
                    </span>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/each}
      {/if}
    </div>
    <footer class="dialog-foot">
      {#if readFromText(ask.dialect)}<span class="hint deps-note">{m.deps_from_text()}</span>{/if}
      <span class="spacer"></span>
      <button class="btn primary" type="submit">{m.org_close()}</button>
    </footer>
  </form>
</dialog>
