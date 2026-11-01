<script lang="ts">
  import { untrack } from 'svelte';
  import { KINDS, parseTtl, type Kind } from '../lib/keys';
  import { writeKey, type KeyBrowserState } from '../lib/keys.svelte';
  import { showToast } from '../lib/toast.svelte';
  import type { sessionOf } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Select from './Select.svelte';

  let { st, session, prefix, onclose }: { st: KeyBrowserState; session: NonNullable<ReturnType<typeof sessionOf>>; prefix: string; onclose: () => void } = $props();

  let key = $state(untrack(() => prefix));
  let kind = $state<Kind>('string');
  let name = $state('');
  let value = $state('');
  let ttl = $state('');
  let showErrors = $state(false);
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let dialog: HTMLDialogElement;

  $effect(() => dialog.showModal());

  const LABEL: Record<Kind, () => string> = {
    string: m.kv_type_string,
    hash: m.kv_type_hash,
    list: m.kv_type_list,
    set: m.kv_type_set,
    zset: m.kv_type_zset,
    stream: m.kv_type_stream,
    'ReJSON-RL': m.kv_type_json,
  };
  const needsName = $derived(kind === 'hash' || kind === 'set' || kind === 'zset' || kind === 'stream');
  const needsValue = $derived(kind !== 'set');
  const nameLabel = $derived(kind === 'hash' || kind === 'stream' ? m.kv_col_field() : m.kv_col_member());
  const valueLabel = $derived(kind === 'zset' ? m.kv_col_score() : m.kv_col_value());
  const ttlMs = $derived(ttl.trim() ? parseTtl(ttl) : null);
  const bad = $derived({ key: !key.trim(), name: needsName && !name.trim(), ttl: !!ttl.trim() && ttlMs === null, score: kind === 'zset' && !Number.isFinite(Number(value.replace(',', '.') || 'x')) });

  function focus(node: HTMLInputElement) {
    queueMicrotask(() => {
      node.focus();
      node.setSelectionRange(node.value.length, node.value.length);
    });
  }

  function close() {
    dialog.close();
    onclose();
  }

  async function submit() {
    showErrors = true;
    if (Object.values(bad).some(Boolean)) return;
    busy = true;
    failure = null;
    const write = { op: 'create' as const, key: key.trim(), kind, first: { name: name.trim(), value: kind === 'zset' ? value.replace(',', '.') : value }, ttl_ms: ttlMs };
    const done = await writeKey(session.info, write);
    busy = false;
    if (!done) return;
    if (!done.ok) {
      failure = done.message;
      return;
    }
    st.applied(write);
    showToast(m.kv_created({ key: write.key }));
    close();
    await st.select(session.info.session_id, write.key);
  }
</script>

<dialog class="sheet-dialog org-dialog kv-new" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="kv-new-title">
  <form method="dialog" onsubmit={(e) => (e.preventDefault(), void submit())}>
    <header class="dialog-head">
      <h2 id="kv-new-title" class="display">{m.kv_new_key()}</h2>
    </header>
    <div class="dialog-body">
      <label class="form-field" class:invalid={showErrors && bad.key}>
        <span class="caps">{m.kv_key_name()}</span>
        <span class="field"><input class="mono" bind:value={key} use:focus spellcheck="false" placeholder={m.kv_key_hint()} /></span>
      </label>
      <div class="form-row two">
        <label class="form-field">
          <span class="caps">{m.kv_type()}</span>
          <Select class="field" label={m.kv_type()} bind:value={kind} options={KINDS.map((k) => ({ value: k, label: LABEL[k]() }))} />
        </label>
        <label class="form-field" class:invalid={showErrors && bad.ttl}>
          <span class="caps">{m.kv_ttl()}</span>
          <span class="field"><input bind:value={ttl} placeholder={m.kv_ttl_none_hint()} spellcheck="false" /></span>
        </label>
      </div>
      {#if needsName}
        <label class="form-field" class:invalid={showErrors && bad.name}>
          <span class="caps">{nameLabel}</span>
          <span class="field"><input class="mono" bind:value={name} spellcheck="false" /></span>
        </label>
      {/if}
      {#if needsValue}
        <label class="form-field" class:invalid={showErrors && bad.score}>
          <span class="caps">{valueLabel}</span>
          {#if kind === 'string' || kind === 'ReJSON-RL'}
            <span class="field"><textarea class="mono kv-new-value" bind:value rows="5" spellcheck="false" placeholder={kind === 'ReJSON-RL' ? m.kv_ph_json() : ''}></textarea></span>
          {:else}
            <span class="field"><input class="mono" bind:value spellcheck="false" /></span>
          {/if}
        </label>
      {/if}
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn primary" type="submit" disabled={busy}>{m.kv_create()}</button>
    </footer>
  </form>
</dialog>
