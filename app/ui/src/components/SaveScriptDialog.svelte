<script lang="ts">
  import { untrack } from 'svelte';
  import { connections } from '../lib/connections.svelte';
  import { folderList, parseTags } from '../lib/library';
  import { createScript, library, patchScript } from '../lib/library.svelte';
  import { panel } from '../lib/panel.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import type { ParamValue } from '../lib/bindings';
  import { paramsSummary } from '../lib/params';
  import { suggest } from '../lib/suggest';

  let { sql, connectionId, edit, params = [], onclose }: { sql: string; connectionId: string; edit: string | null; params?: ParamValue[]; onclose: () => void } = $props();

  const existing = untrack(() => (edit ? (library.scripts.find((s) => s.id === edit) ?? null) : null));
  const firstLine = untrack(() => sql.trim().split('\n')[0]?.replace(/^--\s*/, '').slice(0, 60) ?? '');
  let name = $state(existing?.name ?? firstLine);
  let folder = $state(existing?.folder ?? '');
  let tags = $state(existing ? existing.tags.join(', ') : '');
  let where = $state<'any' | 'one'>(existing ? (existing.connection_id ? 'one' : 'any') : 'one');
  let showErrors = $state(false);
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let dialog: HTMLDialogElement;

  const folders = $derived(folderList(library.scripts));
  const connectionName = $derived(connections.list.find((c) => c.id === (existing?.connection_id ?? connectionId))?.name ?? '');

  $effect(() => dialog.showModal());

  function focus(node: HTMLInputElement) {
    queueMicrotask(() => {
      node.focus();
      node.select();
    });
  }

  function close() {
    dialog.close();
    panel.saving = null;
    onclose();
  }

  async function submit() {
    showErrors = true;
    if (!name.trim()) return;
    busy = true;
    failure = null;
    const owner = where === 'one' ? (existing?.connection_id ?? connectionId) : null;
    try {
      if (existing) await patchScript(existing, { name: name.trim(), folder, tags: parseTags(tags), connection_id: owner }, m.undo_script_edited());
      else await createScript({ name: name.trim(), folder, tags: parseTags(tags), connection_id: owner, sql, params: $state.snapshot(params) });
      close();
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<dialog class="sheet-dialog org-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="save-script-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <header class="dialog-head">
      <h2 id="save-script-title" class="display">{existing ? m.panel_scripts_edit_title() : m.panel_scripts_save_title()}</h2>
    </header>
    <div class="dialog-body">
      <label class="form-field" class:invalid={showErrors && !name.trim()}>
        <span class="caps">{m.panel_scripts_name()}</span>
        <span class="field"><input bind:value={name} use:focus /></span>
      </label>
      <div class="form-row two">
        <label class="form-field">
          <span class="caps">{m.panel_scripts_folder()}</span>
          <span class="field"><input bind:value={folder} use:suggest={{ options: folders, label: m.panel_scripts_folder() }} placeholder={m.panel_scripts_folder_hint()} /></span>
        </label>
        <label class="form-field">
          <span class="caps">{m.panel_scripts_tags()}</span>
          <span class="field"><input bind:value={tags} placeholder={m.panel_scripts_tags_hint()} /></span>
        </label>
      </div>
      <div class="form-field">
        <span class="caps">{m.panel_scripts_for()}</span>
        <Segmented
          label={m.panel_scripts_for()}
          value={where}
          options={[
            { value: 'one', label: connectionName },
            { value: 'any', label: m.panel_scripts_any() },
          ] as const}
          onchange={(v) => (where = v)}
        />
      </div>
      {#if !existing}<pre class="script-preview" aria-label={m.panel_scripts_sql()}>{sql.length > 400 ? `${sql.slice(0, 400)}…` : sql}</pre>{/if}
      {#if !existing && params.length}<p class="script-params"><span class="caps">{params.length === 1 ? m.params_saved_one() : m.params_saved_many({ count: params.length })}</span> <span class="script-params-list">{paramsSummary(params)}</span></p>{/if}
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn primary" type="submit" disabled={busy}>{m.form_save()}</button>
    </footer>
  </form>
</dialog>
