<script lang="ts">
  import { api, ApiError, type SessionInfo } from '../lib/api';
  import type { CoreError } from '../lib/bindings';
  import type { ForeignKey } from '../lib/bindings';
  import type { TableInfo } from '../lib/data.svelte';
  import { labelColumns } from '../lib/fk';
  import { num } from '../lib/format';
  import { rowsOf, Stale } from '../lib/related';
  import { m } from '../paraglide/messages';

  const LIMIT = 40;

  interface Props {
    session: SessionInfo;
    target: TableInfo;
    fk: ForeignKey;
    column: string;
    text: string;
    original: string | null;
    anchor: DOMRect;
    onpick: (values: Map<string, string | null>, step: boolean) => void;
  }
  let { session, target, fk, column, text, original, anchor, onpick }: Props = $props();

  const shown = $derived([...fk.ref_columns, ...labelColumns(target.columns, fk.ref_columns)]);
  const at = $derived(shown.indexOf(column));
  let rows = $state.raw<(string | null)[][]>([]);
  let total = $state<number | null>(null);
  let more = $state(false);
  let loading = $state(true);
  let failure = $state<string | null>(null);
  let active = $state(-1);
  let list = $state<HTMLElement>();
  let seq = 0;
  let job: string | null = null;

  const fault = (e: unknown): CoreError | null => (e instanceof ApiError ? e.error : e && typeof e === 'object' && 'code' in e ? (e as CoreError) : null);

  function stopSearch() {
    if (job) void api.jobCancel(job).catch(() => {});
    job = null;
  }

  $effect(() => stopSearch);

  const search = $derived(text.trim() === (original ?? '').trim() ? '' : text.trim());

  $effect(() => {
    const q = search;
    const mine = ++seq;
    loading = true;
    stopSearch();
    const t = setTimeout(async () => {
      const id = crypto.randomUUID();
      job = id;
      try {
        const stale = () => mine !== seq;
        const page = await rowsOf(session.session_id, target, shown, [], q || null, LIMIT, stale, id);
        const known = q || original === null || page.rows.some((r) => r[at] === original);
        const current = known ? [] : (await rowsOf(session.session_id, target, shown, [{ column, op: 'eq', value: original }], null, 1, stale)).rows;
        if (mine !== seq) return;
        rows = [...current, ...page.rows];
        total = page.total;
        more = page.has_more;
        failure = null;
        active = q ? (rows.length ? 0 : -1) : rows.findIndex((r) => r[at] === original);
      } catch (e) {
        const f = fault(e);
        const cancelled = f?.code === 'driver' && f.detail.kind === 'cancelled';
        const slow = f?.code === 'driver' && f.detail.kind === 'timeout';
        if (mine === seq && !(e instanceof Stale) && !cancelled) {
          rows = [];
          failure = slow ? m.fk_picker_slow() : e instanceof Error ? e.message : String(e);
        }
      } finally {
        if (job === id) job = null;
        if (mine === seq) loading = false;
      }
    }, q ? 260 : 0);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const el = list?.querySelector<HTMLElement>(`[data-i="${active}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  });

  function choose(i: number, step: boolean) {
    const r = rows[i];
    if (!r) return;
    onpick(new Map(fk.ref_columns.map((c, k) => [fk.columns[k] ?? c, r[k] ?? null])), step);
  }

  export function key(e: KeyboardEvent): boolean {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = rows.length;
      if (!n) return true;
      active = e.key === 'ArrowDown' ? Math.min(n - 1, active + 1) : Math.max(-1, active - 1);
      return true;
    }
    if (e.key === 'PageDown' || e.key === 'PageUp') {
      e.preventDefault();
      active = Math.max(0, Math.min(rows.length - 1, active + (e.key === 'PageDown' ? 8 : -8)));
      return true;
    }
    if ((e.key === 'Enter' || e.key === 'Tab') && active >= 0 && rows[active]?.[at] !== original) {
      e.preventDefault();
      choose(active, e.key === 'Enter');
      return true;
    }
    return false;
  }

  const below = $derived(anchor.bottom + 320 < window.innerHeight);
  const left = $derived(Math.max(8, Math.min(anchor.left, window.innerWidth - 440)));
</script>

<div
  class="fkp"
  style:left="{left}px"
  style:top={below ? `${anchor.bottom + 3}px` : undefined}
  style:bottom={below ? undefined : `${window.innerHeight - anchor.top + 3}px`}
  style:min-width="{Math.max(anchor.width, 300)}px"
  role="presentation"
  onpointerdown={(e) => e.preventDefault()}
>
  <header class="fkp-head">
    <span class="fkp-table">{m.fk_picker({ table: target.name })}</span>
    <span class="fkp-count">{loading ? m.fk_loading() : [total === null ? '' : total === 1 ? m.fk_rows_one() : m.fk_rows_other({ count: num(total) }), more ? m.fk_picker_more({ count: LIMIT }) : ''].filter(Boolean).join(' · ')}</span>
  </header>
  {#if failure}
    <p class="fkp-note err" role="alert">{failure}</p>
  {:else if !loading && rows.length === 0}
    <p class="fkp-note">{m.fk_picker_empty()}</p>
  {:else}
    <div class="fkp-list" role="listbox" aria-label={m.fk_picker({ table: target.name })} bind:this={list} style:--fkp-cols={shown.length}>
      <div class="fkp-cols" aria-hidden="true">
        {#each shown as c (c)}<span>{c}</span>{/each}
      </div>
      {#each rows as r, i (i)}
        <div class="fkp-row" class:active={i === active} class:current={original !== null && r[at] === original} role="option" aria-selected={i === active} data-i={i} tabindex="-1" onpointerenter={() => (active = i)} onclick={() => choose(i, false)} onkeydown={() => {}}>
          {#each r as v, k (k)}
            <span class:key={k < fk.ref_columns.length} class:null={v === null}>{v ?? 'NULL'}</span>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
  <footer class="fkp-foot">{m.fk_picker_keys()}</footer>
</div>
