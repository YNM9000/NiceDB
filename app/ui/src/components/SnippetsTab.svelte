<script lang="ts">
  import { ClipboardCopy, CopyPlus, Pencil, Plus, Search, SquarePlus, TextCursorInput, Trash2 } from '@lucide/svelte';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { matchSnippet } from '../lib/library';
  import { deleteSnippet, library, saveSnippet } from '../lib/library.svelte';
  import { plain, snippetsFor, type ReadySnippet } from '../lib/snippets';
  import type { Dialect } from '../lib/bindings';
  import { newTab, activeTab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import type Editor from './Editor.svelte';
  import Select from './Select.svelte';
  import { DIALECT_NAME } from '../lib/tools';

  let { dialect, editor, inQuery }: { dialect: Dialect; editor: Editor | null; inQuery: boolean } = $props();

  const DIALECTS: Dialect[] = ['postgres', 'mysql', 'sqlite', 'mssql', 'duckdb', 'clickhouse', 'firebird', 'hana', 'oracle', 'db2', 'ansi'];

  let query = $state('');
  let form = $state<{ id: string; name: string; trigger: string; dialect: string; description: string; body: string } | null>(null);
  let showErrors = $state(false);
  let failure = $state<string | null>(null);

  const all = $derived(snippetsFor(dialect, library.snippets));
  const shown = $derived(all.filter((s) => matchSnippet(s, query)));
  const mine = $derived(shown.filter((s) => !s.builtin));
  const ready = $derived(shown.filter((s) => s.builtin));
  const canInsert = $derived(inQuery && !!editor);
  const triggerOk = $derived(!!form && /^\w+$/.test(form.trigger));

  function newSnippet() {
    showErrors = false;
    form = { id: crypto.randomUUID(), name: '', trigger: '', dialect: '', description: '', body: '' };
  }

  function edit(s: ReadySnippet) {
    showErrors = false;
    form = { id: s.id, name: s.name, trigger: s.trigger, dialect: s.dialect ?? '', description: s.description, body: s.body };
  }

  function copyToMine(s: ReadySnippet) {
    showErrors = false;
    form = { id: crypto.randomUUID(), name: s.name, trigger: s.trigger, dialect: dialect, description: s.description, body: s.body };
  }

  async function submit() {
    showErrors = true;
    if (!form || !form.name.trim() || !triggerOk || !form.body.trim()) return;
    failure = null;
    try {
      await saveSnippet({ id: form.id, name: form.name.trim(), trigger: form.trigger, description: form.description.trim(), body: form.body, dialect: form.dialect || null });
      form = null;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
  }

  function menu(e: MouseEvent, s: ReadySnippet) {
    const own = library.snippets.find((x) => x.id === s.id);
    openMenu(e, [
      { label: m.history_insert(), icon: TextCursorInput, disabled: !canInsert, run: () => editor?.insertSnippet(s.body) },
      { label: m.history_open_tab(), icon: SquarePlus, run: () => openTab(s) },
      { label: m.menu_copy(), icon: ClipboardCopy, run: () => copyText(plain(s.body)) },
      'separator',
      s.builtin ? { label: m.panel_snippets_duplicate(), icon: CopyPlus, run: () => copyToMine(s) } : { label: m.panel_scripts_edit(), icon: Pencil, run: () => edit(s) },
      !s.builtin && own && { label: m.panel_delete(), icon: Trash2, danger: true, run: () => deleteSnippet(own) },
    ]);
  }

  function openTab(s: ReadySnippet) {
    const t = activeTab();
    if (t) newTab(t.connectionId, plain(s.body));
  }

  function focus(node: HTMLInputElement) {
    queueMicrotask(() => node.focus());
  }
</script>

<div class="snippets">
  <div class="panel-tools">
    <label class="field">
      <Search size={13} strokeWidth={1.8} aria-hidden="true" />
      <input type="search" placeholder={m.panel_snippets_search()} aria-label={m.panel_snippets_search()} bind:value={query} />
    </label>
  </div>

  {#if form}
    <form
      class="snippet-form"
      onsubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label class="form-field" class:invalid={showErrors && !form.name.trim()}>
        <span class="caps">{m.panel_snippets_name()}</span>
        <span class="field"><input bind:value={form.name} use:focus /></span>
      </label>
      <div class="form-row two">
        <label class="form-field" class:invalid={showErrors && !triggerOk}>
          <span class="caps">{m.panel_snippets_trigger()}</span>
          <span class="field"><input bind:value={form.trigger} spellcheck="false" autocapitalize="off" /></span>
        </label>
        <label class="form-field">
          <span class="caps">{m.panel_snippets_dialect()}</span>
          <Select class="field" label={m.panel_snippets_dialect()} bind:value={form.dialect} options={[{ value: '', label: m.panel_snippets_any() }, ...DIALECTS.map((d) => ({ value: d, label: DIALECT_NAME[d] }))]} />
        </label>
      </div>
      <label class="form-field">
        <span class="caps">{m.panel_snippets_description()}</span>
        <span class="field"><input bind:value={form.description} /></span>
      </label>
      <label class="form-field" class:invalid={showErrors && !form.body.trim()}>
        <span class="caps">{m.panel_snippets_body()}</span>
        <textarea class="snippet-body" rows="6" spellcheck="false" bind:value={form.body}></textarea>
      </label>
      <p class="hint">{m.panel_snippets_hint({ example: '${1:' + m.panel_snippets_example() + '}' })}</p>
      {#if showErrors && !triggerOk}<p class="error" role="alert">{m.panel_snippets_trigger_bad()}</p>{/if}
      {#if failure}<p class="error" role="alert">{failure}</p>{/if}
      <div class="panel-actions">
        <span class="spacer"></span>
        <button class="btn ghost compact" type="button" onclick={() => (form = null)}>{m.form_cancel()}</button>
        <button class="btn primary compact" type="submit">{m.form_save()}</button>
      </div>
    </form>
  {:else}
    <div class="panel-actions">
      <button class="btn compact" type="button" onclick={newSnippet}>
        <Plus size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.panel_snippets_new()}
      </button>
    </div>
    <div class="panel-scroll">
      {#each [{ title: m.panel_snippets_mine(), list: mine }, { title: m.panel_snippets_default({ dialect: DIALECT_NAME[dialect] }), list: ready }] as g (g.title)}
        {#if g.list.length > 0}
          <h3 class="caps group-title">{g.title}</h3>
          <ul class="panel-list">
            {#each g.list as s (s.id)}
              <li class="panel-item">
                <button type="button" class="panel-row" data-tip={m.history_insert()} aria-disabled={!canInsert} onclick={() => canInsert && editor?.insertSnippet(s.body)} oncontextmenu={(e) => menu(e, s)}>
                  <span class="row-title">{s.name} <code class="trigger">{s.trigger}</code></span>
                  <span class="row-sub">{s.description || plain(s.body).split('\n')[0]}</span>
                </button>
                <span class="row-actions">
                  <button type="button" class="row-act" data-tip={m.history_insert()} aria-label="{m.history_insert()}: {s.name}" disabled={!canInsert} onclick={() => editor?.insertSnippet(s.body)}><TextCursorInput size={13} strokeWidth={1.8} /></button>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      {/each}
      {#if shown.length === 0}<p class="hint panel-empty">{m.panel_snippets_none()}</p>{/if}
    </div>
  {/if}
</div>
