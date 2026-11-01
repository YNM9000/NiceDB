<script lang="ts">
  import { Droplet, TriangleAlert } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { counted, touches } from '../lib/estimate';
  import { confirmState } from '../lib/guard.svelte';
  import { prefs } from '../lib/prefs.svelte';
  import { confirmAnswer } from '../lib/prodsafety';
  import { m } from '../paraglide/messages';

  const req = $derived(confirmState.req);
  let dialog = $state<HTMLDialogElement>();
  let typed = $state('');
  let decline = $state<HTMLButtonElement>();

  const typing = $derived(!!req?.production && prefs.value.safety.confirm_mode === 'typed');
  const matches = $derived(!!req && (!typing || typed.trim() === req.connection.trim()));

  $effect(() => {
    if (req && dialog && !dialog.open) {
      typed = '';
      dialog.showModal();
      if (!typing) void tick().then(() => decline?.focus());
    }
    if (!req && dialog?.open) dialog.close();
  });

  function answer(ok: boolean) {
    if (!req) return;
    req.resolve(ok ? confirmAnswer(prefs.value.safety.confirm_mode, req.production, typed) : false);
  }
</script>

<dialog class="sheet-dialog confirm-write" class:prod={req?.production} bind:this={dialog} oncancel={(e) => (e.preventDefault(), answer(false))} aria-labelledby="cw-title">
  {#if req}
    <form
      method="dialog"
      onsubmit={(e) => {
        e.preventDefault();
        if (typing && matches) answer(true);
      }}
    >
      <header class="dialog-head cw-head">
        {#if req.production}<span class="cw-stamp">{m.env_production()}</span>{:else}<span class="cw-warn"><TriangleAlert size={18} strokeWidth={1.8} aria-hidden="true" /></span>{/if}
        <h2 class="display cw-title" id="cw-title">{req.title}</h2>
        {#if req.connection}<p class="cw-sub">{req.connection}</p>{/if}
      </header>
      <div class="dialog-body cw-body">
        {#if req.lines.length}
          <ol class="cw-lines" class:single={req.lines.length === 1}>
            {#each req.lines as line, i (i)}<li>{line}</li>{/each}
          </ol>
        {/if}
        {#if req.risks.length}
          <ul class="cw-risks">
            {#each req.risks as r (r.index)}
              <li>
                <code>{r.sql.length > 220 ? `${r.sql.slice(0, 220)}…` : r.sql}</code>
                {#if counted(r)}<span class="cw-rows" class:unknown={r.rows === null}>{touches(r.rows)}</span>{/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if req.rows}<p class="cw-rows" class:unknown={req.rows === touches(null)}>{req.rows}</p>{/if}
        {#if req.stale}<p class="cw-stale" role="alert">{req.stale}</p>{/if}
        {#if req.note}<p class="cw-note">{req.note}</p>{/if}
        {#if typing}
          <label class="cw-type">
            <span>{m.confirm_type()} <b>{req.connection}</b></span>
            <!-- svelte-ignore a11y_autofocus -->
            <input bind:value={typed} autocomplete="off" spellcheck="false" aria-label={m.confirm_name()} autofocus />
          </label>
        {/if}
      </div>
      <footer class="dialog-foot cw-foot">
        <span class="spacer"></span>
        <button class="btn" type="button" bind:this={decline} onclick={() => answer(false)}>{m.confirm_cancel()}</button>
        <button class="btn danger cw-go" type={typing ? 'submit' : 'button'} disabled={!matches} onclick={() => !typing && answer(true)}>
          {#if req.production}<Droplet size={13} strokeWidth={1.8} aria-hidden="true" />{/if}
          {req.production && !typing ? m.confirm_ok() : req.action}
        </button>
      </footer>
    </form>
  {/if}
</dialog>
