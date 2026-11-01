<script lang="ts">
  import { ClipboardCopy, Undo2 } from '@lucide/svelte';
  import type { SessionInfo } from '../lib/api';
  import { appearance } from '../lib/appearance.svelte';
  import type { Target } from '../lib/changeset';
  import type { DataTab } from '../lib/data.svelte';
  import { m } from '../paraglide/messages';

  let { d, session }: { d: DataTab; session: SessionInfo } = $props();

  const TITLE = { registro: m.data_panel_registro, tavola: m.data_panel_tavola, banco: m.data_panel_banco, listino: m.data_panel_listino };

  const built = $derived((void d.rev, d.pending?.build() ?? { set: null, targets: [] as Target[] }));

  $effect(() => {
    void d.rev;
    void d.refreshPreview(session).catch(() => {});
  });

  function keyText(key: string): string {
    const info = d.info;
    if (!info) return key;
    const values = JSON.parse(key) as (string | null)[];
    return info.key.map((c, i) => `${info.columns[c]?.name ?? ''} ${values[i] ?? 'NULL'}`).join(', ');
  }

  function label(t: Target): string {
    if (t.kind === 'insert') return m.data_rev_new();
    if (t.kind === 'delete') return m.data_rev_deleted({ key: keyText(t.key) });
    return `${keyText(t.key)} · ${t.columns.join(', ')}`;
  }

  function revert(t: Target) {
    d.pending?.revert(t);
    d.touch();
  }

  async function copy() {
    const p = d.preview;
    if (p) await navigator.clipboard.writeText([p.begin, ...p.statements, p.commit].map((s) => `${s};`).join('\n'));
  }
</script>

<section class="dv-panel" aria-label={TITLE[appearance.value.style]()}>
  <header class="dv-panel-head">
    <h3 class="dv-panel-title">{TITLE[appearance.value.style]()}</h3>
    <span class="dv-panel-note">{m.data_panel_note()}</span>
    <span class="spacer"></span>
    <button class="btn ghost" type="button" onclick={() => void copy()} disabled={!d.preview}>
      <ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.data_copy_sql()}
    </button>
  </header>
  <ol class="dv-revs">
    {#if d.preview}<li class="dv-rev tx"><span class="dv-rev-n"></span><span class="dv-rev-label"></span><code>{d.preview.begin};</code><span></span></li>{/if}
    {#each built.targets as t, i (i)}
      {@const mark = d.pending?.mark(t)}
      <li class="dv-rev {t.kind}" class:conflict={mark?.kind === 'conflict'} class:failed={mark?.kind === 'error'}>
        <span class="dv-rev-n" data-n={i + 1}>{i + 1}</span>
        <span class="dv-rev-label">{label(t)}</span>
        <code>{d.preview?.statements[i] ?? ''};</code>
        <button class="icon-btn" type="button" aria-label={m.data_rev_revert()} data-tip={m.data_rev_revert()} onclick={() => revert(t)}><Undo2 size={13} strokeWidth={1.8} /></button>
        {#if mark}<span class="dv-rev-err">{mark.kind === 'conflict' ? m.data_conflict_row() : mark.message}</span>{/if}
      </li>
    {/each}
    {#if d.preview}<li class="dv-rev tx"><span class="dv-rev-n"></span><span class="dv-rev-label"></span><code>{d.preview.commit};</code><span></span></li>{/if}
  </ol>
</section>
