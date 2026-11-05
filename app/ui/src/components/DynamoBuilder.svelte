<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { type DynamoAsk } from '../lib/dynamo-builder.svelte';
  import { queryOn } from '../lib/explorer.svelte';
  import { ICON } from '../lib/icons';
  import { dynamoCall, dynamoKeys, dynamoType, type DynamoSpec, type FilterOp, type KeyOp } from '../lib/nosql-lang';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';

  let { ask, onclose }: { ask: DynamoAsk; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  const rel = $derived(ask.rel);
  let spec = $state<DynamoSpec>({ op: 'query', index: untrack(() => ask.index), pk: '', skOp: '', sk: '', sk2: '', filters: [], limit: '', descending: false });

  const keys = $derived(dynamoKeys(rel, spec.index));
  const pk = $derived(keys[0]);
  const sk = $derived(keys[1]);
  const query = $derived(spec.op === 'query' && !!pk);
  const call = $derived(dynamoCall(rel, spec));
  const ready = $derived(!query || spec.pk.trim() !== '');

  const indexOptions = $derived([{ value: '', label: m.ddb_b_table() }, ...rel.indexes.map((i) => ({ value: i.name, label: i.name }))]);
  const KEY_OPS: { value: KeyOp; label: string }[] = [
    { value: '', label: m.ddb_b_any() },
    { value: '=', label: '=' },
    { value: '<', label: '<' },
    { value: '<=', label: '≤' },
    { value: '>', label: '>' },
    { value: '>=', label: '≥' },
    { value: 'between', label: m.ddb_b_between() },
    { value: 'begins_with', label: m.ddb_b_begins() },
  ];
  const FILTER_OPS: { value: FilterOp; label: string }[] = [
    { value: '=', label: '=' },
    { value: '<>', label: '≠' },
    { value: '<', label: '<' },
    { value: '<=', label: '≤' },
    { value: '>', label: '>' },
    { value: '>=', label: '≥' },
    { value: 'begins_with', label: m.ddb_b_begins() },
    { value: 'contains', label: m.ddb_b_contains() },
    { value: 'exists', label: m.ddb_b_exists() },
    { value: 'not_exists', label: m.ddb_b_missing() },
  ];
  const attrs = $derived(rel.columns.map((c) => ({ value: c.name, label: `${c.name} · ${dynamoType(rel, c.name)}` })));

  onMount(() => dialog.showModal());

  function close() {
    dialog.close();
    onclose();
  }

  function addFilter() {
    spec.filters = [...spec.filters, { attr: rel.columns.find((c) => !c.primary_key)?.name ?? rel.columns[0]?.name ?? '', op: '=', value: '' }];
  }

  function send(execute: boolean) {
    void queryOn(ask.connectionId, ask.database, call, execute);
    close();
  }
</script>

<dialog class="sheet-dialog tool-dialog wide ddb-builder" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="ddb-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      if (ready) send(true);
    }}
  >
    <header class="dialog-head">
      <h2 id="ddb-title" class="display">{rel.name}</h2>
      <p class="hint">{m.ddb_b_hint()}</p>
    </header>
    <div class="dialog-body">
      <div class="form-row two">
        <div class="form-field">
          <span class="caps">{m.ddb_b_operation()}</span>
          <Segmented label={m.ddb_b_operation()} value={spec.op} options={[{ value: 'query', label: 'Query' }, { value: 'scan', label: 'Scan' }]} onchange={(v) => (spec.op = v)} />
        </div>
        <div class="form-field">
          <span class="caps">{m.ddb_b_index()}</span>
          <Select class="field" label={m.ddb_b_index()} value={spec.index ?? ''} options={indexOptions} onchange={(v) => (spec.index = v || null)} />
        </div>
      </div>

      {#if query && pk}
        <label class="form-field">
          <span class="caps">{m.ddb_b_partition()} <span class="ddb-attr">{pk} · {dynamoType(rel, pk)}</span></span>
          <span class="field"><input bind:value={spec.pk} autocomplete="off" spellcheck="false" placeholder={m.ddb_b_value()} /></span>
        </label>
        {#if sk}
          <div class="form-field">
            <span class="caps">{m.ddb_b_sort()} <span class="ddb-attr">{sk} · {dynamoType(rel, sk)}</span></span>
            <div class="ddb-line" class:between={spec.skOp === 'between'}>
              <Select class="field" label={m.ddb_b_sort()} bind:value={spec.skOp} options={KEY_OPS} />
              {#if spec.skOp}
                <span class="field"><input bind:value={spec.sk} autocomplete="off" spellcheck="false" aria-label={m.ddb_b_value()} placeholder={m.ddb_b_value()} /></span>
              {/if}
              {#if spec.skOp === 'between'}
                <span class="field"><input bind:value={spec.sk2} autocomplete="off" spellcheck="false" aria-label={m.ddb_b_value_to()} placeholder={m.ddb_b_value_to()} /></span>
              {/if}
            </div>
          </div>
        {/if}
      {/if}

      <div class="form-field">
        <span class="caps">{m.ddb_b_filters()}</span>
        {#each spec.filters as f, i (i)}
          <div class="ddb-line filter">
            <Select class="field" label={m.ddb_b_attribute()} bind:value={f.attr} options={attrs} />
            <Select class="field" label={m.ddb_b_condition()} bind:value={f.op} options={FILTER_OPS} />
            {#if f.op !== 'exists' && f.op !== 'not_exists'}
              <span class="field"><input bind:value={f.value} autocomplete="off" spellcheck="false" aria-label={m.ddb_b_value()} placeholder={m.ddb_b_value()} /></span>
            {:else}
              <span></span>
            {/if}
            <button type="button" class="icon-btn" data-tip={m.ddb_b_remove()} aria-label={m.ddb_b_remove()} onclick={() => (spec.filters = spec.filters.filter((_, j) => j !== i))}>
              <ICON.remove size={14} strokeWidth={1.8} aria-hidden="true" />
            </button>
          </div>
        {/each}
        <div><button type="button" class="btn ghost small" onclick={addFilter}><ICON.newTab size={13} strokeWidth={1.8} aria-hidden="true" /> {m.ddb_b_add_filter()}</button></div>
      </div>

      <div class="form-row two">
        <label class="form-field">
          <span class="caps">{m.ddb_b_limit()}</span>
          <span class="field"><input bind:value={spec.limit} inputmode="numeric" autocomplete="off" placeholder={m.ddb_b_limit_none()} /></span>
        </label>
        {#if query}
          <div class="form-field">
            <span class="caps">{m.ddb_b_order()}</span>
            <label class="check"><input type="checkbox" bind:checked={spec.descending} /> {m.ddb_b_descending()}</label>
          </div>
        {/if}
      </div>

      <div class="form-field">
        <span class="caps">{m.ddb_b_call()}</span>
        <pre class="tool-sql ddb-call">{call}</pre>
        <span class="hint">{spec.limit.trim() ? m.ddb_b_limit_hint() : m.ddb_b_pages_hint()}</span>
      </div>
    </div>
    <footer class="dialog-foot">
      <button class="btn ghost" type="button" onclick={() => send(false)}>
        <ICON.newQuery size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.tool_open_editor()}
      </button>
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn primary" type="submit" disabled={!ready}>
        <ICON.run size={13} strokeWidth={2} aria-hidden="true" />
        {m.ddb_b_run()}
      </button>
    </footer>
  </form>
</dialog>
