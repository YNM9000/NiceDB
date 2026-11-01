<script lang="ts">
  import { KeyRound, ShieldCheck } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, type ExportFormat } from '../lib/api';
  import { connections } from '../lib/connections.svelte';
  import { org, spaces } from '../lib/org.svelte';
  import { pickedCount } from '../lib/tree';
  import { m } from '../paraglide/messages';
  import NodePicker from './NodePicker.svelte';
  import Segmented from './Segmented.svelte';

  let { folders: f0, connections: c0, onclose }: { folders: string[]; connections: string[]; onclose: () => void } = $props();

  const everything = untrack(() => f0.length === 0 && c0.length === 0);
  let folders = $state<string[]>(untrack(() => (everything ? org.folders.filter((f) => !f.parent).map((f) => f.id) : [...f0])));
  let conns = $state<string[]>(untrack(() => (everything ? connections.list.filter((c) => !c.folder).map((c) => c.id) : [...c0])));
  let kind = $state<'sealed' | 'plain'>('sealed');
  let withSpaces = $state(spaces.list.length > 0);
  let history = $state(false);
  let settings = $state(false);
  let library = $state(false);
  let secrets = $state(false);
  let passphrase = $state('');
  let again = $state('');
  let showErrors = $state(false);
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let saved = $state(false);
  let dialog: HTMLDialogElement;

  const count = $derived(pickedCount(org.folders, connections.list, { folders, connections: conns }));
  const short = $derived(kind === 'sealed' && passphrase.length < 8);
  const mismatch = $derived(kind === 'sealed' && passphrase !== again);

  $effect(() => dialog.showModal());

  function close() {
    passphrase = '';
    again = '';
    dialog.close();
    onclose();
  }

  async function submit() {
    if (saved) return close();
    showErrors = true;
    failure = null;
    if (count === 0 || short || mismatch) return;
    busy = true;
    const format: ExportFormat = kind === 'sealed' ? { kind: 'sealed', passphrase } : { kind: 'plain' };
    const sealed = kind === 'sealed';
    try {
      const done = await api.packageExport(
        sealed ? m.org_export_file() : m.org_export_file_plain(),
        {
          folders,
          connections: conns,
          workspaces: sealed && withSpaces ? spaces.list.map((w) => w.id) : [],
          history: sealed && history,
          settings: sealed && settings,
          library: sealed && library,
          secrets: sealed && secrets,
        },
        format,
      );
      if (done) saved = true;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<dialog class="sheet-dialog org-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="export-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <header class="dialog-head">
      <h2 id="export-title" class="display">{m.org_export_title()}</h2>
    </header>
    <div class="dialog-body">
      {#if saved}
        <p class="org-done" role="status"><ShieldCheck size={16} strokeWidth={1.8} aria-hidden="true" /> {kind === 'sealed' ? m.org_export_done() : m.org_export_done_plain()}</p>
      {:else}
        <div class="form-field">
          <span class="caps">{m.org_export_format()}</span>
          <Segmented
            label={m.org_export_format()}
            value={kind}
            options={[
              { value: 'sealed', label: m.org_export_sealed() },
              { value: 'plain', label: m.org_export_plain() },
            ] as const}
            onchange={(v) => (kind = v)}
          />
          <p class="hint">{kind === 'sealed' ? m.org_export_sealed_hint() : m.org_export_plain_hint()}</p>
        </div>

        <div class="form-field" class:invalid={showErrors && count === 0}>
          <span class="caps">{m.org_export_what()}</span>
          <NodePicker bind:folders bind:conns label={m.org_export_what()} />
        </div>

        {#if kind === 'sealed'}
          <div class="form-field">
            <span class="caps">{m.org_export_also()}</span>
            <div class="org-checks">
              <label class="check"><input type="checkbox" bind:checked={withSpaces} disabled={spaces.list.length === 0} /> {m.org_export_spaces({ count: spaces.list.length })}</label>
              <label class="check"><input type="checkbox" bind:checked={history} /> {m.org_export_history()}</label>
              <label class="check"><input type="checkbox" bind:checked={library} /> {m.org_export_library()}</label>
              <label class="check"><input type="checkbox" bind:checked={settings} /> {m.org_export_settings()}</label>
              <label class="check"><input type="checkbox" bind:checked={secrets} /> <KeyRound size={13} strokeWidth={1.8} aria-hidden="true" /> {m.org_export_secrets()}</label>
            </div>
            {#if secrets}<p class="hint warn-note">{m.org_export_secrets_hint()}</p>{/if}
          </div>
          <div class="form-row two">
            <label class="form-field" class:invalid={showErrors && short}>
              <span class="caps">{m.org_passphrase()}</span>
              <span class="field"><input type="password" bind:value={passphrase} autocomplete="new-password" /></span>
            </label>
            <label class="form-field" class:invalid={showErrors && mismatch}>
              <span class="caps">{m.org_passphrase_again()}</span>
              <span class="field"><input type="password" bind:value={again} autocomplete="new-password" /></span>
            </label>
          </div>
          <p class="hint" class:test-result={showErrors && (short || mismatch)}>
            {showErrors && short ? m.org_passphrase_short() : showErrors && mismatch ? m.org_passphrase_mismatch() : m.org_passphrase_hint()}
          </p>
        {/if}
        {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
      {/if}
    </div>
    <footer class="dialog-foot">
      {#if !saved}<span class="org-summary">{m.org_count_conns({ count })}</span>{/if}
      <span class="spacer"></span>
      {#if !saved}<button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>{/if}
      <button class="btn primary" type="submit" disabled={busy}>{saved ? m.org_close() : m.org_export_go()}</button>
    </footer>
  </form>
</dialog>
