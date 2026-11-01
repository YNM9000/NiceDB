<script lang="ts">
  import { Check, ChevronDown, GitCommitHorizontal, Undo2 } from '@lucide/svelte';
  import { openMenu } from '../lib/contextmenu.svelte';
  import { clock } from '../lib/guard.svelte';
  import type { CommitMode } from '../lib/api';
  import { MODES, modeHint, modeLabel, modeTag, txSummary } from '../lib/tx';
  import { commitTx, openFor, rollbackTx, setMode, txOf, txs } from '../lib/tx.svelte';
  import { m } from '../paraglide/messages';

  let { sessionId, running = false }: { sessionId: string; running?: boolean } = $props();

  const tx = $derived(txOf(sessionId));
  const mode = $derived<CommitMode>(tx?.mode ?? 'auto');
  const open = $derived(!!tx?.open);
  const busy = $derived(running || !!txs.busy[sessionId]);
  const SHORT = { auto: m.tx_mode_short_auto, manual: m.tx_mode_short_manual, smart: m.tx_mode_short_smart };

  function pickMode(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu(
      { x: r.left, y: r.bottom + 4 },
      MODES.map((value) => ({
        label: modeLabel(value),
        hint: modeTag(value),
        icon: value === mode ? Check : undefined,
        run: () => value !== mode && void setMode(sessionId, value),
      })),
      m.tx_mode_label(),
    );
  }
</script>

<div class="txbar" class:open class:dry={!!tx?.dry_run} class:failed={!!tx?.failed}>
  <button class="btn ghost tx-mode" type="button" aria-haspopup="menu" aria-label={`${m.tx_mode_label()}: ${modeLabel(mode)}`} data-tip={`${modeLabel(mode)}. ${modeHint(mode)}`} onclick={pickMode}>
    <GitCommitHorizontal size={14} strokeWidth={1.8} aria-hidden="true" />
    <span class="tx-mode-name">{SHORT[mode]()}</span>
    <ChevronDown size={12} strokeWidth={2} aria-hidden="true" />
  </button>
  {#if tx?.open}
    <span class="tx-live" role="status">
      <span class="tx-lamp" aria-hidden="true"></span>
      <span class="tx-what">{tx.dry_run ? m.tx_dry() : m.tx_open()}</span>
      <span class="tx-sum">{txSummary(tx)}</span>
      <span class="tx-time" role="timer">{clock(openFor(sessionId))}</span>
    </span>
    <button class="btn tx-rollback" type="button" disabled={busy} onclick={() => void rollbackTx(sessionId)}>
      <Undo2 size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.tx_rollback()}
    </button>
    <button class="btn primary tx-commit" type="button" disabled={busy || tx.failed} onclick={() => void commitTx(sessionId)}>
      <Check size={13} strokeWidth={2} aria-hidden="true" />
      {m.tx_commit()}
    </button>
  {/if}
</div>
