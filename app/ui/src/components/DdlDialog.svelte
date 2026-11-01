<script lang="ts">
  import { selectAll } from '@codemirror/commands';
  import { EditorState } from '@codemirror/state';
  import { EditorView } from '@codemirror/view';
  import { ClipboardCopy, TextCursorInput, TextSelect } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { copyText, keys, openMenu } from '../lib/contextmenu.svelte';
  import { loadDdl, openDdl, type DdlAsk } from '../lib/ddl.svelte';
  import { viewerExtensions } from '../lib/editor';
  import { prefs } from '../lib/prefs.svelte';
  import { m } from '../paraglide/messages';

  let { ask, onclose }: { ask: DdlAsk; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  let host = $state<HTMLDivElement>();
  let sql = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let view: EditorView | null = null;

  const KIND = { table: m.ddl_kind_table, view: m.ddl_kind_view, routine: m.ddl_kind_routine, trigger: m.ddl_kind_trigger, index: m.ddl_kind_index, sequence: m.ddl_kind_sequence } as const;
  const where = $derived([ask.object.kind === 'table' && ask.dialect === null ? m.ddl_kind_collection() : KIND[ask.object.kind](), ask.object.namespace, ask.object.table].filter(Boolean).join(' · '));

  onMount(() => {
    dialog.showModal();
    const o = ask.object;
    loadDdl(ask.connectionId, ask.database, o).then(
      (text) => (sql = text),
      (e: unknown) => (failure = e instanceof Error ? e.message : String(e)),
    );
    return () => view?.destroy();
  });

  $effect(() => {
    if (!host || sql === null || view) return;
    view = new EditorView({ parent: host, state: EditorState.create({ doc: sql, extensions: viewerExtensions(ask.dialect, prefs.value.editor) }) });
  });

  function close() {
    dialog.close();
    onclose();
  }

  function menu(e: MouseEvent) {
    const v = view;
    if (!v) return;
    const sel = v.state.selection.main;
    openMenu(e, [
      { label: m.menu_copy(), icon: ClipboardCopy, shortcut: keys('Mod+C'), disabled: sel.empty, run: () => copyText(v.state.sliceDoc(sel.from, sel.to)) },
      { label: m.menu_select_all(), icon: TextSelect, shortcut: keys('Mod+A'), run: () => selectAll(v) },
    ]);
  }
</script>

<dialog class="sheet-dialog tool-dialog wide ddl-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="ddl-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      if (sql) void copyText(sql);
    }}
  >
    <header class="dialog-head">
      <h2 id="ddl-title" class="display">{ask.object.name}</h2>
      <p class="hint">{where}</p>
    </header>
    <div class="dialog-body">
      {#if failure}
        <p class="test-result" role="alert">{m.ddl_failed({ name: ask.object.name, reason: failure })}</p>
      {:else if sql === null}
        <p class="hint ddl-wait" role="status"><span class="conn-state busy" aria-hidden="true"></span> {m.ddl_loading()}</p>
      {:else}
        <div class="ddl-code" bind:this={host} oncontextmenu={menu} role="region" aria-label={m.ddl_code({ product: ask.product })}></div>
        <span class="hint">{m.ddl_source({ product: ask.product })}</span>
      {/if}
    </div>
    <footer class="dialog-foot">
      <button class="btn ghost" type="button" disabled={!sql} onclick={() => sql && (void openDdl(ask.connectionId, ask.database, sql), close())}>
        <TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.tool_open_editor()}
      </button>
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.org_close()}</button>
      <button class="btn primary" type="submit" disabled={!sql}>
        <ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.copy()}
      </button>
    </footer>
  </form>
</dialog>
