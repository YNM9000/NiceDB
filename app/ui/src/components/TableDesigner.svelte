<script lang="ts">
  import { EditorState } from '@codemirror/state';
  import { EditorView } from '@codemirror/view';
  import { ArrowDown, ArrowLeft, ArrowUp, KeyRound, Plus, RotateCcw, Trash2, TriangleAlert, X } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import type { FkAction } from '../lib/bindings';
  import { addPart, argKind, canReorder, columnMark, droppedColumns, ENGINE_NAME, FK_SQL, fkActionsOf, flipPart, hasComments, hasIdentity, hasPartial, moveColumn, movePart, newColumn, noteLevel, noteText, removePart, removeColumn, renameColumn, shownRef, takenNames, togglePrimary, typeJoin, typeParts, typesOf, uniqueName, applyHint } from '../lib/design';
  import type { Design, DesignSection } from '../lib/design.svelte';
  import { viewerExtensions } from '../lib/editor';
  import { loadedSchema } from '../lib/explorer.svelte';
  import { num } from '../lib/format';
  import { prefs } from '../lib/prefs.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';
  import { suggest } from '../lib/suggest';

  let { design }: { design: Design } = $props();
  const d = $derived(design.after);
  const e = $derived(design.engine);
  const dialect = $derived(design.session?.info.dialect ?? 'postgres');

  const sections = $derived([
    { value: 'columns', label: `${m.dz_columns()} ${d.columns.length}` },
    { value: 'indexes', label: `${m.dz_indexes()} ${d.indexes.length}` },
    { value: 'foreign_keys', label: `${m.dz_foreign_keys()} ${d.foreign_keys.length}` },
    { value: 'checks', label: `${m.dz_checks()} ${d.checks.length}` },
  ] as const);

  const space = $derived(loadedSchema(design.connectionId, design.database).data?.namespaces.find((n) => n.name === design.namespace) ?? null);
  const tables = $derived([...new Set([d.name, ...(space?.tables.map((t) => t.name) ?? [])])].filter(Boolean));
  const refColumns = (table: string) => (table === d.name ? d.columns.map((c) => c.name) : (space?.tables.find((t) => t.name === table)?.columns.map((c) => c.name) ?? []));
  const dropped = $derived(droppedColumns(design.before, d));

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    JSON.stringify(design.after);
    void design.before;
    clearTimeout(timer);
    timer = setTimeout(() => void design.refreshScript(), 90);
  });
  onDestroy(() => {
    clearTimeout(timer);
    view?.destroy();
  });

  let host = $state<HTMLDivElement>();
  let view: EditorView | null = null;
  $effect(() => {
    const text = design.text;
    if (!host) return;
    if (!view || view.dom.parentElement !== host) {
      view?.destroy();
      view = new EditorView({ parent: host, state: EditorState.create({ doc: text, extensions: [viewerExtensions(dialect, prefs.value.editor), EditorView.lineWrapping] }) });
    } else if (view.state.doc.toString() !== text) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
  });

  function setType(i: number, base: string) {
    const c = d.columns[i];
    if (!c) return;
    const { args } = typeParts(c.type_name);
    c.type_name = typeJoin(base, argKind(e, base) === 'none' ? '' : args);
  }

  function setArgs(i: number, args: string) {
    const c = d.columns[i];
    if (c) c.type_name = typeJoin(typeParts(c.type_name).base, args);
  }

  const opt = (s: string) => (s.trim() ? s : null);

  function addColumn() {
    d.columns.push(newColumn(e, d.columns.map((c) => c.name)));
    queueMicrotask(() => document.querySelector<HTMLInputElement>('.dz-cols .dz-row:last-of-type .dz-col-name')?.select());
  }

  function restore(name: string) {
    const b = design.before?.columns.find((c) => c.name === name);
    if (b) d.columns.push(structuredClone($state.snapshot(b)));
  }

  const EXPR = '\u0000expr';
  let adding = $state<number | null>(null);
  const focusOnMount = (node: HTMLInputElement) => node.focus();

  function exprDone(value: string, ix: (typeof d.indexes)[number]) {
    if (adding === null) return;
    adding = null;
    if (value.trim()) addPart(ix, value.trim(), true);
  }

  function exprKey(ev: KeyboardEvent & { currentTarget: HTMLInputElement }, ix: (typeof d.indexes)[number]) {
    if (ev.key === 'Enter') {
      ev.preventDefault();
      exprDone(ev.currentTarget.value, ix);
    } else if (ev.key === 'Escape') {
      ev.stopPropagation();
      adding = null;
    }
  }

  function addIndex() {
    const first = d.columns.find((c) => !d.primary_key.includes(c.name)) ?? d.columns[0];
    if (!first) return;
    d.indexes.push({ origin: null, name: uniqueName(`${d.name || m.dz_default_table()}_${first.name}_idx`, takenNames(d)), columns: [first.name], unique: false });
  }

  function addForeignKey() {
    const target = tables.find((t) => t !== d.name) ?? d.name;
    const free = d.columns.filter((c) => !d.primary_key.includes(c.name) && !d.foreign_keys.some((f) => f.columns.includes(c.name)));
    const local = free.find((c) => c.name.endsWith('_id')) ?? free[0] ?? d.columns[0];
    if (!local || !target) return;
    d.foreign_keys.push({ origin: null, name: uniqueName(`${d.name || m.dz_default_table()}_${target}_fk`, takenNames(d)), columns: [local.name], ref_namespace: '', ref_table: target, ref_columns: [refColumns(target)[0] ?? 'id'], on_delete: 'no_action', on_update: 'no_action' });
  }

  function addCheck() {
    d.checks.push({ origin: null, name: uniqueName(`${d.name || m.dz_default_table()}_check`, takenNames(d)), expression: '' });
  }

  const ACTION: Record<FkAction, string> = FK_SQL;
