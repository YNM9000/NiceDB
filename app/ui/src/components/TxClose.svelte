<script lang="ts">
  import { Check, Undo2 } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { txSummary } from '../lib/tx';
  import { txs } from '../lib/tx.svelte';
  import { m } from '../paraglide/messages';

  const ask = $derived(txs.ask);
  let dialog = $state<HTMLDialogElement>();
  let keep = $state<HTMLButtonElement>();

  const TITLE = { tab: m.tx_close_tab_title, space: m.tx_close_space_title, app: m.tx_close_app_title };

  $effect(() => {
    if (ask && dialog && !dialog.open) {
      dialog.showModal();
      void tick().then(() => keep?.focus());
    }
    if (!ask && dialog?.open) dialog.close();
  });
</script>

<dialog class="sheet-dialog tx-close" bind:this={dialog} oncancel={(e) => (e.preventDefault(), ask?.resolve(false))} aria-labelledby="txc-title">
  {#if ask}
    <header class="dialog-head">
      <h2 class="display" id="txc-title">{TITLE[ask.reason]()}</h2>
    </header>
    <div class="dialog-body txc-body">
      <ul class="txc-list">
        {#each ask.list as w (w.sessionId)}
          <li>
            <span class="txc-name">{w.connection}{#if w.database}<span class="txc-db">{w.database}</span>{/if}</span>
            <span class="txc-sum">{w.state.dry_run ? `${m.tx_dry()} · ` : ''}{txSummary(w.state)}</span>
          </li>
        {/each}
      </ul>
      <p class="txc-note">{m.tx_close_body()}</p>
    </div>
    <footer class="dialog-foot">
      <button class="btn" type="button" bind:this={keep} onclick={() => ask?.resolve(false)}>{m.tx_close_keep()}</button>
      <span class="spacer"></span>
      <button class="btn" type="button" onclick={() => ask?.resolve('rollback')}><Undo2 size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tx_close_rollback()}</button>
      <button class="btn primary" type="button" disabled={ask.list.some((w) => w.state.failed)} onclick={() => ask?.resolve('commit')}><Check size={13} strokeWidth={2} aria-hidden="true" /> {m.tx_close_commit()}</button>
    </footer>
  {/if}
</dialog>
