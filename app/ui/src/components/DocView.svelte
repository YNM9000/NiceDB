<script lang="ts">
  import { ClipboardCopy, CopyPlus, FilePlus2, PenLine, Search, Trash2 } from '@lucide/svelte';
  import { onMount, tick } from 'svelte';
  import { EditorView } from '@codemirror/view';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { summaryParts } from '../lib/ejson';
  import { docEdit, writeDocuments } from '../lib/docs.svelte';
  import { docSource, freshCopy, keyLabel, keyOf } from '../lib/doc-source';
  import { fieldExtensions } from '../lib/editor';
  import { num } from '../lib/format';
  import { checkJson, uncomment } from '../lib/mql';
  import { showToast } from '../lib/toast.svelte';
  import { databaseOf, sessionOf, type ResultSet, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import ValueJson from './ValueJson.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import type { DocumentWrite } from '../lib/bindings';

  let { result, tab }: { result: ResultSet; tab: Tab } = $props();

  let ROW = $state(26);
  const OVERSCAN = 12;

  const session = $derived(sessionOf(tab));
  const source = $derived(session ? docSource(session.info.caps, result.sql, session.schema) : null);
  const collection = $derived(source?.collection ?? null);
  const editable = $derived(!!session?.info.caps.editable && !!collection);
  const home = (d: unknown) => (d && typeof d === 'object' && typeof (d as { _index?: unknown })._index === 'string' ? (d as { _index: string })._index : collection);
  const docs = $derived((void result.rev, void result.count, result.docs));

  let selected = $state(0);
  let query = $state('');
  let list = $state<HTMLElement>();
  let scrollTop = $state(0);
  let height = $state(400);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(result.count, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  const shown = $derived(Array.from({ length: Math.max(0, last - first) }, (_, k) => first + k));
  const doc = $derived((void result.rev, docs[Math.min(selected, Math.max(0, result.count - 1))]));
  const id = $derived(doc === undefined ? null : keyOf(doc, source));

  onMount(() => {
    if (!list) return;
    ROW = parseFloat(getComputedStyle(list).getPropertyValue('--nd-doc-row')) || ROW;
    const watch = new ResizeObserver(() => (height = list?.clientHeight ?? height));
    watch.observe(list);
    return () => watch.disconnect();
  });

  async function go(i: number) {
    selected = Math.max(0, Math.min(result.count - 1, i));
    await tick();
    const top = selected * ROW;
    if (list && top < list.scrollTop) list.scrollTop = top;
    else if (list && top + ROW > list.scrollTop + list.clientHeight) list.scrollTop = top + ROW - list.clientHeight;
    list?.querySelector<HTMLElement>(`[data-doc="${selected}"]`)?.focus({ preventScroll: true });
  }

  function onkeydown(e: KeyboardEvent) {
    const page = Math.max(1, Math.floor(height / ROW) - 1);
    const to = { ArrowDown: selected + 1, ArrowUp: selected - 1, PageDown: selected + page, PageUp: selected - page, Home: 0, End: result.count - 1 }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    void go(to);
  }

  const pretty = (v: unknown) => JSON.stringify(v, null, 2);

  function edit(index: number | null, copy = false) {
    const base = index === null ? null : docs[index];
    if (index !== null && base === undefined) return;
    if (base === null || base === undefined || copy) {
      const fresh = base && typeof base === 'object' ? freshCopy(base as Record<string, unknown>, source) : {};
      docEdit.open = { title: m.doc_new_title({ collection: collection ?? '' }), text: pretty(fresh), id: null, index: null };
    } else {
      docEdit.open = { title: m.doc_edit_title({ id: keyLabel(base, source) ?? '' }), text: pretty(base), id: keyOf(base, source), index };
    }
    docEdit.error = null;
  }

  $effect(() => registerCommands([{ id: 'docs.new', label: m.doc_new, group: m.pal_group_query, when: () => editable && !docEdit.open, run: () => edit(null) }]));

  async function remove(index: number) {
    const target = docs[index];
    const key = keyOf(target, source);
    const where = home(target);
    if (!session || !where || !key) return;
    const done = await writeDocuments(session.info, { kind: 'delete', database: databaseOf(tab) ?? session.database, collection: where, ids: [key] }, [m.doc_confirm_delete({ id: keyLabel(target, source) ?? key })]);
    if (!done) return;
    if (!done.ok) return showToast(done.message, null, 'error');
    result.replaceDocument(index, null);
    selected = Math.min(selected, result.count - 1);
    showToast(m.doc_deleted());
  }

  function menu(e: MouseEvent, index: number) {
    e.preventDefault();
    selected = index;
    const target = docs[index];
    openMenu(e, [
      { label: m.doc_copy_json(), icon: ClipboardCopy, run: () => copyText(pretty(target)) },
      { label: m.doc_copy_id(), icon: ClipboardCopy, disabled: !keyOf(target, source), run: () => copyText(keyOf(target, source) ?? '') },
      editable && 'separator',
      editable && { label: m.doc_edit(), icon: PenLine, run: () => edit(index) },
      editable && { label: m.doc_duplicate(), icon: CopyPlus, run: () => edit(index, true) },
      editable && { label: m.doc_delete(), icon: Trash2, danger: true, run: () => void remove(index) },
    ]);
  }

  let host = $state<HTMLDivElement>();
  let view: EditorView | null = null;

  $effect(() => {
    const open = docEdit.open;
    if (!open || !host) return;
    view = new EditorView({ parent: host, doc: open.text, extensions: [fieldExtensions(null, open.title, '{ }', (t) => (docEdit.open && (docEdit.open.text = t))), EditorView.lineWrapping] });
    view.focus();
    return () => {
      view?.destroy();
      view = null;
    };
  });

  async function save() {
    const open = docEdit.open;
    if (!open || !session || !collection) return;
    const clean = uncomment(open.text);
    const bad = checkJson(clean);
    if (bad) {
      docEdit.error = m.doc_not_json({ problem: bad.message === 'json_not_object' ? m.doc_not_object() : m[bad.message]() });
      return;
    }
    const database = databaseOf(tab) ?? session.database;
    const where = open.index === null ? collection : (home(docs[open.index]) ?? collection);
    const write: DocumentWrite = open.id === null ? { kind: 'insert', database, collection: where, documents: [clean] } : { kind: 'replace', database, collection: where, id: open.id, document: clean };
    docEdit.busy = true;
    const done = await writeDocuments(session.info, write, [open.id === null ? m.doc_confirm_insert() : m.doc_confirm_replace({ id: open.id })]);
    docEdit.busy = false;
    if (!done) return;
    if (!done.ok) {
      docEdit.error = done.message;
      return;
    }
    const parsed = JSON.parse(clean) as Record<string, unknown>;
    if (open.index !== null) {
      const was = docs[open.index] as Record<string, unknown> | undefined;
      const kept = source?.keys ? Object.fromEntries(source.keys.map((k) => [k, was?.[k]])) : { _id: was?._id, ...(was && '_index' in was ? { _index: was._index } : {}) };
      result.replaceDocument(open.index, { ...parsed, ...kept });
    }
    docEdit.open = null;
    showToast(open.id === null ? m.doc_inserted() : m.doc_saved());
  }
</script>

<div class="docs" class:editing={!!docEdit.open}>
  <div class="docs-list-wrap">
    <div class="docs-head">
      <span class="caps">{m.doc_count({ count: num(result.count) })}</span>
      <span class="spacer"></span>
      {#if editable}
        <button class="btn ghost" type="button" onclick={() => edit(null)}><FilePlus2 size={13} strokeWidth={1.8} aria-hidden="true" />{m.doc_new()}</button>
      {/if}
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="docs-list" role="listbox" tabindex="-1" aria-label={m.doc_list()} bind:this={list} onscroll={() => (scrollTop = list?.scrollTop ?? 0)} {onkeydown}>
      <div class="docs-spacer" style:height="{result.count * ROW}px">
        {#each shown as i (i)}
          {@const d = (void result.rev, docs[i])}
          <div
            class="docs-row"
            role="option"
            aria-selected={i === selected}
            tabindex={i === selected ? 0 : -1}
            data-doc={i}
            style:top="{i * ROW}px"
            onclick={() => (selected = i)}
            onkeydown={(e) => e.key === 'Enter' && editable && edit(i)}
            ondblclick={() => editable && edit(i)}
            oncontextmenu={(e) => menu(e, i)}
          >
            <span class="docs-id">{keyLabel(d, source) ?? num(i + 1)}</span>
            <span class="docs-sum">{#each summaryParts(d, 4, source?.keys ?? ['_id', '_index']) as [k, v], j (j)}{#if j}<span class="docs-sep" aria-hidden="true">·</span>{/if}<span class="docs-k">{k}</span> {v}{/each}</span>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <div class="docs-detail">
    {#if docEdit.open}
      <div class="docs-head">
        <span class="docs-title">{docEdit.open.title}</span>
        <span class="spacer"></span>
        <button class="btn ghost" type="button" disabled={docEdit.busy} onclick={() => (docEdit.open = null)}>{m.form_cancel()}</button>
        <button class="btn primary" type="button" disabled={docEdit.busy} onclick={() => void save()}>{docEdit.open.id === null ? m.doc_insert() : m.doc_save()}</button>
      </div>
      {#if docEdit.error}<p class="docs-error" role="alert">{docEdit.error}</p>{/if}
      <div class="docs-editor" bind:this={host}></div>
    {:else if doc !== undefined}
      <div class="docs-head">
        <span class="docs-title" data-tip={id ?? ''}>{#if id}<span class="docs-k">{source?.keys ? source.keys.join(' · ') : '_id'}</span> {keyLabel(doc, source) ?? id}{:else}{m.doc_no_id()}{/if}</span>
        <span class="spacer"></span>
        <label class="field docs-find"><Search size={13} strokeWidth={1.8} aria-hidden="true" /><input bind:value={query} placeholder={m.vp_json_search()} aria-label={m.vp_json_search()} spellcheck="false" /></label>
        <button class="icon-btn" type="button" data-tip={m.doc_copy_json()} aria-label={m.doc_copy_json()} onclick={() => void copyText(pretty(doc))}><ClipboardCopy size={14} strokeWidth={1.8} /></button>
        {#if editable}
          <button class="icon-btn" type="button" data-tip={m.doc_edit()} aria-label={m.doc_edit()} onclick={() => edit(selected)}><PenLine size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn" type="button" data-tip={m.doc_duplicate()} aria-label={m.doc_duplicate()} onclick={() => edit(selected, true)}><CopyPlus size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn danger" type="button" data-tip={m.doc_delete()} aria-label={m.doc_delete()} onclick={() => void remove(selected)}><Trash2 size={14} strokeWidth={1.8} /></button>
        {/if}
      </div>
      {#key selected}
        <ValueJson value={doc} {query} column="" dialect={null} bson />
      {/key}
    {/if}
  </div>
</div>
