<script lang="ts">
  import { untrack } from 'svelte';
  import { ChartArea, ChartBar, ChartColumn, ChartLine, ChartPie, ChartScatter, ChevronDown, FileDown } from '@lucide/svelte';
  import { api } from '../lib/api';
  import { canLog, canStack, columnKind, defaults, fileName, fit, hit, layout, paint, SERIES, toSvg, TOPS, topOf, valueFormat, type Agg, type ChartColumn as Col, type ChartKind, type ChartSettings, type ChartSort, type Prepared, type Role } from '../lib/chart';
  import { charts, prepareLater } from '../lib/chart.svelte';
  import { num, tag } from '../lib/format';
  import { m } from '../paraglide/messages';
  import Menu from './Menu.svelte';
  import Popover from './Popover.svelte';
  import Select from './Select.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { regrouped, type GroupValue } from '../lib/summary';

  type Row = (string | null)[];
  interface Props {
    columns: Col[];
    rows: Row[];
    rev: number;
    key: string;
    name: string;
    partial?: number;
    groups?: ((x: string, values: GroupValue[]) => Promise<Row[]>) | null;
  }
  let { columns, rows, rev, key, name, partial = 0, groups = null }: Props = $props();

  const KINDS: { value: ChartKind; label: () => string; icon: typeof ChartBar }[] = [
    { value: 'bar', label: m.chart_bar, icon: ChartColumn },
    { value: 'hbar', label: m.chart_hbar, icon: ChartBar },
    { value: 'line', label: m.chart_line, icon: ChartLine },
    { value: 'area', label: m.chart_area, icon: ChartArea },
    { value: 'pie', label: m.chart_pie, icon: ChartPie },
    { value: 'scatter', label: m.chart_scatter, icon: ChartScatter },
  ];
  const AGGS: { value: Agg; label: () => string }[] = [
    { value: 'none', label: m.chart_agg_none },
    { value: 'sum', label: m.chart_agg_sum },
    { value: 'count', label: m.chart_agg_count },
    { value: 'avg', label: m.chart_agg_avg },
    { value: 'min', label: m.chart_agg_min },
    { value: 'max', label: m.chart_agg_max },
  ];
  const SORTS: { value: ChartSort; label: () => string }[] = [
    { value: 'none', label: m.chart_sort_none },
    { value: 'x', label: m.chart_sort_x },
    { value: 'desc', label: m.chart_sort_desc },
    { value: 'asc', label: m.chart_sort_asc },
  ];

  const locale = tag();
  const fmt = valueFormat(locale);
  const sample = $derived((void rev, rows.slice(0, 200)));
  const kinds = $derived(columns.map((c, i) => columnKind(c.type_name, sample.map((r) => r[i] ?? null))));
  const numeric = $derived(columns.filter((_, i) => kinds[i] === 'number'));

  function distinct(c: number) {
    const seen = new Set<string | null>();
    const head = rows.slice(0, 2000);
    for (const r of head) seen.add(r[c] ?? null);
    return seen.size === head.length;
  }

  const settings = $derived(fit(charts.settings[key], columns, kinds, () => defaults(columns, kinds, distinct)));

  function set(patch: Partial<ChartSettings>) {
    const next = { ...settings, ...patch };
    if (patch.x !== undefined) {
      const c = columns.findIndex((col) => col.name === patch.x);
      if (kinds[c] === 'text' && ['line', 'area', 'scatter'].includes(next.kind)) next.kind = 'bar';
      if (next.agg === 'none' && !distinct(c)) next.agg = next.ys.length ? 'sum' : 'count';
    }
    if (next.kind === 'scatter') next.agg = 'none';
    if (patch.kind === 'pie' && (!next.top || next.top > SERIES - 1)) next.top = topOf(SERIES - 1);
    charts.settings[key] = next;
  }

  const xi = $derived(columns.findIndex((c) => c.name === settings.x));
  const xKind = $derived(kinds[xi] ?? 'text');
  const ys = $derived(settings.kind === 'pie' ? settings.ys.slice(0, 1) : settings.ys);
  const stackable = $derived(canStack(settings.kind) && ys.length > 1);
  const loggable = $derived(canLog(settings.kind) && !(stackable && settings.stacked));
  $effect(() =>
    registerCommands([
      { id: 'chart.stacked', label: m.chart_cmd_stacked, group: m.pal_group_panels, when: () => stackable, run: () => set({ stacked: !settings.stacked }) },
      { id: 'chart.log', label: m.chart_cmd_log, group: m.pal_group_panels, when: () => loggable, run: () => set({ log: !settings.log }) },
    ]),
  );

  const problem = $derived(settings.kind === 'scatter' && xKind === 'text' ? m.chart_need_x() : !ys.length && settings.agg !== 'count' ? m.chart_need_y() : null);

  let data = $state.raw<Prepared | null>(null);
  let busy = $state(false);

  const full = $derived(!!charts.full[key] && !!groups && partial > 0 && settings.agg !== 'none' && !problem && xi >= 0);
  const wanted = $derived(full ? { x: settings.x, values: ys.length ? ys.map((y): GroupValue => ({ column: y, func: settings.agg === 'none' ? 'sum' : settings.agg })) : [{ column: null, func: 'count' as const }] } : null);
  let server = $state.raw<{ sig: string; rows: Row[] } | null>(null);
  let fetching = $state(false);
  let serverError = $state<string | null>(null);

  $effect(() => {
    const w = wanted;
    const ask = groups;
    if (!w || !ask) {
      server = null;
      return;
    }
    const sig = JSON.stringify(w);
    if (untrack(() => server?.sig) === sig) return;
    let live = true;
    fetching = true;
    serverError = null;
    ask(w.x, w.values)
      .then((got) => {
        if (live) server = { sig, rows: got };
      })
      .catch((e: unknown) => {
        if (!live) return;
        serverError = e instanceof Error ? e.message : String(e);
        charts.full[key] = false;
      })
      .finally(() => {
        if (live) fetching = false;
      });
    return () => (live = false);
  });

  $effect(() => {
    void rev;
    if (problem || xi < 0) {
      data = null;
      return;
    }
    const grouped = full ? server : null;
    if (full && !grouped) return;
    const all = grouped ? grouped.rows : rows;
    const input = {
      xs: all.map((r) => (grouped ? r[0] : r[xi]) ?? null),
      xKind,
      ys: grouped
        ? (ys.length ? ys : ['']).map((y, i) => ({ name: y, values: all.map((r) => r[i + 1] ?? null) }))
        : ys.map((y) => {
            const c = columns.findIndex((col) => col.name === y);
            return { name: y, values: all.map((r) => r[c] ?? null) };
          }),
      agg: grouped ? regrouped(settings.agg === 'none' ? 'sum' : settings.agg) : settings.agg,
      sort: settings.sort,
      top: settings.top,
      kind: settings.kind,
      otherLabel: m.chart_other(),
      locale,
    };
    const out = prepareLater(input);
    if (!(out instanceof Promise)) {
      data = out;
      busy = false;
      return;
    }
    busy = true;
    let live = true;
    void out.then((p) => {
      if (!live) return;
      data = p;
      busy = false;
    });
    return () => (live = false);
  });

  const AGG_NAME: Record<Agg, () => string> = { none: () => '', sum: m.chart_agg_sum, count: m.chart_agg_count, avg: m.chart_agg_avg, min: m.chart_agg_min, max: m.chart_agg_max };
  const seriesLabel = (y: string) => (!y ? m.chart_rows() : settings.agg === 'none' ? y : m.chart_of({ agg: AGG_NAME[settings.agg](), column: y }));
  const legend = $derived((data?.series ?? []).map((s) => seriesLabel(s.name)));

  let width = $state(0);
  let height = $state(0);
  const scene = $derived(data && data.cats.length && width > 80 && height > 80 ? layout(data, { width, height, kind: settings.kind, legend, locale, centerLabel: legend[0] ?? '', stacked: settings.stacked, log: settings.log }) : null);

  const VAR: Record<string, string> = { ink: '--nd-ink', 'ink-2': '--nd-ink-2', 'ink-3': '--nd-ink-3', rule: '--nd-rule', surface: '--nd-surface' };
  const cssVar = (r: Role) => VAR[r] ?? `--nd-chart-${Number(r.slice(1)) + 1}`;
  const paintOf = (r: Role | undefined) => (r ? `var(${cssVar(r)})` : 'none');

  let active = $state<number | null>(null);
  let pointer = $state<{ x: number; y: number } | null>(null);
  let stage = $state<HTMLDivElement>();
  let svg = $state<SVGSVGElement>();

  $effect(() => {
    void scene;
    if (active !== null && scene && active >= (data?.cats.length ?? 0)) active = null;
  });

  function move(e: PointerEvent) {
    if (!scene || !svg) return;
    const box = svg.getBoundingClientRect();
    const x = e.clientX - box.left;
    const y = e.clientY - box.top;
    active = hit(scene, x, y);
    pointer = active === null ? null : { x, y };
  }

  function onkeydown(e: KeyboardEvent) {
    const n = data?.cats.length ?? 0;
    if (!n) return;
    const back = ['ArrowLeft', 'ArrowUp'].includes(e.key);
    const fwd = ['ArrowRight', 'ArrowDown'].includes(e.key);
    let next: number | null = null;
    if (back || fwd) next = active === null ? (fwd ? 0 : n - 1) : Math.max(0, Math.min(n - 1, active + (fwd ? 1 : -1)));
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = n - 1;
    else if (e.key === 'Escape' && active !== null) next = null;
    else return;
    e.preventDefault();
    active = next;
    pointer = null;
  }

  const total = $derived((data?.series[0]?.values ?? []).reduce<number>((a, v) => a + Math.max(0, v ?? 0), 0));
  const tip = $derived.by(() => {
    if (active === null || !data) return null;
    const cat = data.cats[active];
    if (!cat) return null;
    const lines = data.series.map((s, k) => ({ k, name: legend[k] ?? s.name, value: fmt(s.values[active ?? 0] ?? null) }));
    const share = settings.kind === 'pie' && total ? m.chart_share({ share: new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 1 }).format(Math.max(0, data.series[0]?.values[active] ?? 0) / total) }) : null;
    return { title: cat.label, lines, share };
  });
  const tipText = $derived(tip ? [tip.title, ...tip.lines.map((l) => `${l.name} ${l.value}`), tip.share ?? ''].filter(Boolean).join(', ') : '');

  const anchor = $derived.by(() => {
    if (!scene || active === null) return null;
    if (pointer) return pointer;
    const marks = scene.marks.filter((mk) => mk.cat === active);
    let x = 0;
    let y = Infinity;
    for (const mk of marks) {
      const s = mk.shape;
      if (s.t === 'rect') {
        x = s.x + s.w / 2;
        y = Math.min(y, settings.kind === 'hbar' ? s.y + s.h / 2 : s.y);
        if (settings.kind === 'hbar') x = s.x + s.w;
      } else if (s.t === 'circle') {
        x = s.cx;
        y = Math.min(y, s.cy);
      }
    }
    const p = scene.pick;
    if (p.kind === 'pie') {
      const a = ((p.angles[active] ?? 0) + (p.angles[active + 1] ?? 0)) / 2;
      return { x: p.cx + p.r1 * Math.sin(a), y: p.cy - p.r1 * Math.cos(a) };
    }
    return { x, y: y === Infinity ? scene.plot.y : y };
  });

  const tipLeft = $derived(anchor ? anchor.x > width - 220 : false);

  const summary = $derived(m.chart_label({ kind: KINDS.find((k) => k.value === settings.kind)?.label() ?? '', what: legend.join(', ') || m.chart_rows(), x: settings.x }));

  let yOpen = $state(false);
  let saved = $state<string | null>(null);

  function colors(): Record<string, string> {
    const cs = stage ? getComputedStyle(stage) : null;
    const out: Record<string, string> = {};
    const roles: Role[] = ['ink', 'ink-2', 'ink-3', 'rule', 'surface', ...Array.from({ length: SERIES }, (_, i) => `s${i}` as Role)];
    for (const r of roles) out[r] = cs?.getPropertyValue(cssVar(r)).trim() || '#000';
    return out;
  }

  function fonts() {
    const cs = stage ? getComputedStyle(stage) : null;
    return { ui: cs?.getPropertyValue('--nd-font-ui').trim() || 'sans-serif', mono: cs?.getPropertyValue('--nd-font-mono').trim() || 'monospace' };
  }

  async function exportAs(ext: 'svg' | 'png') {
    if (!scene) return;
    const file = fileName(name, ext);
    let bytes: Uint8Array;
    if (ext === 'svg') bytes = new TextEncoder().encode(toSvg(scene, colors(), fonts()));
    else {
      const k = 2;
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(scene.width * k);
      canvas.height = Math.round(scene.height * k);
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.scale(k, k);
      paint(ctx, scene, colors(), fonts());
      const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
      if (!blob) return;
      bytes = new Uint8Array(await blob.arrayBuffer());
    }
    if (await api.exportSave(file, bytes)) {
      saved = file;
      setTimeout(() => (saved = null), 4000);
    }
  }

  function toggleY(name: string) {
    const on = settings.ys.includes(name);
    const next = on ? settings.ys.filter((y) => y !== name) : [...settings.ys, name];
    set({ ys: columns.filter((c) => next.includes(c.name)).map((c) => c.name), ...(!on && settings.agg === 'count' && !settings.ys.length ? { agg: 'sum' as Agg } : {}) });
  }

  const kindButtons: HTMLButtonElement[] = $state([]);
  function kindKey(e: KeyboardEvent, i: number) {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const next = (i + step + KINDS.length) % KINDS.length;
    const k = KINDS[next];
    if (!k) return;
    set({ kind: k.value });
    kindButtons[next]?.focus();
  }
