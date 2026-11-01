<script lang="ts">
  import { Folder, KeyRound, Layers, Palette, Plug, ShieldCheck, TriangleAlert } from '@lucide/svelte';
  import { ApiError, api, type ImportChoice, type ImportOutcome, type ImportPreview } from '../lib/api';
  import { loadAppearance } from '../lib/appearance.svelte';
  import type { ImportItem_Serialize as Item, ItemKind, Resolution } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { loadSpaces, org, reloadTree, spaces } from '../lib/org.svelte';
  import { loadLibrary } from '../lib/library.svelte';
  import { loadMarks } from '../lib/palette.svelte';
  import { m } from '../paraglide/messages';
  import { getLocale } from '../paraglide/runtime';
  import Segmented from './Segmented.svelte';

  let { onclose }: { onclose: () => void } = $props();

  type Step = 'pick' | 'passphrase' | 'preview' | 'done';
  let step = $state<Step>('pick');
  let path = $state<string | null>(null);
  let passphrase = $state('');
  let preview = $state<ImportPreview | null>(null);
  let choices = $state<Record<string, Resolution>>({});
  let outcome = $state<ImportOutcome | null>(null);
  let failure = $state<string | null>(null);
  let busy = $state(false);
  let dialog: HTMLDialogElement;

  const key = (i: Item) => `${i.kind}:${i.id}`;
  const fileName = $derived(path?.split(/[\\/]/).pop() ?? '');
  const conflicts = $derived(preview?.items.filter((i) => i.conflict).length ?? 0);

  $effect(() => {
    dialog.showModal();
    void pick();
  });

  function close() {
    passphrase = '';
    dialog.close();
    onclose();
  }

  async function pick() {
    path = await api.packagePick().catch(() => null);
    if (!path) return close();
    await open(null);
  }

  function defaultFor(i: Item): Resolution {
    if (i.kind === 'settings') return 'skip';
    if (i.kind === 'folder') return 'skip';
    return 'keep_both';
  }

  async function open(pass: string | null) {
    if (!path) return;
    busy = true;
    failure = null;
    try {
      preview = await api.packagePreview(path, pass);
      choices = Object.fromEntries(preview.items.filter((i) => i.conflict).map((i) => [key(i), defaultFor(i)]));
      step = 'preview';
    } catch (e) {
      if (e instanceof ApiError && e.error.code === 'package' && (e.error.detail === 'needs_passphrase' || e.error.detail === 'wrong_passphrase')) {
        if (step === 'passphrase' || e.error.detail === 'wrong_passphrase') failure = e.message;
        step = 'passphrase';
      } else {
        failure = e instanceof Error ? e.message : String(e);
        step = 'passphrase';
      }
    } finally {
      busy = false;
    }
  }

  async function apply() {
    if (!path || !preview) return;
    busy = true;
    failure = null;
    try {
      const list: ImportChoice[] = preview.items.filter((i) => i.conflict).map((i) => ({ kind: i.kind, id: i.id, resolution: choices[key(i)] ?? defaultFor(i) }));
      outcome = await api.packageImport(path, preview.sealed ? passphrase : null, list);
      passphrase = '';
      step = 'done';
      await Promise.all([reloadTree(), loadSpaces()]);
      if (outcome.settings) await loadAppearance();
      if (outcome.scripts + outcome.snippets + outcome.notes > 0) await loadLibrary();
      if (outcome.marks > 0) await loadMarks();
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function submit() {
    if (step === 'passphrase') void open(passphrase);
    else if (step === 'preview') void apply();
    else if (step === 'done') close();
  }

  const KIND: Record<ItemKind, { icon: typeof Folder; label: () => string }> = {
    folder: { icon: Folder, label: m.org_kind_folder },
    connection: { icon: Plug, label: m.org_kind_connection },
    workspace: { icon: Layers, label: m.org_kind_workspace },
    settings: { icon: Palette, label: m.org_kind_settings },
  };

  function existingName(i: Item): string {
    const c = i.conflict;
    if (!c || c.reason === 'replaces') return '';
    const id = c.existing;
    if (i.kind === 'folder') return org.folders.find((f) => f.id === id)?.name ?? id;
    if (i.kind === 'connection') return connections.list.find((x) => x.id === id)?.name ?? id;
    return spaces.list.find((w) => w.id === id)?.name ?? id;
  }

  function conflictText(i: Item): string {
    const c = i.conflict;
    if (!c) return '';
    if (c.reason === 'replaces') return m.org_conflict_settings();
    return c.reason === 'same_id' ? m.org_conflict_same_id({ name: existingName(i) }) : m.org_conflict_same_name({ name: existingName(i) });
  }

  function options(i: Item) {
    if (i.kind === 'settings')
      return [
        { value: 'skip', label: m.org_res_keep_mine() },
        { value: 'replace', label: m.org_res_replace() },
      ] as const;
    if (i.kind === 'folder')
      return [
        { value: 'skip', label: m.org_res_merge() },
        { value: 'keep_both', label: m.org_res_separate() },
      ] as const;
    return [
      { value: 'keep_both', label: m.org_res_keep_both() },
      { value: 'replace', label: m.org_res_replace() },
      { value: 'skip', label: m.org_res_skip() },
    ] as const;
  }

  const created = $derived(preview ? new Date(preview.created_at ?? 0).toLocaleDateString(getLocale(), { day: 'numeric', month: 'long', year: 'numeric' }) : '');
</script>

<dialog class="sheet-dialog org-dialog import-dialog" class:narrow={step !== 'preview'} bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="import-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <header class="dialog-head">
      <h2 id="import-title" class="display">{m.org_import_title()}</h2>
      {#if fileName}<p class="org-file">{fileName}</p>{/if}
    </header>
    <div class="dialog-body">
      {#if step === 'pick'}
        <p class="org-note">{m.org_import_picking()}</p>
      {:else if step === 'passphrase'}
        <p class="org-note">{m.org_import_sealed()}</p>
        <label class="form-field" class:invalid={!!failure}>
          <span class="caps">{m.org_passphrase()}</span>
          <span class="field"><input type="password" bind:value={passphrase} autocomplete="off" use:focusInput /></span>
        </label>
      {:else if step === 'preview' && preview}
        <p class="org-note">
          {preview.sealed ? m.org_import_summary_sealed({ date: created, count: preview.items.length }) : m.org_import_summary_plain({ count: preview.items.length })}
          {#if preview.history > 0}{m.org_import_history({ count: preview.history })}{/if}
          {#if preview.scripts + preview.snippets + preview.notes > 0}{m.org_import_library({ scripts: preview.scripts, snippets: preview.snippets, notes: preview.notes })}{/if}
        </p>
        {#if conflicts > 0}<p class="hint warn-note"><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.org_import_conflicts({ count: conflicts })}</p>{/if}
        <ul class="import-list">
          {#each preview.items as item (key(item))}
            {@const k = KIND[item.kind]}
            <li class="import-item" class:clash={!!item.conflict}>
              <span class="import-icon" aria-hidden="true"><k.icon size={15} strokeWidth={1.7} /></span>
              <span class="import-what">
                <span class="import-name">{item.kind === 'settings' ? m.org_kind_settings() : item.name}</span>
                <span class="import-meta">
                  {#if item.kind !== 'settings'}<span>{[k.label(), item.path.join(' / ')].filter(Boolean).join(' · ')}</span>{/if}
                  {#if item.secret}<span class="import-secret"><KeyRound size={11} strokeWidth={2} aria-hidden="true" /> {m.org_import_with_secret()}</span>{/if}
                </span>
                {#if item.conflict}<span class="import-conflict">{conflictText(item)}</span>{/if}
              </span>
              {#if item.conflict}
                <Segmented label={conflictText(item)} value={choices[key(item)] ?? defaultFor(item)} options={options(item)} onchange={(v) => (choices[key(item)] = v)} />
              {:else}
                <span class="import-new">{m.org_import_new()}</span>
              {/if}
            </li>
          {/each}
        </ul>
      {:else if step === 'done' && outcome}
        <p class="org-done" role="status"><ShieldCheck size={16} strokeWidth={1.8} aria-hidden="true" /> {m.org_import_done({ added: outcome.added, replaced: outcome.replaced, skipped: outcome.skipped })}</p>
        {#if outcome.history > 0}<p class="hint">{m.org_import_done_history({ count: outcome.history })}</p>{/if}
        {#if outcome.scripts + outcome.snippets + outcome.notes > 0}<p class="hint">{m.org_import_done_library({ scripts: outcome.scripts, snippets: outcome.snippets, notes: outcome.notes })}</p>{/if}
        {#if outcome.params + outcome.profiles + outcome.marks > 0}<p class="hint">{m.org_import_done_extras({ params: outcome.params, profiles: outcome.profiles, marks: outcome.marks })}</p>{/if}
        {#if outcome.settings}<p class="hint">{m.org_import_done_settings()}</p>{/if}
      {/if}
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      <span class="spacer"></span>
      {#if step !== 'done'}<button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>{/if}
      {#if step === 'passphrase'}
        <button class="btn primary" type="submit" disabled={busy || !passphrase}>{m.org_import_open()}</button>
      {:else if step === 'preview'}
        <button class="btn primary" type="submit" disabled={busy}>{m.org_import_go()}</button>
      {:else if step === 'done'}
        <button class="btn primary" type="submit">{m.org_close()}</button>
      {/if}
    </footer>
  </form>
</dialog>

<script lang="ts" module>
  function focusInput(node: HTMLInputElement) {
    queueMicrotask(() => node.focus());
  }
</script>
