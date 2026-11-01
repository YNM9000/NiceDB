<script lang="ts">
  import { tabKey } from '../lib/tabs';
  import { Binary, ChevronDown, ChevronUp, ClipboardCopy, Download, FileUp, Pencil, Search, X } from '@lucide/svelte';
  import { api } from '../lib/api';
  import type { Column, Dialect } from '../lib/bindings';
  import { toBase64 } from '../lib/bytes';
  import { copyText, keys } from '../lib/contextmenu.svelte';
  import { recordText } from '../lib/copy';
  import { num, tag } from '../lib/format';
  import { byteLength, bytesOf, fileName, formatXml, hexDump, hexLines, hexOfBytes, isBinaryColumn, isSvgText, kindOf, magicOf, parseHexInput, parseJson, sizeText, type ValueKind } from '../lib/value';
  import { tiffPreview } from '../lib/tiff';
  import type { ValueMode } from '../lib/valuepanel.svelte';
  import { m } from '../paraglide/messages';
  import Resizer from './Resizer.svelte';
  import ValueJson from './ValueJson.svelte';
  import ValueText from './ValueText.svelte';

  interface Props {
    columns: readonly Column[];
    count: number;
    focus: { r: number; c: number } | null;
    at: (r: number, c: number) => string | null;
    rowLabel: (r: number) => string;
    dialect: Dialect | null;
    mode: ValueMode;
    lock?: ((r: number, c: number) => string | null) | undefined;
    commit?: ((r: number, c: number, value: string | null) => boolean) | undefined;
    onmove: (r: number, c: number) => void;
    onmode: (mode: ValueMode) => void;
    onclose: () => void;
  }
  let { columns, count, focus, at, rowLabel, dialect, mode, lock, commit, onmove, onmode, onclose }: Props = $props();

  const IMAGE_LIMIT = 16 * 1024 * 1024;
  const HEX_EDIT_LIMIT = 1024 * 1024;

  const col = $derived(focus ? columns[focus.c] : undefined);
  const type = $derived(col?.type_name ?? null);
  const raw = $derived(focus ? at(focus.r, focus.c) : null);
  const kind = $derived<ValueKind>(focus ? kindOf(raw, type) : 'null');
  const bin = $derived(kind === 'image' || kind === 'binary');
  const magic = $derived(raw !== null && bin ? magicOf(raw) : null);
  const parsed = $derived(kind === 'json' && raw !== null ? parseJson(raw) : null);
  const xml = $derived(kind === 'xml' && raw !== null ? formatXml(raw) : '');
  const why = $derived(focus && lock ? lock(focus.r, focus.c) : null);
  const binColumn = $derived(isBinaryColumn(type));
  const canEdit = $derived(!!focus && !!lock && !!commit && why === null && !bin && !binColumn);
  const canEditBin = $derived(!!focus && !!lock && !!commit && why === null && (bin || binColumn));
  const tooBig = $derived(raw !== null && bin && byteLength(raw) > HEX_EDIT_LIMIT);
  const svgText = $derived((kind === 'xml' || kind === 'text') && raw !== null && isSvgText(raw));

  let jsonView = $state<'tree' | 'text'>('tree');
  let query = $state('');
  let editing = $state<string | null>(null);
  let hexMode = $state(false);
  let loaded = $state<string | null>(null);
  let svgView = $state<'image' | 'text'>('image');
  let tiff = $state<{ url: string; width: number; height: number } | null>(null);
  let initial = '';
  let problem = $state<string | null>(null);
  let broken = $state(false);
  let dims = $state('');
  let saved = $state<string | null>(null);
  let tree = $state<ReturnType<typeof ValueJson>>();
  let area = $state<HTMLTextAreaElement>();
  let record = $state<HTMLElement>();

  $effect(() => {
    void focus?.r;
    void focus?.c;
    editing = null;
    hexMode = false;
    problem = null;
    broken = false;
    dims = '';
    saved = null;
    loaded = null;
  });

  $effect(() => {
    tiff = null;
    if (magic?.ext !== 'tif' || raw === null || byteLength(raw) > IMAGE_LIMIT) return;
    let live = true;
    void tiffPreview(bytesOf(raw)).then((t) => {
      if (!live) return;
      tiff = t;
      if (!t) broken = true;
    });
    return () => {
      live = false;
    };
  });

  const image = $derived.by(() => {
    if (kind !== 'image' || raw === null || !magic || byteLength(raw) > IMAGE_LIMIT) return null;
    if (magic.ext === 'tif') return tiff?.url ?? null;
    return `data:${magic.mime};base64,${toBase64(bytesOf(raw))}`;
  });

  const svgImage = $derived(svgText && raw !== null ? `data:image/svg+xml;base64,${toBase64(new TextEncoder().encode(raw))}` : null);

  const badge = $derived(
    svgText ? 'SVG' :
    kind === 'json' ? 'JSON' : kind === 'xml' ? 'XML' : kind === 'image' || (kind === 'binary' && magic?.label) ? (magic?.label ?? '') : kind === 'binary' ? m.vp_kind_binary() : kind === 'null' ? 'NULL' : m.vp_kind_text(),
  );

  const meta = $derived.by(() => {
    if (raw === null) return '';
    if (bin) return [sizeText(byteLength(raw), tag()), dims].filter(Boolean).join(' · ');
    const lines = raw.split('\n').length;
    return [raw.length === 1 ? m.vp_chars_one() : m.vp_chars({ count: num(raw.length) }), lines > 1 ? m.vp_lines({ count: num(lines) }) : ''].filter(Boolean).join(' · ');
  });

  async function save() {
    if (!focus || raw === null || !col) return;
    const name = fileName(col.name, rowLabel(focus.r), kind, magic ?? magicOf(''));
    const data = bin ? bytesOf(raw) : new TextEncoder().encode(raw);
    if (await api.exportSave(name, data)) saved = name;
  }

  function startEdit() {
    if (!canEdit) return;
    editing = initial = raw !== null && parsed?.ok && /^jsonb\b/i.test(type ?? '') ? JSON.stringify(parsed.value, null, 2) : (raw ?? '');
    problem = null;
    queueMicrotask(() => area?.focus());
  }

  function startHex() {
    if (!canEditBin || tooBig) return;
    editing = initial = raw !== null && bin ? hexLines(raw, 8) : '';
    hexMode = true;
    problem = null;
    queueMicrotask(() => {
      area?.focus();
      area?.setSelectionRange(0, 0);
      if (area) area.scrollTop = 0;
    });
  }

  async function loadFile() {
    if (!canEditBin || !focus || !commit) return;
    const at = focus;
    const file = await api.fileLoad().catch((e: unknown) => {
      problem = e instanceof Error ? e.message : String(e);
      return null;
    });
    if (!file || !commit(at.r, at.c, hexOfBytes(file.bytes))) return;
    loaded = m.vp_loaded({ name: file.name, size: sizeText(file.bytes.length, tag()) });
  }

  function finish(value: string | null) {
    if (!focus || !commit) return;
    if (value !== null && value === initial) {
      editing = null;
      hexMode = false;
      return;
    }
    if (value !== null && hexMode) {
      const hex = parseHexInput(value);
      if (hex === null) {
        problem = m.vp_hex_invalid();
        return;
      }
      value = hex;
    }
    if (value !== null && type && /^jsonb?\b/i.test(type)) {
      const p = parseJson(value);
      if (!p.ok) {
        problem = m.vp_json_invalid({ message: p.error });
        return;
      }
    }
    if (commit(focus.r, focus.c, value)) {
      editing = null;
      hexMode = false;
      problem = null;
    }
  }

  function prettify() {
    if (editing === null) return;
    const p = parseJson(editing);
    if (p.ok) editing = JSON.stringify(p.value, null, 2);
    else problem = m.vp_json_invalid({ message: p.error });
  }

  function editKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      editing = null;
      hexMode = false;
      problem = null;
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      finish(editing);
    }
  }

  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      tree?.next(e.shiftKey ? -1 : 1);
    } else if (e.key === 'Escape' && query) {
      e.preventDefault();
      e.stopPropagation();
      query = '';
    }
  }

  function panelKey(e: KeyboardEvent) {
    if (e.defaultPrevented || editing !== null || e.target instanceof HTMLInputElement) return;
    if (e.key === 'Escape') onclose();
    else if (e.key === 'F2' && mode === 'value') {
      if (canEditBin) startHex();
      else startEdit();
    }
    else return;
    e.preventDefault();
  }

  const move = (dr: number) => focus && onmove(Math.max(0, Math.min(count - 1, focus.r + dr)), focus.c);

  function recordKey(e: KeyboardEvent) {
    if (!focus) return;
    const steps: Record<string, number> = { ArrowUp: -1, ArrowDown: 1, PageUp: -10, PageDown: 10 };
    const step = steps[e.key];
    if (step !== undefined) move(step);
    else if (e.key === 'Home' && (e.ctrlKey || e.metaKey)) onmove(0, focus.c);
    else if (e.key === 'End' && (e.ctrlKey || e.metaKey)) onmove(count - 1, focus.c);
    else if (e.key === 'Tab') {
      const c = focus.c + (e.shiftKey ? -1 : 1);
      if (c < 0 || c >= columns.length) return;
      onmove(focus.r, c);
      queueMicrotask(() => record?.querySelector('.vp-field.on')?.scrollIntoView({ block: 'nearest' }));
    } else if (e.key === 'Enter') onmode('value');
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') void copyText(raw ?? '');
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function fieldText(v: string | null, t: string | null): string {
    if (v === null) return 'NULL';
    const k = kindOf(v, t);
    if (k === 'image' || k === 'binary') {
      const mg = magicOf(v);
      return `${mg.label || m.vp_kind_binary()} · ${sizeText(byteLength(v), tag())}`;
    }
    const first = v.length > 400 ? v.slice(0, 400) : v;
    const nl = first.indexOf('\n');
    return nl < 0 ? first : `${first.slice(0, nl)} …`;
  }

  const rowValues = $derived(focus ? columns.map((_, c) => at(focus.r, c)) : []);
  const uid = $props.id();
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside class="vp" aria-label={m.vp_title()} onkeydown={panelKey}>
  <Resizer pane="value" side="start" label={m.vp_resize()} />
  <header class="history-head vp-head">
    <div class="results-tabs vp-modes" role="tablist" aria-label={m.vp_title()} tabindex="-1" onkeydown={tabKey}>
      <button id="{uid}-value" type="button" role="tab" aria-selected={mode === 'value'} aria-controls="{uid}-panel" tabindex={mode === 'value' ? 0 : -1} data-tip={keys('Shift+Enter')} onclick={() => onmode('value')}>{m.vp_tab_value()}</button>
      <button id="{uid}-record" type="button" role="tab" aria-selected={mode === 'record'} aria-controls="{uid}-panel" tabindex={mode === 'record' ? 0 : -1} data-tip={keys('Alt+Enter')} onclick={() => onmode('record')}>{m.vp_tab_record()}</button>
    </div>
    <span class="spacer"></span>
    <button class="icon-btn" type="button" aria-label={m.vp_close()} data-tip={m.vp_close()} onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>

  <div class="vp-tabpanel" role="tabpanel" id="{uid}-panel" aria-labelledby="{uid}-{mode}">
  {#if !focus || !col}
    <p class="history-empty vp-empty">{m.vp_no_cell()}</p>
  {:else if mode === 'record'}
    <div class="vp-nav">
      <button class="icon-btn" type="button" aria-label={m.vp_prev_row()} data-tip={`${m.vp_prev_row()} (↑)`} disabled={focus.r === 0} onclick={() => move(-1)}><ChevronUp size={15} strokeWidth={1.8} /></button>
      <button class="icon-btn" type="button" aria-label={m.vp_next_row()} data-tip={`${m.vp_next_row()} (↓)`} disabled={focus.r >= count - 1} onclick={() => move(1)}><ChevronDown size={15} strokeWidth={1.8} /></button>
      <span class="vp-pos">{m.vp_row_of({ row: rowLabel(focus.r), n: num(focus.r + 1), count: num(count) })}</span>
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={() => void copyText(recordText(columns, rowValues))}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" />{m.vp_copy_row()}</button>
    </div>
    <div class="vp-rec" role="listbox" tabindex="0" aria-label={m.vp_tab_record()} aria-activedescendant={`vp-f-${focus.c}`} bind:this={record} onkeydown={recordKey}>
      {#each columns as c, i (i)}
        {@const v = rowValues[i] ?? null}
        <div id={`vp-f-${i}`} class="vp-field" class:on={i === focus.c} role="option" aria-selected={i === focus.c} tabindex="-1" onclick={() => focus && onmove(focus.r, i)} ondblclick={() => onmode('value')} onkeydown={() => {}}>
          <span class="vp-fname">{c.name}{#if c.type_name}<small>{c.type_name}</small>{/if}</span>
          <span class="vp-fval" class:null={v === null} data-tip={v !== null && v.length < 2000 ? v : undefined}>{fieldText(v, c.type_name ?? null)}</span>
        </div>
      {/each}
    </div>
    <p class="vp-foot">{m.vp_record_keys()}</p>
  {:else}
    <div class="vp-sub">
      <div class="vp-what">
        <span class="vp-col">{col.name}</span>
        {#if col.type_name}<span class="vp-type">{col.type_name}</span>{/if}
        <span class="vp-row">{m.vp_row({ row: rowLabel(focus.r) })}</span>
      </div>
      <div class="vp-facts">
        <span class="vp-badge" data-kind={kind}>{badge}</span>
        {#if meta}<span class="vp-meta">{meta}</span>{/if}
      </div>
    </div>
    {#if editing === null && (raw !== null || !!lock)}
      <div class="vp-tools">
        {#if kind === 'json' && parsed?.ok}
          <div class="seg" role="radiogroup" aria-label={m.vp_json_view()}>
            <button type="button" role="radio" aria-checked={jsonView === 'tree'} onclick={() => (jsonView = 'tree')}>{m.vp_json_tree_short()}</button>
            <button type="button" role="radio" aria-checked={jsonView === 'text'} onclick={() => (jsonView = 'text')}>{m.vp_json_text()}</button>
          </div>
          {#if jsonView === 'tree'}
            <label class="field vp-search">
              <Search size={13} strokeWidth={1.8} aria-hidden="true" />
              <input bind:value={query} placeholder={m.vp_json_search()} aria-label={m.vp_json_search()} spellcheck="false" onkeydown={searchKey} />
              {#if query}<span class="vp-hits">{tree?.hitText()}</span>{/if}
            </label>
          {/if}
        {/if}
        {#if svgText}
          <div class="seg" role="radiogroup" aria-label={m.vp_svg_view()}>
            <button type="button" role="radio" aria-checked={svgView === 'image'} onclick={() => (svgView = 'image')}>{m.vp_svg_image()}</button>
            <button type="button" role="radio" aria-checked={svgView === 'text'} onclick={() => (svgView = 'text')}>{m.vp_svg_text()}</button>
          </div>
        {/if}
        {#if kind !== 'json' || jsonView !== 'tree' || !parsed?.ok}<span class="spacer"></span>{/if}
        {#if raw !== null}
          <button class="icon-btn" type="button" aria-label={m.vp_copy()} data-tip={m.vp_copy()} onclick={() => void copyText(raw ?? '')}><ClipboardCopy size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn" type="button" aria-label={m.vp_save()} data-tip={m.vp_save()} onclick={() => void save()}><Download size={14} strokeWidth={1.8} /></button>
        {/if}
        {#if lock && (bin || binColumn)}
          <button class="icon-btn" type="button" aria-label={m.vp_load()} data-tip={why ?? m.vp_load()} disabled={!canEditBin} onclick={() => void loadFile()}><FileUp size={14} strokeWidth={1.8} /></button>
          <button class="btn ghost vp-edit" type="button" disabled={!canEditBin || tooBig} data-tip={why ?? (tooBig ? m.vp_hex_too_big() : keys('F2'))} onclick={startHex}><Binary size={13} strokeWidth={1.8} aria-hidden="true" />{m.vp_hex_edit()}</button>
        {:else if lock}
          <button class="btn ghost vp-edit" type="button" disabled={!canEdit} data-tip={why ?? keys('F2')} onclick={startEdit}><Pencil size={13} strokeWidth={1.8} aria-hidden="true" />{m.vp_edit()}</button>
        {/if}
      </div>
    {/if}
    {#if saved}<p class="vp-note" role="status">{m.vp_saved({ name: saved })}</p>{/if}
    {#if loaded && editing === null}<p class="vp-note" role="status">{loaded}</p>{/if}
    {#if problem && editing === null}<p class="vp-problem" role="alert">{problem}</p>{/if}

    <div class="vp-body" class:flush={editing === null && kind === 'json' && jsonView === 'tree' && !!parsed?.ok}>
      {#if editing !== null}
        <textarea class="vp-area" class:hex={hexMode} bind:this={area} bind:value={editing} spellcheck="false" aria-label={col.name} onkeydown={editKey}></textarea>
        {#if hexMode}<p class="vp-note vp-hex-hint">{m.vp_hex_hint()}</p>{/if}
        {#if problem}<p class="vp-problem" role="alert">{problem}</p>{/if}
        <div class="vp-actions">
          {#if col && /^jsonb?\b/i.test(type ?? '')}<button class="btn ghost" type="button" onclick={prettify}>{m.vp_format()}</button>{/if}
          <button class="btn ghost" type="button" onclick={() => finish(null)}>NULL</button>
          <span class="spacer"></span>
          <button class="btn ghost" type="button" onclick={() => ((editing = null), (hexMode = false), (problem = null))}>{m.vp_cancel()}</button>
          <button class="btn primary" type="button" data-tip={keys('Mod+Enter')} onclick={() => finish(editing)}>{m.vp_apply()}</button>
        </div>
      {:else if raw === null}
        <p class="vp-null">NULL</p>
      {:else if kind === 'json' && parsed?.ok}
        {#if jsonView === 'tree'}
          <ValueJson bind:this={tree} value={parsed.value} {query} column={col.name} {dialect} />
        {:else}
          <ValueText text={JSON.stringify(parsed.value, null, 2)} />
        {/if}
      {:else if svgText && svgImage && svgView === 'image' && !broken}
        <figure class="vp-image">
          <img src={svgImage} alt={m.vp_image_alt({ column: col.name })} onload={(e) => (dims = `${(e.currentTarget as HTMLImageElement).naturalWidth} × ${(e.currentTarget as HTMLImageElement).naturalHeight} px`)} onerror={() => (broken = true)} />
        </figure>
      {:else if kind === 'xml'}
        <ValueText text={xml} xml />
      {:else if kind === 'image' && image && !broken}
        <figure class="vp-image">
          <img src={image} alt={m.vp_image_alt({ column: col.name })} onload={(e) => (dims = `${(e.currentTarget as HTMLImageElement).naturalWidth} × ${(e.currentTarget as HTMLImageElement).naturalHeight} px`)} onerror={() => (broken = true)} />
        </figure>
      {:else if kind === 'image' && magic?.ext === 'tif' && !tiff && !broken && raw !== null && byteLength(raw) <= IMAGE_LIMIT}
        <p class="vp-note">{m.vp_tiff_reading()}</p>
      {:else if bin}
        {#if kind === 'image'}<p class="vp-note">{m.vp_image_broken()}</p>{/if}
        <table class="vp-hex">
          <tbody>
            {#each hexDump(raw, 256, 8) as line (line.offset)}
              <tr><th scope="row">{line.offset}</th><td class="vp-hex-b">{line.hex}</td><td class="vp-hex-a">{line.ascii}</td></tr>
            {/each}
          </tbody>
        </table>
        {#if byteLength(raw) > 256}<p class="vp-note">{m.vp_hex_head({ count: num(256), total: sizeText(byteLength(raw), tag()) })}</p>{/if}
      {:else}
        <ValueText text={raw} />
      {/if}
    </div>
    {#if binColumn && lock && editing === null}<p class="vp-foot">{m.vp_bin_edit()}</p>{/if}
  {/if}
  </div>
</aside>