</script>

<section class="chart" aria-label={m.chart_tab()}>
  <div class="chart-bar">
    <div class="chart-kinds" role="radiogroup" aria-label={m.chart_kind()}>
      {#each KINDS as k, i (k.value)}
        {@const Icon = k.icon}
        <button bind:this={kindButtons[i]} type="button" role="radio" aria-checked={settings.kind === k.value} aria-label={k.label()} data-tip={k.label()} tabindex={settings.kind === k.value ? 0 : -1} onclick={() => set({ kind: k.value })} onkeydown={(e) => kindKey(e, i)}>
          <Icon size={15} strokeWidth={1.7} aria-hidden="true" />
        </button>
      {/each}
    </div>
    <label class="chart-field">
      <span>{m.chart_x()}</span>
      <Select class="tool-select" label={m.chart_x()} value={settings.x} onchange={(v) => set({ x: v })} options={columns.map((c) => ({ value: c.name, label: c.name }))} />
    </label>
    <div class="chart-field">
      <span>{m.chart_y()}</span>
      <Popover label={m.chart_y_label()} bind:open={yOpen} class="btn chart-pick">
        {#snippet trigger()}
          <span class="chart-pick-text">{ys.length ? ys.join(', ') : settings.agg === 'count' ? m.chart_rows() : m.chart_y_none()}</span>
          <ChevronDown size={12} strokeWidth={1.8} aria-hidden="true" />
        {/snippet}
        <div class="chart-ys">
          {#each numeric as c (c.name)}
            <label><input type="checkbox" checked={settings.ys.includes(c.name)} onchange={() => toggleY(c.name)} /><code>{c.name}</code><span>{c.type_name}</span></label>
          {:else}
            <p>{m.chart_y_none()}</p>
          {/each}
        </div>
        <div class="chart-axis" role="group" aria-label={m.chart_axis()}>
          <label class:off={!stackable}><input type="checkbox" checked={stackable && !!settings.stacked} disabled={!stackable} onchange={() => set({ stacked: !settings.stacked })} />{m.chart_stacked()}</label>
          <label class:off={!loggable} data-tip={stackable && settings.stacked ? m.chart_log_stacked() : undefined}><input type="checkbox" checked={loggable && !!settings.log} disabled={!loggable} onchange={() => set({ log: !settings.log })} />{m.chart_log()}</label>
        </div>
      </Popover>
    </div>
    <label class="chart-field">
      <span>{m.chart_agg()}</span>
      <Select class="tool-select" label={m.chart_agg()} value={settings.agg} disabled={settings.kind === 'scatter'} onchange={(v) => set({ agg: v })} options={AGGS.map((a) => ({ value: a.value, label: a.label() }))} />
    </label>
    <label class="chart-field">
      <span>{m.chart_sort()}</span>
      <Select class="tool-select" label={m.chart_sort()} value={settings.sort} onchange={(v) => set({ sort: v })} options={SORTS.map((s) => ({ value: s.value, label: s.label() }))} />
    </label>
    <label class="chart-field">
      <span>{m.chart_top()}</span>
      <Select class="tool-select" label={m.chart_top()} value={settings.top} onchange={(v) => set({ top: v })} options={TOPS.map((t) => ({ value: t, label: t ? m.chart_top_n({ n: t }) : m.chart_top_all() }))} />
    </label>
    <span class="spacer"></span>
    {#if saved}<span class="chart-saved" role="status">{m.chart_saved({ name: saved })}</span>{/if}
    <Menu label={m.chart_export()} items={[{ value: 'svg' as const, label: m.chart_export_svg() }, { value: 'png' as const, label: m.chart_export_png() }]} onpick={(v) => void exportAs(v)} disabled={!scene}>
      <FileDown size={14} strokeWidth={1.8} aria-hidden="true" />
      {m.chart_export()}
    </Menu>
  </div>

  <div class="chart-stage" class:busy={busy || fetching} bind:this={stage} bind:clientWidth={width} bind:clientHeight={height}>
    {#if problem}
      <p class="chart-empty">{problem}</p>
    {:else if data && !data.cats.length}
      <p class="chart-empty">{busy ? m.chart_busy() : m.chart_empty()}</p>
    {:else if scene}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <svg
        bind:this={svg}
        class="chart-svg"
        width={scene.width}
        height={scene.height}
        viewBox="0 0 {scene.width} {scene.height}"
        role="img"
        tabindex="0"
        aria-label={summary}
        aria-describedby="{key}-keys"
        onpointermove={move}
        onpointerleave={() => ((active = null), (pointer = null))}
        onkeydown={onkeydown}
        onblur={() => !pointer && (active = null)}
      >
        {#each scene.prims as p, i (i)}
          {#if p.t === 'rect'}
            <rect x={p.x} y={p.y} width={p.w} height={p.h} style:fill={paintOf(p.fill)} style:fill-opacity={p.opacity} />
          {:else if p.t === 'line'}
            <line x1={p.x1} y1={p.y1} x2={p.x2} y2={p.y2} style:stroke={paintOf(p.stroke)} stroke-width={p.width} stroke-dasharray={p.dash} />
          {:else if p.t === 'path'}
            <path d={p.d} style:fill={paintOf(p.fill)} style:stroke={paintOf(p.stroke)} stroke-width={p.width} style:fill-opacity={p.opacity} stroke-linejoin="round" />
          {:else if p.t === 'circle'}
            <circle cx={p.cx} cy={p.cy} r={p.r} style:fill={paintOf(p.fill)} style:fill-opacity={p.opacity} />
          {:else}
            <text x={p.x} y={p.y} text-anchor={p.anchor} font-size={p.size} font-weight={p.weight} class:mono={p.mono} style:fill={paintOf(p.fill)}>{p.text}</text>
          {/if}
        {/each}
        {#if active !== null}
          {#if (settings.kind === 'line' || settings.kind === 'area') && scene.pick.kind === 'x'}
            {@const x = scene.pick.at[active] ?? 0}
            <line class="chart-cross" x1={x} x2={x} y1={scene.plot.y} y2={scene.plot.y + scene.plot.h} />
          {/if}
          {#each scene.marks.filter((mk) => mk.cat === active) as mk (mk.series)}
            {@const s = mk.shape}
            {#if s.t === 'rect'}
              <rect class="chart-on" x={s.x} y={s.y} width={s.w} height={s.h} style:fill={paintOf(s.fill)} />
            {:else if s.t === 'circle'}
              <circle class="chart-on" cx={s.cx} cy={s.cy} r={s.r} style:fill={paintOf(s.fill)} />
            {:else if s.t === 'path'}
              <path class="chart-on" d={s.d} style:fill={paintOf(s.fill)} />
            {/if}
          {/each}
        {/if}
      </svg>
      {#if tip && anchor}
        <div class="chart-tip" class:left={tipLeft} style:left="{anchor.x}px" style:top="{anchor.y}px" aria-hidden="true">
          <strong>{tip.title}</strong>
          {#each tip.lines as l (l.k)}
            <span class="chart-tip-row"><i style:background={paintOf(`s${l.k % SERIES}`)}></i><span>{l.name}</span><b>{l.value}</b></span>
          {/each}
          {#if tip.share}<span class="chart-tip-share">{tip.share}</span>{/if}
        </div>
      {/if}
    {/if}
  </div>
  <p id="{key}-keys" class="chart-sr">{m.chart_keys()}</p>
  <p class="chart-sr" aria-live="polite">{tipText}</p>
  {#if partial && groups}
    <p class="chart-note chart-scope" data-scope={full && server ? 'all' : 'loaded'}>
      {#if full && server}<span>{m.chart_full_on()}</span><button type="button" class="chart-scope-btn" onclick={() => (charts.full[key] = false)}>{m.chart_full_off()}</button>
      {:else if settings.agg === 'none'}<span>{m.chart_partial_none({ count: num(partial) })}</span>
      {:else}<span>{m.chart_partial({ count: num(partial) })}</span><button type="button" class="chart-scope-btn" disabled={fetching} onclick={() => (charts.full[key] = true)}>{m.chart_full()}</button>{/if}
      {#if serverError}<span class="chart-scope-err">{m.chart_full_failed({ message: serverError })}</span>{/if}
    </p>
  {/if}
  {#if data && (data.skipped || data.dropped || scene?.hidden)}
    <p class="chart-note">
      {#if data.dropped}{m.chart_dropped({ count: num(data.cats.length) })}{/if}
      {#if data.skipped}{data.skipped === 1 ? m.chart_skipped_one() : m.chart_skipped_other({ count: num(data.skipped) })}{/if}
      {#if scene?.hidden}{m.chart_log_hidden({ count: num(scene.hidden) })}{/if}
    </p>
  {/if}
</section>
