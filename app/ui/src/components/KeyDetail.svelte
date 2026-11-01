<script lang="ts">
  import { Check, ClipboardCopy, KeyRound, PenLine, Plus, RefreshCw, Timer, Trash2, X } from '@lucide/svelte';
  import type { KeyEntry, KeyWrite_Deserialize as KeyWrite } from '../lib/bindings';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { num } from '../lib/format';
  import { asJson, parseTtl } from '../lib/keys';
  import { writeKey, type KeyBrowserState } from '../lib/keys.svelte';
  import { showToast } from '../lib/toast.svelte';
  import { bytes, duration } from '../lib/tools';
  import type { sessionOf } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import { tick } from 'svelte';
  import Segmented from './Segmented.svelte';
  import ValueJson from './ValueJson.svelte';

  let { st, session }: { st: KeyBrowserState; session: NonNullable<ReturnType<typeof sessionOf>> } = $props();

  const sid = $derived(session.info.session_id);
  const writable = $derived(session.info.caps.editable);
  const v = $derived(st.value);
  const key = $derived(st.selected);
  const kind = $derived(v?.kind ?? '');
  const entries = $derived(v?.data.shape === 'entries' ? v.data.entries : []);
  const text = $derived(v?.data.shape === 'text' ? v.data : null);
  const size = $derived(st.keys.find((k) => k.key === key)?.bytes ?? null);

  let renaming = $state<string | null>(null);
  let ttlEdit = $state<string | null>(null);
  let confirmDelete = $state(false);
  let draft = $state<string | null>(null);
  let mode = $state<'text' | 'json'>('text');
  let picked = $state<number | null>(null);
  let editing = $state<{ index: number; text: string } | null>(null);
  let addName = $state('');
  let addValue = $state('');
  let busy = $state(false);

  $effect(() => {
    void key;
    renaming = null;
    ttlEdit = null;
    confirmDelete = false;
    draft = null;
    picked = null;
    editing = null;
    addName = '';
    addValue = '';
  });

  const parsed = $derived(text ? asJson(draft ?? text.text) : undefined);
  $effect(() => {
    if (kind === 'ReJSON-RL' || (parsed !== undefined && draft === null)) mode = 'json';
    else if (parsed === undefined) mode = 'text';
  });

  const LENGTH: Record<string, (n: number) => string> = {
    string: (n) => m.kv_len_chars({ count: num(n) }),
    hash: (n) => m.kv_len_fields({ count: num(n) }),
    list: (n) => m.kv_len_items({ count: num(n) }),
    set: (n) => m.kv_len_members({ count: num(n) }),
    zset: (n) => m.kv_len_members({ count: num(n) }),
    stream: (n) => m.kv_len_entries({ count: num(n) }),
  };

  const COLUMNS: Record<string, [string, string | null]> = {
    hash: [m.kv_col_field(), m.kv_col_value()],
    list: ['#', m.kv_col_value()],
    set: [m.kv_col_member(), null],
    zset: [m.kv_col_member(), m.kv_col_score()],
    stream: [m.kv_col_id(), m.kv_col_fields()],
  };

  async function send(write: KeyWrite, toast: string, reload = true): Promise<boolean> {
    busy = true;
    const done = await writeKey(session.info, write);
    busy = false;
    if (!done) return false;
    if (!done.ok) {
      showToast(done.message, null, 'error');
      return false;
    }
    st.applied(write);
    showToast(toast);
    if (reload && write.op !== 'delete') await st.readValue(sid);
    return true;
  }

  async function rename() {
    const to = renaming?.trim();
    if (!key || !to || to === key) return void (renaming = null);
    if (await send({ op: 'rename', key, to }, m.kv_renamed({ key: to }))) renaming = null;
  }

  async function saveTtl(clear = false) {
    if (!key) return;
    const ms = clear ? null : parseTtl(ttlEdit ?? '');
    if (!clear && ms === null) return showToast(m.kv_ttl_bad(), null, 'error');
    if (await send({ op: 'expire', key, ttl_ms: ms }, clear ? m.kv_ttl_cleared() : m.kv_ttl_set({ ttl: duration(ms) }))) ttlEdit = null;
  }

  async function remove() {
    if (!key) return;
    if (!confirmDelete && session.info.environment !== 'production') {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 4000);
      return;
    }
    confirmDelete = false;
    await send({ op: 'delete', keys: [key] }, m.kv_deleted({ key }));
  }

  async function saveText() {
    if (!key || draft === null) return;
    if (kind === 'ReJSON-RL' && asJson(draft) === undefined && !/^\s*("|-?\d|true|false|null)/.test(draft)) return showToast(m.kv_json_bad(), null, 'error');
    const write: KeyWrite = kind === 'ReJSON-RL' ? { op: 'json_set', key, value: draft } : { op: 'set_string', key, value: draft };
    if (await send(write, m.kv_saved())) draft = null;
  }

  function entryWrite(e: KeyEntry, index: number, value: string): KeyWrite | null {
    if (!key) return null;
    if (kind === 'hash') return { op: 'hash_set', key, field: e.name, value };
    if (kind === 'list') return { op: 'list_set', key, index: Number(e.name) || index, value };
    if (kind === 'zset') {
      const score = Number(value.replace(',', '.'));
      return Number.isFinite(score) ? { op: 'zset_add', key, member: e.name, score } : null;
    }
    return null;
  }

  async function saveEntry() {
    const ed = editing;
    const e = ed ? entries[ed.index] : undefined;
    if (!ed || !e) return;
    const w = entryWrite(e, ed.index, ed.text);
    if (!w) return showToast(m.kv_score_bad(), null, 'error');
    if (await send(w, m.kv_saved())) editing = null;
  }

  function removeWrite(e: KeyEntry): KeyWrite | null {
    if (!key) return null;
    if (kind === 'hash') return { op: 'hash_delete', key, fields: [e.name] };
    if (kind === 'list') return { op: 'list_remove', key, index: Number(e.name), value: e.value };
    if (kind === 'set') return { op: 'set_remove', key, members: [e.name] };
    if (kind === 'zset') return { op: 'zset_remove', key, members: [e.name] };
    if (kind === 'stream') return { op: 'stream_delete', key, ids: [e.name] };
    return null;
  }

  async function removeEntry(e: KeyEntry) {
    const w = removeWrite(e);
    if (w && (await send(w, m.kv_item_removed()))) picked = null;
  }

  function fieldsOf(text: string): KeyEntry[] {
    return text
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => {
        const at = l.search(/[\s=]/);
        return at < 0 ? { name: l, value: '' } : { name: l.slice(0, at), value: l.slice(at + 1).trim() };
      });
  }

  async function add(head = false) {
    if (!key) return;
    let w: KeyWrite | null = null;
    if (kind === 'hash' && addName) w = { op: 'hash_set', key, field: addName, value: addValue };
    else if (kind === 'list') w = { op: 'list_push', key, values: [addValue], head };
    else if (kind === 'set' && addName) w = { op: 'set_add', key, members: [addName] };
    else if (kind === 'zset' && addName) {
      const score = Number(addValue.replace(',', '.') || '0');
      if (!Number.isFinite(score)) return showToast(m.kv_score_bad(), null, 'error');
      w = { op: 'zset_add', key, member: addName, score };
    } else if (kind === 'stream') {
      const fields = fieldsOf(addValue);
      if (fields.length) w = { op: 'stream_add', key, fields };
    }
    if (!w) return;
    if (await send(w, m.kv_item_added())) {
      addName = '';
      addValue = '';
    }
  }

  function entryMenu(ev: MouseEvent, e: KeyEntry, i: number) {
    ev.preventDefault();
    entryMenuAt(ev, e, i);
  }

  function entryMenuAt(at: MouseEvent | { x: number; y: number }, e: KeyEntry, i: number) {
    picked = i;
    const editable = writable && ['hash', 'list', 'zset'].includes(kind);
    openMenu(at, [
      { label: m.kv_copy_value(), icon: ClipboardCopy, run: () => copyText(kind === 'set' ? e.name : e.value) },
      kind !== 'list' && { label: m.kv_copy_name(), icon: ClipboardCopy, hint: e.name, run: () => copyText(e.name) },
      editable && 'separator',
      editable && { label: m.kv_edit(), icon: PenLine, run: () => (editing = { index: i, text: e.value }) },
      writable && { label: m.kv_remove_item(), icon: Trash2, danger: true, run: () => void removeEntry(e) },
    ]);
  }

  const uid = $props.id();
  let table = $state<HTMLTableElement>();

  function entryKey(ev: KeyboardEvent) {
    if (editing || !entries.length) return;
    const last = entries.length - 1;
    const at = picked ?? -1;
    const go = (i: number) => {
      ev.preventDefault();
      picked = Math.max(0, Math.min(last, i));
      table?.querySelector(`#${CSS.escape(`${uid}-r${picked}`)}`)?.scrollIntoView({ block: 'nearest' });
    };
    if (ev.key === 'ArrowDown') go(at + 1);
    else if (ev.key === 'ArrowUp') go(at < 0 ? last : at - 1);
    else if (ev.key === 'Home') go(0);
    else if (ev.key === 'End') go(last);
    else if (ev.key === 'PageDown') go(at + 10);
    else if (ev.key === 'PageUp') go(at - 10);
    else {
      const i = picked;
      const e = i === null ? undefined : entries[i];
      if (i === null || !e) return;
      const row = table?.querySelector(`#${CSS.escape(`${uid}-r${i}`)}`)?.getBoundingClientRect();
      if (ev.key === 'Enter' && writable && ['hash', 'list', 'zset'].includes(kind)) {
        ev.preventDefault();
        editing = { index: i, text: e.value };
        void tick().then(() => table?.querySelector('textarea')?.focus());
      } else if (ev.key === 'Delete' && writable) {
        ev.preventDefault();
        void removeEntry(e);
      } else if ((ev.key === 'ContextMenu' || (ev.key === 'F10' && ev.shiftKey)) && row) {
        ev.preventDefault();
        entryMenuAt({ x: row.left + 24, y: row.bottom }, e, i);
      }
    }
  }

  const pickedEntry = $derived(picked === null ? null : (entries[picked] ?? null));
  const pickedValue = $derived(pickedEntry ? (kind === 'set' ? pickedEntry.name : pickedEntry.value) : '');
  const pickedJson = $derived(pickedEntry ? asJson(pickedValue) : undefined);
  const known = $derived(['string', 'hash', 'list', 'set', 'zset', 'stream', 'ReJSON-RL'].includes(kind));
</script>

<section class="kv-detail" aria-label={key ?? m.kv_no_key()}>
  {#if !key}
    <div class="kv-blank">
      <KeyRound size={22} strokeWidth={1.4} aria-hidden="true" />
      <p>{m.kv_pick()}</p>
    </div>
  {:else}
    <header class="kv-dhead">
      {#if renaming !== null}
        <form class="kv-rename" onsubmit={(e) => (e.preventDefault(), void rename())}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="mono" bind:value={renaming} aria-label={m.kv_rename()} spellcheck="false" autofocus onkeydown={(e) => e.key === 'Escape' && (renaming = null)} />
          <button class="icon-btn" type="submit" aria-label={m.kv_rename()} data-tip={m.kv_rename()} disabled={busy}><Check size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn" type="button" aria-label={m.form_cancel()} data-tip={m.form_cancel()} onclick={() => (renaming = null)}><X size={14} strokeWidth={1.8} /></button>
        </form>
      {:else}
        <h2 class="kv-key mono" data-tip={key}>{key}</h2>
        <span class="spacer"></span>
        <button class="icon-btn" type="button" aria-label={m.kv_copy_name()} data-tip={m.kv_copy_name()} onclick={() => void copyText(key)}><ClipboardCopy size={14} strokeWidth={1.8} /></button>
        <button class="icon-btn" type="button" aria-label={m.kv_reload()} data-tip={m.kv_reload()} onclick={() => void st.readValue(sid)}><RefreshCw size={13} strokeWidth={1.8} /></button>
        {#if writable && v && v.data.shape !== 'missing'}
          <button class="icon-btn" type="button" aria-label={m.kv_rename()} data-tip={m.kv_rename()} onclick={() => (renaming = key)}><PenLine size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn" type="button" aria-label={m.kv_ttl_edit()} data-tip={m.kv_ttl_edit()} onclick={() => (ttlEdit = v?.ttl_ms ? String(Math.round(v.ttl_ms / 1000)) : '')}><Timer size={14} strokeWidth={1.8} /></button>
          <button class="icon-btn danger" class:armed={confirmDelete} type="button" aria-label={confirmDelete ? m.kv_delete_confirm() : m.kv_delete()} data-tip={confirmDelete ? m.kv_delete_confirm() : m.kv_delete()} onclick={() => void remove()}><Trash2 size={14} strokeWidth={1.8} /></button>
        {/if}
      {/if}
    </header>

    {#if v}
      <div class="kv-meta">
        <span class="kv-badge">{kind}</span>
        {#if v.length !== null && LENGTH[kind]}<span>{LENGTH[kind]?.(v.length)}</span>{/if}
        {#if size}<span>{bytes(size)}</span>{/if}
        <span class:kv-expiring={v.ttl_ms !== null}>{v.ttl_ms === null ? m.kv_no_ttl() : m.kv_ttl_left({ ttl: duration(v.ttl_ms) })}</span>
        {#if confirmDelete}<span class="kv-warn" role="status">{m.kv_delete_again()}</span>{/if}
      </div>
    {/if}

    {#if ttlEdit !== null}
      <form class="kv-ttlbar" onsubmit={(e) => (e.preventDefault(), void saveTtl())}>
        <label class="field">
          <span class="caps">{m.kv_ttl()}</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={ttlEdit} placeholder={m.kv_ph_ttl()} spellcheck="false" autofocus onkeydown={(e) => e.key === 'Escape' && (ttlEdit = null)} />
        </label>
        <button class="btn primary" type="submit" disabled={busy}>{m.kv_ttl_apply()}</button>
        {#if v?.ttl_ms !== null}<button class="btn ghost" type="button" disabled={busy} onclick={() => void saveTtl(true)}>{m.kv_ttl_clear()}</button>{/if}
        <button class="btn ghost" type="button" onclick={() => (ttlEdit = null)}>{m.form_cancel()}</button>
      </form>
    {/if}

    {#if st.valueError}
      <p class="kv-empty err" role="alert">{st.valueError}</p>
    {:else if !v}
      <p class="kv-empty">{m.schema_loading()}</p>
    {:else if v.data.shape === 'missing'}
      <p class="kv-empty">{known || kind === 'none' ? m.kv_gone() : m.kv_unknown_type({ kind })}</p>
    {:else if text}
      <div class="kv-texthead">
        {#if parsed !== undefined || kind === 'ReJSON-RL'}
          <Segmented label={m.kv_view()} value={mode} options={[{ value: 'text', label: m.kv_view_text() }, { value: 'json', label: m.kv_view_json() }]} onchange={(x) => (mode = x)} />
        {/if}
        {#if text.binary}<span class="kv-note">{m.kv_binary()}</span>{/if}
        {#if text.truncated}<span class="kv-note">{m.kv_truncated()}</span>{/if}
        <span class="spacer"></span>
        {#if draft !== null}
          <button class="btn ghost" type="button" disabled={busy} onclick={() => (draft = null)}>{m.kv_discard()}</button>
          <button class="btn primary" type="button" disabled={busy} onclick={() => void saveText()}>{m.kv_save()}</button>
        {/if}
      </div>
      {#if mode === 'json' && parsed !== undefined && draft === null}
        <div class="kv-json"><ValueJson value={parsed} query="" column={key} dialect={null} /></div>
      {:else}
        <textarea
          class="kv-text mono"
          spellcheck="false"
          aria-label={m.kv_col_value()}
          readonly={!writable || text.binary || text.truncated}
          value={draft ?? text.text}
          oninput={(e) => (draft = e.currentTarget.value)}
          onkeydown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) void saveText();
            if (e.key === 'Escape') draft = null;
          }}
        ></textarea>
      {/if}
    {:else}
      {@const cols = COLUMNS[kind]}
      <div class="kv-entries">
        <table
          class="tool-table kv-table"
          role="grid"
          tabindex="0"
          aria-label={key}
          aria-describedby="{uid}-keys"
          aria-activedescendant={picked === null ? undefined : `${uid}-r${picked}`}
          bind:this={table}
          onkeydown={entryKey}
        >
          <thead>
            <tr>
              <th class:num={kind === 'list'}>{cols?.[0]}</th>
              {#if cols?.[1]}<th class:num={kind === 'zset'}>{cols[1]}</th>{/if}
            </tr>
          </thead>
          <tbody>
            {#each entries as e, i (kind === 'list' ? i : e.name)}
              <tr id="{uid}-r{i}" aria-selected={picked === i} class:picked={picked === i} onclick={() => (picked = i)} oncontextmenu={(ev) => entryMenu(ev, e, i)} ondblclick={() => writable && ['hash', 'list', 'zset'].includes(kind) && (editing = { index: i, text: e.value })}>
                <td class="mono kv-cell-name" class:num={kind === 'list'}>{e.name}</td>
                {#if cols?.[1]}
                  <td class="mono kv-cell-value" class:num={kind === 'zset'}>
                    {#if editing?.index === i}
                      <!-- svelte-ignore a11y_autofocus -->
                      <textarea
                        class="kv-inline mono"
                        bind:value={editing.text}
                        autofocus
                        rows={Math.min(6, editing.text.split('\n').length)}
                        aria-label={cols[1]}
                        onkeydown={(ev) => {
                          if (ev.key === 'Enter' && !ev.shiftKey) (ev.preventDefault(), void saveEntry());
                          if (ev.key === 'Escape') editing = null;
                        }}
                        onblur={() => (editing = null)}
                      ></textarea>
                    {:else}{e.value}{/if}
                  </td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
        <span class="sr-only" id="{uid}-keys">{writable ? m.kv_entries_keys_edit() : m.kv_entries_keys()}</span>
        {#if v.next}
          <button class="btn ghost kv-more" type="button" disabled={st.valueBusy} onclick={() => void st.readValue(sid, true)}>{m.kv_more_items({ shown: num(entries.length), total: num(v.length ?? entries.length) })}</button>
        {/if}
      </div>

      {#if pickedEntry && (pickedJson !== undefined || pickedValue.length > 60 || pickedValue.includes('\n'))}
        <div class="kv-pick">
          <div class="kv-pickhead">
            <span class="caps">{kind === 'stream' ? pickedEntry.name : kind === 'set' ? m.kv_col_member() : pickedEntry.name}</span>
            <span class="spacer"></span>
            <button class="icon-btn" type="button" aria-label={m.kv_copy_value()} data-tip={m.kv_copy_value()} onclick={() => void copyText(pickedValue)}><ClipboardCopy size={13} strokeWidth={1.8} /></button>
            {#if writable}<button class="icon-btn danger" type="button" aria-label={m.kv_remove_item()} data-tip={m.kv_remove_item()} onclick={() => void removeEntry(pickedEntry)}><Trash2 size={13} strokeWidth={1.8} /></button>{/if}
          </div>
          {#if pickedJson !== undefined}
            <div class="kv-json small"><ValueJson value={pickedJson} query="" column={pickedEntry.name} dialect={null} /></div>
          {:else}
            <pre class="kv-full mono">{pickedValue}</pre>
          {/if}
        </div>
      {/if}

      {#if writable}
        <form class="kv-add" onsubmit={(e) => (e.preventDefault(), void add())}>
          {#if kind === 'hash' || kind === 'set' || kind === 'zset'}
            <input class="mono" bind:value={addName} placeholder={kind === 'hash' ? m.kv_col_field() : m.kv_col_member()} aria-label={kind === 'hash' ? m.kv_col_field() : m.kv_col_member()} spellcheck="false" />
          {/if}
          {#if kind === 'stream'}
            <textarea class="mono" bind:value={addValue} rows="2" placeholder={m.kv_stream_hint()} aria-label={m.kv_col_fields()} spellcheck="false"></textarea>
          {:else if kind !== 'set'}
            <input class="mono" bind:value={addValue} placeholder={kind === 'zset' ? m.kv_col_score() : m.kv_col_value()} aria-label={kind === 'zset' ? m.kv_col_score() : m.kv_col_value()} spellcheck="false" />
          {/if}
          {#if kind === 'list'}
            <button class="btn ghost" type="button" disabled={busy} onclick={() => void add(true)}>{m.kv_push_head()}</button>
            <button class="btn" type="submit" disabled={busy}><Plus size={13} strokeWidth={1.8} aria-hidden="true" />{m.kv_push_tail()}</button>
          {:else}
            <button class="btn" type="submit" disabled={busy}><Plus size={13} strokeWidth={1.8} aria-hidden="true" />{kind === 'stream' ? m.kv_add_entry() : m.kv_add()}</button>
          {/if}
        </form>
      {/if}
    {/if}
  {/if}
</section>
