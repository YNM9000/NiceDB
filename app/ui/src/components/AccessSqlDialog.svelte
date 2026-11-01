<script lang="ts">
  import { Play, TextCursorInput } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { runScript } from '../lib/tools.svelte';
  import { newTab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';

  interface Props {
    title: string;
    subject?: string | null;
    sql: string;
    warn?: string | null;
    danger?: boolean;
    connectionId: string;
    onclose: () => void;
    ondone: () => void;
  }
  let { title, subject = null, sql: initial, warn = null, danger = false, connectionId, onclose, ondone }: Props = $props();

  let sql = $state(untrack(() => initial));
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let dialog: HTMLDialogElement;

  $effect(() => dialog.showModal());

  function close() {
    dialog.close();
    onclose();
  }

  async function execute() {
    if (!sql.trim()) return;
    busy = true;
    failure = null;
    const error = await runScript(connectionId, sql);
    busy = false;
    if (error) failure = error;
    else {
      ondone();
      close();
    }
  }
</script>

<dialog class="sheet-dialog tool-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="asql-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void execute();
    }}
  >
    <header class="dialog-head">
      <h2 id="asql-title" class="display">{title}</h2>
      {#if subject}<p class="hint">{subject}</p>{/if}
    </header>
    <div class="dialog-body">
      {#if warn}<p class="tool-warn">{warn}</p>{/if}
      <label class="form-field">
        <span class="caps">{m.tool_sql_review()}</span>
        <textarea class="tool-sql" rows="6" spellcheck="false" bind:value={sql}></textarea>
        <span class="hint">{m.tool_sql_review_hint()}</span>
      </label>
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      <button class="btn ghost" type="button" disabled={!sql.trim()} onclick={() => (newTab(connectionId, sql), close())}>
        <TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.tool_open_editor()}
      </button>
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn primary" class:danger type="submit" disabled={busy || !sql.trim()}>
        <Play size={13} strokeWidth={2} aria-hidden="true" />
        {m.tool_sql_run()}
      </button>
    </footer>
  </form>
</dialog>
