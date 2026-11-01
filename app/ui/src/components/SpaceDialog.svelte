<script lang="ts">
  import { Plus, X } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { secretLike, VAR_EXAMPLE, VAR_NAME } from '../lib/vars';
  import { createSpace, deleteSpace, editSpace, spaces } from '../lib/org.svelte';
  import type { TreeNode } from '../lib/tree';
  import { m } from '../paraglide/messages';
  import NodePicker from './NodePicker.svelte';

  let { id, preset, remove = false, toVars = false, onclose }: { id: string | null; preset: TreeNode | null; remove?: boolean; toVars?: boolean; onclose: () => void } = $props();

  const initial = untrack(() => (id ? (spaces.list.find((w) => w.id === id) ?? null) : null));
  const p = untrack(() => preset);
  let name = $state(initial?.name ?? '');
  let folders = $state<string[]>(initial ? [...initial.folders] : p?.kind === 'folder' ? [p.id] : []);
  let conns = $state<string[]>(initial ? [...initial.connections] : p?.kind === 'connection' ? [p.id] : []);
  let vars = $state<{ name: string; value: string }[]>(Object.entries(initial?.vars ?? {}).map(([name, value]) => ({ name, value })));
  let showErrors = $state(false);
  let busy = $state(false);

  function varProblem(i: number): string | null {
    const v = vars[i];
    if (!v) return null;
    const n = v.name.trim();
    if (!n) return v.value.trim() ? m.org_space_var_bad_name() : null;
    if (!VAR_NAME.test(n)) return m.org_space_var_bad_name();
    if (secretLike(n)) return m.org_space_var_secret();
    if (vars.findIndex((x) => x.name.trim() === n) !== i) return m.org_space_var_dup();
    return null;
  }

  const varsBad = $derived(vars.some((_, i) => varProblem(i) !== null));
  const varsMap = () => Object.fromEntries(vars.filter((v) => v.name.trim()).map((v) => [v.name.trim(), v.value]));
  let dialog: HTMLDialogElement;

  $effect(() => dialog.showModal());

  function focus(node: HTMLInputElement) {
    if (!untrack(() => toVars)) queueMicrotask(() => node.focus());
  }

  function varsFirst(node: HTMLElement) {
    if (!untrack(() => toVars)) return;
    queueMicrotask(() => {
      node.scrollIntoView({ block: 'nearest' });
      (node.querySelector<HTMLElement>('.vars-grid input') ?? node.querySelector<HTMLElement>('.vars-add'))?.focus();
    });
  }

  function close() {
    dialog.close();
    onclose();
  }

  async function submit() {
    showErrors = true;
    if (!remove && (!name.trim() || varsBad)) return;
    busy = true;
    try {
      if (remove && initial) await deleteSpace(initial.id);
      else if (initial) await editSpace(initial.id, { name: name.trim(), folders, connections: conns, vars: varsMap() });
      else await createSpace(name.trim(), folders, conns, varsMap());
      close();
    } finally {
      busy = false;
    }
  }
</script>

<dialog class="sheet-dialog org-dialog" class:narrow={remove} bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="space-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <header class="dialog-head">
      <h2 id="space-title" class="display">
        {remove ? m.org_space_delete_title({ name: initial?.name ?? '' }) : initial ? m.org_space_edit_title() : m.org_space_new_title()}
      </h2>
    </header>
    <div class="dialog-body">
      {#if remove}
        <p class="org-note">{m.org_space_delete_body()}</p>
      {:else}
        <label class="form-field" class:invalid={showErrors && !name.trim()}>
          <span class="caps">{m.org_space_name()}</span>
          <span class="field"><input bind:value={name} placeholder={m.org_space_name_hint()} use:focus /></span>
        </label>
        <div class="form-field">
          <span class="caps">{m.org_space_members()}</span>
          <p class="hint">{m.org_space_members_hint()}</p>
          <NodePicker bind:folders bind:conns label={m.org_space_members()} />
        </div>
        <div class="form-field space-vars" use:varsFirst>
          <span class="caps" id="space-vars-label">{m.org_space_vars()}</span>
          <p class="hint">{m.org_space_vars_hint({ example: VAR_EXAMPLE })}</p>
          {#if vars.length}
            <div class="vars-grid" role="group" aria-labelledby="space-vars-label">
              <span class="vars-head caps">{m.org_space_var_name()}</span>
              <span class="vars-head caps">{m.org_space_var_value()}</span>
              <span></span>
              {#each vars as v, i (i)}
                {@const problem = showErrors || v.name.trim() ? varProblem(i) : null}
                <span class="field" class:bad={!!problem}
                  ><input bind:value={v.name} placeholder={m.org_ph_var_name()} spellcheck="false" autocomplete="off" aria-label={m.org_space_var_name()} aria-invalid={!!problem} /></span
                >
                <span class="field"><input bind:value={v.value} placeholder={m.org_ph_var_value()} spellcheck="false" autocomplete="off" aria-label={m.org_space_var_value()} /></span>
                <button class="icon-btn" type="button" aria-label={m.org_space_var_remove({ name: v.name || '…' })} onclick={() => (vars = vars.filter((_, k) => k !== i))}><X size={13} strokeWidth={1.8} /></button>
                {#if problem}<p class="vars-problem" role="status">{problem}</p>{/if}
              {/each}
            </div>
          {/if}
          <button class="btn ghost vars-add" type="button" onclick={() => (vars = [...vars, { name: '', value: '' }])}><Plus size={14} strokeWidth={1.8} /> {m.org_space_var_add()}</button>
        </div>
      {/if}
    </div>
    <footer class="dialog-foot">
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn {remove ? 'danger' : 'primary'}" type="submit" disabled={busy}>
        {remove ? m.org_space_delete() : initial ? m.form_save() : m.org_space_create()}
      </button>
    </footer>
  </form>
</dialog>
