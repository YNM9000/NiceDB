<script lang="ts">
  import { num } from '../lib/format';
  import { previewRows } from '../lib/tx';
  import type { ResultSet, Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';

  let { tab }: { tab: Tab } = $props();

  const SHOWN = 200;
  const writes = $derived(tab.results.filter((r) => r.preview || (r.done?.rowsAffected ?? null) !== null));

  function kind(r: ResultSet) {
    if (r.preview) return r.preview.kind === 'update' ? m.tx_dry_updated() : m.tx_dry_deleted();
    return /^\s*insert/i.test(r.sql) ? m.tx_dry_inserted() : (r.done?.command ?? '');
  }

  function touched(r: ResultSet) {
    const n = r.done?.rowsAffected ?? r.preview?.before.length ?? 0;
    const text = n === 1 ? m.tx_touched_one() : m.tx_touched_other({ count: num(n) });
    const shown = r.preview?.truncated || (!r.preview && r.count > SHOWN);
    return shown ? `${text} · ${m.tx_dry_first({ count: SHOWN })}` : text;
  }

  const cell = (v: string | null) => v ?? 'NULL';
</script>

<div class="dry-view">
  {#if writes.length === 0}
    <p class="results-empty">{m.tx_dry_empty()}</p>
  {/if}
  {#each writes as r (r.index)}
    <section class="dry-st" aria-label={kind(r)}>
      <header class="dry-head">
        <span class="dry-kind">{kind(r)}</span>
        <code class="dry-sql">{r.sql.length > 140 ? `${r.sql.slice(0, 140)}…` : r.sql}</code>
        <span class="dry-count">{touched(r)}</span>
      </header>
      {#if r.preview}
        {@const p = r.preview}
        {@const changed = new Set(p.changed)}
        {#if p.kind === 'update' && !p.measured}<p class="dry-none dry-estimate">{m.tx_dry_estimate()}</p>{/if}
        <div class="dry-scroll">
          <table class="dry-table" class:deleted={p.kind === 'delete'}>
            <thead>
              <tr>
                {#each p.columns as c, i (i)}<th class:changed={changed.has(i)} scope="col">{c.name}</th>{/each}
              </tr>
            </thead>
            <tbody>
              {#each previewRows(p) as row, r (r)}
                <tr>
                  {#each row as c, i (i)}
                    <td class:changed={c.changed} class:null={c.before === null && !c.changed}>
                      {#if c.changed}
                        <del aria-label={`${m.tx_before()}: ${cell(c.before)}`}>{cell(c.before)}</del><ins aria-label={`${m.tx_after()}: ${cell(c.after)}`}>{cell(c.after)}</ins>
                      {:else}{cell(c.before)}{/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else if r.hasGrid}
        <div class="dry-scroll">
          <table class="dry-table inserted">
            <thead>
              <tr>{#each r.columns as c, i (i)}<th scope="col">{c.name}</th>{/each}</tr>
            </thead>
            <tbody>
              {#each r.rows.slice(0, SHOWN) as row, k (k)}
                <tr>{#each row as v, i (i)}<td class:null={v === null}>{cell(v)}</td>{/each}</tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="dry-none">{m.tx_dry_no_preview()}</p>
      {/if}
    </section>
  {/each}
</div>