</script>


<div class="designer" class:no-comments={!hasComments(e)}>
  <div class="dz-main">
    {#if design.failure}
      <p class="tool-note err" role="alert">{m.dz_load_failed({ reason: design.failure })}</p>
    {:else if design.loading}
      <p class="hint dz-wait" role="status"><span class="conn-state busy" aria-hidden="true"></span> {m.dz_loading()}</p>
    {:else}
      <header class="dz-head">
        <label class="dz-field dz-table">
          <span class="caps">{design.origin ? m.dz_table() : m.dz_new_table_name()}</span>
          <input class="mono" value={d.name} placeholder={m.dz_name_placeholder()} spellcheck="false" oninput={(ev) => (d.name = ev.currentTarget.value.trim())} />
        </label>
        {#if hasComments(e)}
          <label class="dz-field dz-comment">
            <span class="caps">{m.dz_comment()}</span>
            <input value={d.comment ?? ''} oninput={(ev) => (d.comment = opt(ev.currentTarget.value))} />
          </label>
        {/if}
        <p class="dz-where hint">{design.namespace} · {design.database} · {ENGINE_NAME[e]}</p>
      </header>

      <Segmented class="dz-sections" label={m.dz_sections()} value={design.section} options={sections} onchange={(v: DesignSection) => (design.section = v)} />

      <div class="dz-body">
        {#if design.section === 'columns'}
          <div class="dz-grid dz-cols" role="table" aria-label={m.dz_columns()}>
            <div class="dz-row dz-th" role="row">
              <span role="columnheader" aria-label={m.dz_order()}></span>
              <span role="columnheader" data-tip={m.dz_pk_long()}>{m.dz_pk()}</span>
              <span role="columnheader">{m.dz_name()}</span>
              <span role="columnheader">{m.dz_type()}</span>
              <span role="columnheader">{m.dz_length()}</span>
              <span role="columnheader" data-tip={m.dz_null_long()}>{m.dz_null()}</span>
              <span role="columnheader">{m.dz_default()}</span>
              <span role="columnheader" data-tip={m.dz_identity_long()}>{m.dz_identity()}</span>
              {#if hasComments(e)}<span role="columnheader">{m.dz_comment()}</span>{/if}
              <span role="columnheader" aria-label={m.dz_remove()}></span>
            </div>
            {#each d.columns as c, i (i)}
              {@const mark = columnMark(design.before, c)}
              {@const parts = typeParts(c.type_name)}
              {@const kind = argKind(e, parts.base)}
              {@const pk = d.primary_key.includes(c.name)}
              <div class="dz-row {mark ?? ''}" role="row">
                <span class="dz-order" role="cell">
                  <i class="dz-mark" data-tip={mark === 'new' ? m.dz_mark_new() : mark === 'changed' ? m.dz_mark_changed() : ''}></i>
                  <button class="icon-btn" type="button" aria-label={m.dz_move_up({ name: c.name })} disabled={i === 0} onclick={() => moveColumn(d, i, -1)}><ArrowUp size={12} strokeWidth={1.8} /></button>
                  <button class="icon-btn" type="button" aria-label={m.dz_move_down({ name: c.name })} disabled={i === d.columns.length - 1} onclick={() => moveColumn(d, i, 1)}><ArrowDown size={12} strokeWidth={1.8} /></button>
                </span>
                <span role="cell" class="dz-center">
                  <button class="icon-btn dz-key" class:on={pk} type="button" aria-pressed={pk} aria-label={m.dz_pk_toggle({ name: c.name })} data-tip={m.dz_pk_long()} onclick={() => togglePrimary(d, c.name)}><KeyRound size={13} strokeWidth={1.8} /></button>
                </span>
                <span role="cell"><input class="dz-in mono dz-col-name" value={c.name} spellcheck="false" aria-label={m.dz_cell({ name: c.name, what: m.dz_name() })} onchange={(ev) => renameColumn(d, i, ev.currentTarget.value.trim())} /></span>
                <span role="cell"><input class="dz-in mono" use:suggest={{ options: typesOf(e), label: m.dz_type() }} value={parts.base} spellcheck="false" aria-label={m.dz_cell({ name: c.name, what: m.dz_type() })} onchange={(ev) => setType(i, ev.currentTarget.value)} /></span>
                <span role="cell"><input class="dz-in mono" value={parts.args} disabled={kind === 'none' && !parts.args} aria-label={m.dz_cell({ name: c.name, what: m.dz_length() })} onchange={(ev) => setArgs(i, ev.currentTarget.value)} /></span>
                <span role="cell" class="dz-center"><input type="checkbox" checked={c.nullable} disabled={pk} aria-label={m.dz_cell({ name: c.name, what: m.dz_null_long() })} onchange={(ev) => (c.nullable = ev.currentTarget.checked)} /></span>
                <span role="cell"><input class="dz-in mono" value={c.default ?? ''} disabled={c.identity && e !== 'duckdb'} spellcheck="false" aria-label={m.dz_cell({ name: c.name, what: m.dz_default() })} onchange={(ev) => (c.default = opt(ev.currentTarget.value))} /></span>
                <span role="cell" class="dz-center"><input type="checkbox" checked={c.identity} disabled={!hasIdentity(e)} data-tip={hasIdentity(e) ? m.dz_identity_long() : m.dz_identity_none({ engine: ENGINE_NAME[e] })} aria-label={m.dz_cell({ name: c.name, what: m.dz_identity_long() })} onchange={(ev) => (c.identity = ev.currentTarget.checked)} /></span>
                {#if hasComments(e)}<span role="cell"><input class="dz-in" value={c.comment ?? ''} aria-label={m.dz_cell({ name: c.name, what: m.dz_comment() })} onchange={(ev) => (c.comment = opt(ev.currentTarget.value))} /></span>{/if}
                <span role="cell" class="dz-center"><button class="icon-btn dz-del" type="button" aria-label={m.dz_remove_column({ name: c.name })} onclick={() => removeColumn(d, i)}><Trash2 size={13} strokeWidth={1.8} /></button></span>
              </div>
            {/each}
          </div>
          <div class="dz-foot">
            <button class="btn ghost" type="button" onclick={addColumn}><Plus size={13} strokeWidth={1.8} aria-hidden="true" /> {m.dz_add_column()}</button>
            {#if !canReorder(e) && design.origin}<span class="hint">{m.dz_reorder_hint({ engine: ENGINE_NAME[e] })}</span>{/if}
          </div>
          {#if dropped.length}
            <div class="dz-dropped" role="group" aria-label={m.dz_dropped()}>
              <span class="caps">{m.dz_dropped()}</span>
              {#each dropped as c (c.name)}
                <button class="dz-chip gone" type="button" data-tip={m.dz_restore()} onclick={() => restore(c.name)}><span class="mono">{c.name}</span> <RotateCcw size={11} strokeWidth={1.8} aria-label={m.dz_restore()} /></button>
              {/each}
            </div>
          {/if}
        {:else if design.section === 'indexes'}
          <div class="dz-grid dz-idx" role="table" aria-label={m.dz_indexes()}>
            <div class="dz-row dz-th" role="row">
              <span role="columnheader">{m.dz_name()}</span>
              <span role="columnheader">{m.dz_index_columns()}</span>
              <span role="columnheader">{m.dz_index_where()}</span>
              <span role="columnheader">{m.dz_unique()}</span>
              <span role="columnheader" aria-label={m.dz_remove()}></span>
            </div>
            {#each d.indexes as ix, i (i)}
              <div class="dz-row" class:new={!ix.origin} role="row">
                <span role="cell"><input class="dz-in mono" value={ix.name} spellcheck="false" aria-label={m.dz_cell({ name: ix.name, what: m.dz_name() })} onchange={(ev) => (ix.name = ev.currentTarget.value.trim())} /></span>
                <span role="cell" class="dz-chips">
                  {#each ix.columns as col, k (`${k}:${col}`)}
                    {@const label = shownRef(col)}
                    <span class="dz-chip" class:expr={ix.expressions?.[k]}>
                      {#if k > 0}<button class="icon-btn" type="button" aria-label={m.dz_move_up({ name: label })} onclick={() => movePart(ix, k, -1)}><ArrowLeft size={11} strokeWidth={1.8} /></button>{/if}
                      <span class="mono dz-part" data-tip={ix.expressions?.[k] ? m.dz_part_expr() : undefined}>{label}</span>
                      <button class="dz-dir" type="button" aria-pressed={!!ix.descending?.[k]} aria-label={m.dz_part_order({ name: label })} onclick={() => flipPart(ix, k)}>{ix.descending?.[k] ? 'DESC' : 'ASC'}</button>
                      <button class="icon-btn" type="button" aria-label={m.dz_remove_from({ name: label })} disabled={ix.columns.length === 1} onclick={() => removePart(ix, k)}><X size={11} strokeWidth={1.8} /></button>
                    </span>
                  {/each}
                  {#if adding === i}
                    <input class="dz-in mono dz-expr-in" placeholder={m.dz_ph_expr()} spellcheck="false" aria-label={m.dz_add_expr_label({ name: ix.name })} use:focusOnMount onkeydown={(ev) => exprKey(ev, ix)} onblur={(ev) => exprDone(ev.currentTarget.value, ix)} />
                  {:else}
                    <Select class="tool-select dz-add-col" label={m.dz_add_to_index({ name: ix.name })} value="" keep={false} placeholder={m.dz_add_col_short()} onchange={(v) => (v === EXPR ? (adding = i) : v && addPart(ix, v))} options={[...d.columns.filter((c) => !ix.columns.includes(c.name)).map((c) => ({ value: c.name, label: c.name })), { value: EXPR, label: m.dz_add_expr() }]} />
                  {/if}
                </span>
                <span role="cell">
                  {#if hasPartial(e)}
                    <input class="dz-in mono" value={ix.predicate ?? ''} placeholder={m.dz_index_where_hint()} spellcheck="false" aria-label={m.dz_index_where_cell({ name: ix.name })} onchange={(ev) => (ix.predicate = ev.currentTarget.value.trim() || null)} />
                  {:else}
                    <span class="dz-off" data-tip={m.dz_index_where_none({ engine: ENGINE_NAME[e] })}>—</span>
                  {/if}
                </span>
                <span role="cell" class="dz-center"><input type="checkbox" checked={ix.unique} aria-label={m.dz_cell({ name: ix.name, what: m.dz_unique() })} onchange={(ev) => (ix.unique = ev.currentTarget.checked)} /></span>
                <span role="cell" class="dz-center"><button class="icon-btn dz-del" type="button" aria-label={m.dz_remove_named({ name: ix.name })} onclick={() => d.indexes.splice(i, 1)}><Trash2 size={13} strokeWidth={1.8} /></button></span>
              </div>
            {:else}
              <p class="dz-none">{m.dz_no_indexes()}</p>
            {/each}
          </div>
          <div class="dz-foot">
            <button class="btn ghost" type="button" onclick={addIndex} disabled={!d.columns.length}><Plus size={13} strokeWidth={1.8} aria-hidden="true" /> {m.dz_add_index()}</button>
            <span class="hint">{m.dz_pk_where()}</span>
          </div>
        {:else if design.section === 'foreign_keys'}
          <div class="dz-fks">
            {#each d.foreign_keys as f, i (i)}
              <fieldset class="dz-fk" class:new={!f.origin}>
                <legend class="dz-hidden">{f.name}</legend>
                <div class="dz-fk-head">
                  <input class="dz-in mono dz-fk-name" value={f.name} spellcheck="false" aria-label={m.dz_cell({ name: f.name, what: m.dz_name() })} onchange={(ev) => (f.name = ev.currentTarget.value.trim())} />
                  <span class="dz-arrow" aria-hidden="true">→</span>
                  <Select class="tool-select mono" label={m.dz_ref_table()} value={f.ref_table} onchange={(v) => { f.ref_table = v; f.ref_columns = f.columns.map((_, k) => refColumns(f.ref_table)[k] ?? ''); }} options={tables.map((t) => ({ value: t, label: t }))} />
                  <span class="spacer"></span>
                  <button class="icon-btn dz-del" type="button" aria-label={m.dz_remove_named({ name: f.name })} onclick={() => d.foreign_keys.splice(i, 1)}><Trash2 size={13} strokeWidth={1.8} /></button>
                </div>
                {#each f.columns as col, k (k)}
                  <div class="dz-pair">
                    <Select class="tool-select mono" label={m.dz_fk_local({ n: k + 1 })} value={col} onchange={(v) => (f.columns[k] = v)} options={d.columns.map((c) => ({ value: c.name, label: c.name }))} />
                    <span class="dz-arrow" aria-hidden="true">→</span>
                    <Select class="tool-select mono" label={m.dz_fk_ref({ n: k + 1 })} value={f.ref_columns[k] ?? ''} onchange={(v) => (f.ref_columns[k] = v)} options={refColumns(f.ref_table).map((c) => ({ value: c, label: c }))} />
                    {#if f.columns.length > 1}<button class="icon-btn" type="button" aria-label={m.dz_remove_pair()} onclick={() => (f.columns.splice(k, 1), f.ref_columns.splice(k, 1))}><X size={12} strokeWidth={1.8} /></button>{/if}
                  </div>
                {/each}
                <div class="dz-fk-foot">
                  <button class="btn ghost small" type="button" onclick={() => (f.columns.push(d.columns[0]?.name ?? ''), f.ref_columns.push(refColumns(f.ref_table)[f.columns.length - 1] ?? ''))}><Plus size={12} strokeWidth={1.8} aria-hidden="true" /> {m.dz_add_pair()}</button>
                  <span class="spacer"></span>
                  {#each [{ label: m.dz_on_delete(), key: 'on_delete' as const }, { label: m.dz_on_update(), key: 'on_update' as const }] as a (a.key)}
                    <label class="dz-action">
                      <span class="caps">{a.label}</span>
                      <Select class="tool-select" label={a.label} value={f[a.key]} onchange={(v) => (f[a.key] = v)} options={fkActionsOf(e).map((x) => ({ value: x, label: ACTION[x] }))} />
                    </label>
                  {/each}
                </div>
              </fieldset>
            {:else}
              <p class="dz-none">{m.dz_no_foreign_keys()}</p>
            {/each}
          </div>
          <div class="dz-foot">
            <button class="btn ghost" type="button" onclick={addForeignKey} disabled={!d.columns.length}><Plus size={13} strokeWidth={1.8} aria-hidden="true" /> {m.dz_add_foreign_key()}</button>
          </div>
        {:else}
          <div class="dz-grid dz-chk" role="table" aria-label={m.dz_checks()}>
            <div class="dz-row dz-th" role="row">
              <span role="columnheader">{m.dz_name()}</span>
              <span role="columnheader">{m.dz_condition()}</span>
              <span role="columnheader" aria-label={m.dz_remove()}></span>
            </div>
            {#each d.checks as c, i (i)}
              <div class="dz-row" class:new={!c.origin} role="row">
                <span role="cell"><input class="dz-in mono" value={c.name} spellcheck="false" aria-label={m.dz_cell({ name: c.name, what: m.dz_name() })} onchange={(ev) => (c.name = ev.currentTarget.value.trim())} /></span>
                <span role="cell"><input class="dz-in mono" value={c.expression} spellcheck="false" placeholder={m.dz_check_hint()} aria-label={m.dz_cell({ name: c.name, what: m.dz_condition() })} oninput={(ev) => (c.expression = ev.currentTarget.value)} /></span>
                <span role="cell" class="dz-center"><button class="icon-btn dz-del" type="button" aria-label={m.dz_remove_named({ name: c.name })} onclick={() => d.checks.splice(i, 1)}><Trash2 size={13} strokeWidth={1.8} /></button></span>
              </div>
            {:else}
              <p class="dz-none">{m.dz_no_checks()}</p>
            {/each}
          </div>
          <div class="dz-foot">
            <button class="btn ghost" type="button" onclick={addCheck}><Plus size={13} strokeWidth={1.8} aria-hidden="true" /> {m.dz_add_check()}</button>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <aside class="dz-script" aria-label={m.dz_script()}>
    <header class="dz-script-head">
      <h3 class="caps">{m.dz_script()}</h3>
      <span class="hint">{design.script.statements.length === 1 ? m.dz_statement_one({ engine: ENGINE_NAME[e] }) : design.script.statements.length ? m.dz_statements({ count: num(design.script.statements.length), engine: ENGINE_NAME[e] }) : ENGINE_NAME[e]}</span>
    </header>
    {#if design.trial}
      <div class="dz-trial" role="status">
        <p>{m.dz_trial()}</p>
        <div class="dz-trial-btns">
          <button class="btn ghost" type="button" onclick={() => void design.cancelTrial()}>{m.dz_trial_cancel()}</button>
          <button class="btn primary" type="button" onclick={() => void design.confirmTrial()}>{m.dz_trial_confirm()}</button>
        </div>
      </div>
    {/if}
    {#if design.error}<p class="tool-note err dz-error" role="alert">{design.error}</p>{/if}
    {#if design.script.notes.length}
      <ul class="dz-notes">
        {#each design.script.notes as n, i (i)}
          {@const level = noteLevel(n)}
          <li class={level}>{#if level !== 'info'}<TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" />{/if}<span>{noteText(n, e)}</span></li>
        {/each}
      </ul>
    {/if}
    {#if design.script.statements.length}
      <div class="dz-code" bind:this={host} role="region" aria-label={m.dz_script_code()}></div>
    {:else if !design.loading && !design.failure}
      <p class="dz-empty">{design.script.notes.some((n) => n.code === 'trigger_blocks') ? m.dz_blocked_trigger() : design.blocked ? m.dz_blocked() : m.dz_nothing()}</p>
    {/if}
    <p class="hint dz-how">{applyHint(e)}</p>
  </aside>
</div>
