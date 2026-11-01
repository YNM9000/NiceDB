<script lang="ts">
  import { ArrowLeft, ArrowRight, Bookmark, CornerDownRight, Pin, Search } from '@lucide/svelte';
  import { commandIcon, KIND_ICON } from '../lib/icons';
  import { tick, untrack } from 'svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { compact } from '../lib/format';
  import { highlight } from '../lib/fuzzy';
  import { actionsOf, answer, choose, closePalette, commandItems, commandOf, rankChoices, stopAsking, isBookmarked, isPinned, kindLabel, libraryItems, LIVE_EVERY, objectItems, palette, refreshLive, search, searchSliced, SLICED_FROM, toggleBookmark, togglePin, type PaletteAction } from '../lib/palette.svelte';
  import type { Mode, PaletteItem, Section } from '../lib/palette-items';
  import { connections } from '../lib/connections.svelte';
  import { workspace } from '../lib/workspace.svelte';
  import { comboOf } from '../lib/shortcuts';
  import { m } from '../paraglide/messages';


  const MODES: { id: Mode; label: () => string }[] = [
    { id: 'objects', label: m.pal_mode_objects },
    { id: 'library', label: m.pal_mode_library },
    { id: 'commands', label: m.pal_mode_commands },
  ];
  const SECTION: Record<Section['id'], () => string> = {
    pinned: m.pal_pinned,
    bookmarks: m.pal_bookmarks,
    recent: m.pal_recent,
    results: m.pal_results,
    table: m.pal_kinds_table,
    view: m.pal_kinds_view,
    column: m.pal_kinds_column,
    function: m.pal_kinds_function,
    sequence: m.pal_kinds_sequence,
    type: m.pal_kinds_type,
    trigger: m.pal_kinds_trigger,
    index: m.pal_kinds_index,
    database: m.pal_kinds_database,
    session: m.pal_kinds_session,
    user: m.pal_kinds_user,
    script: m.pal_kinds_script,
    note: m.pal_kinds_note,
    snippet: m.pal_kinds_snippet,
    connection: m.pal_kinds_connection,
    space: m.pal_kinds_space,
    query: m.pal_kinds_query,
    command: m.pal_kinds_command,
  };

  let dialog: HTMLDialogElement;
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLDivElement>();
  let active = $state(0);
  let acting = $state<{ item: PaletteItem; actions: PaletteAction[] } | null>(null);
  let actionAt = $state(0);

  $effect(() => {
    if (!palette.open || palette.mode !== 'objects') return;
    const t = setInterval(() => refreshLive(), LIVE_EVERY);
    return () => clearInterval(t);
  });

  const objects = $derived(objectItems());
  const library = $derived(libraryItems());
  const commands = $derived(commandItems());
  const sources = {
    objects: () => objects,
    library: () => library,
    commands: () => commands,
  };
  const all = $derived(sources[palette.mode]());
  const sliced = (list: readonly PaletteItem[]) => list.length > SLICED_FROM && !!palette.query.trim();
  let late = $state.raw<Section[]>([]);
  let lateFor = '';
  let lateObjects = $state(0);
  const sections = $derived(sliced(all) ? late : search(all, palette.mode, palette.query));

  $effect(() => {
    if (!sliced(all)) return;
    const [list, mode, query] = [all, palette.mode, palette.query];
    return untrack(() =>
      searchSliced(list, mode, query, (found) => {
        late = found;
        lateFor = `${mode}\u0000${query}`;
      }),
    );
  });

  function settle() {
    const want = `${palette.mode}\u0000${palette.query}`;
    if (!sliced(all) || lateFor === want) return;
    late = search(all, palette.mode, palette.query);
    lateFor = want;
  }

  $effect(() => {
    if (palette.mode === 'objects' || !sliced(objects)) return;
    const [list, query] = [objects, palette.query];
    return untrack(() => searchSliced(list, 'objects', query, (found) => (lateObjects = found[0]?.items.length ?? 0)));
  });
  const asked = $derived(commandOf(palette.asking));
  const choices = $derived(asked?.ask?.choices ? rankChoices(asked.ask.choices(), palette.query) : null);
  let choiceAt = $state(0);
  $effect(() => {
    if (palette.asking) input?.focus();
  });
  $effect(() => {
    void palette.query;
    void palette.asking;
    choiceAt = 0;
  });
  $effect(() => {
    if (!choices) return;
    void tick().then(() => list?.querySelector<HTMLElement>(`#pal-choice-${choiceAt}`)?.scrollIntoView({ block: 'nearest' }));
  });

  function pick(i = choiceAt) {
    if (choices) {
      const c = choices[i];
      if (c) void answer(c.id);
    } else if (palette.query.trim()) void answer(palette.query.trim());
  }

  function askKeys(e: KeyboardEvent): boolean {
    const combo = comboOf(e);
    const n = choices?.length ?? 0;
    switch (combo) {
      case 'ArrowDown':
        if (n) choiceAt = (choiceAt + 1) % n;
        break;
      case 'ArrowUp':
        if (n) choiceAt = (choiceAt - 1 + n) % n;
        break;
      case 'PageDown':
        if (n) choiceAt = Math.min(n - 1, choiceAt + 10);
        break;
      case 'PageUp':
        choiceAt = Math.max(0, choiceAt - 10);
        break;
      case 'Enter':
        pick();
        break;
      case 'Escape':
        stopAsking();
        break;
      case 'Backspace':
        if (palette.query) return false;
        stopAsking();
        break;
      case 'ArrowLeft':
        if (palette.query) return false;
        stopAsking();
        break;
      case 'Tab':
      case 'Shift+Tab':
        break;
      default:
        return false;
    }
    e.preventDefault();
    return true;
  }

  const flat = $derived(sections.flatMap((s) => s.items));
  const current = $derived(flat[active] ?? null);
  const currentActions = $derived(current ? actionsOf(current) : []);
  const openSessions = $derived(Object.keys(workspace.sessions).length);

  const others = $derived.by(() => {
    if (!palette.query.trim()) return null;
    const count = (mode: Mode) => {
      if (mode === palette.mode) return flat.length;
      if (mode === 'objects' && sliced(objects)) return lateObjects;
      return search(sources[mode](), mode, palette.query)[0]?.items.length ?? 0;
    };
    return Object.fromEntries(MODES.map((x) => [x.id, count(x.id)])) as Record<Mode, number>;
  });

  $effect(() => {
    const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.showModal();
    input?.focus();
    return () => {
      dialog.close();
      queueMicrotask(() => {
        if (!document.activeElement || document.activeElement === document.body) back?.focus();
      });
    };
  });

  $effect(() => {
    void palette.query;
    void palette.mode;
    active = 0;
    acting = null;
  });

  $effect(() => {
    if (acting) return;
    void tick().then(() => list?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' }));
  });

  function setMode(mode: Mode) {
    palette.mode = mode;
    input?.focus();
  }

  function oninput() {
    if (palette.query.startsWith('>') && palette.mode !== 'commands') {
      palette.query = palette.query.slice(1).trimStart();
      palette.mode = 'commands';
    }
  }

  function step(by: number) {
    if (acting) {
      const n = acting.actions.length;
      if (n) actionAt = (actionAt + by + n) % n;
      return;
    }
    const n = flat.length;
    if (!n) return;
    active = Math.abs(by) > 1 ? Math.max(0, Math.min(n - 1, active + by)) : (active + by + n) % n;
  }

  function openActions(item: PaletteItem | null) {
    if (!item) return;
    acting = { item, actions: actionsOf(item) };
    actionAt = 0;
  }

  async function run(item: PaletteItem | null, action: PaletteAction | undefined) {
    if (!item || !action) return;
    const keep = action.id === 'pin' || action.id === 'bookmark';
    await choose(item, action);
    if (keep && acting) acting = { item, actions: actionsOf(item) };
  }

  function copySelected(e: KeyboardEvent) {
    const el = input;
    if (!el || el.selectionStart !== el.selectionEnd || !current) return false;
    const copy = currentActions.find((a) => a.id === 'copy' || a.id === 'signature');
    if (!copy) return false;
    e.preventDefault();
    void run(current, copy);
    return true;
  }

  function onkeydown(e: KeyboardEvent) {
    if (asked && askKeys(e)) return;
    if (asked) return;
    const combo = comboOf(e);
    if (combo?.endsWith('Enter')) settle();
    const caretAtEnd = !!input && input.selectionStart === input.value.length;
    switch (combo) {
      case 'ArrowDown':
        step(1);
        break;
      case 'ArrowUp':
        step(-1);
        break;
      case 'PageDown':
        step(10);
        break;
      case 'PageUp':
        step(-10);
        break;
      case 'Mod+ArrowDown':
      case 'Mod+End':
        if (acting) actionAt = acting.actions.length - 1;
        else active = Math.max(0, flat.length - 1);
        break;
      case 'Mod+ArrowUp':
      case 'Mod+Home':
        if (acting) actionAt = 0;
        else active = 0;
        break;
      case 'Enter':
        if (acting) void run(acting.item, acting.actions[actionAt]);
        else void run(current, currentActions[0]);
        break;
      case 'Shift+Enter':
        if (!acting) void run(current, currentActions[1] ?? currentActions[0]);
        break;
      case 'ArrowRight':
        if (acting || !caretAtEnd || !current) return;
        openActions(current);
        break;
      case 'Mod+Enter':
        if (!acting) openActions(current);
        break;
      case 'ArrowLeft':
        if (!acting) return;
        acting = null;
        break;
      case 'Tab':
      case 'Shift+Tab': {
        const at = MODES.findIndex((x) => x.id === palette.mode);
        const next = MODES[(at + (combo === 'Tab' ? 1 : -1) + MODES.length) % MODES.length];
        if (next) palette.mode = next.id;
        break;
      }
      case 'Mod+B':
        if (current) void togglePin(current);
        break;
      case 'Mod+D':
        if (current && (current.kind === 'table' || current.kind === 'view' || current.kind === 'query')) void toggleBookmark(current);
        break;
      case 'Mod+C':
        if (!copySelected(e)) return;
        break;
      case 'Escape':
        if (acting) acting = null;
        else closePalette();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  function parts(text: string): [string, string, string] {
    const hit = palette.query ? highlight(text, palette.query) : null;
    if (!hit) return [text, '', ''];
    return [text.slice(0, hit[0]), text.slice(hit[0], hit[1]), text.slice(hit[1])];
  }

  const detailOf = (it: PaletteItem) => {
    if (it.kind === 'command') return it.detail ? keys(it.detail) : '';
    if ((it.kind === 'table' || it.kind === 'view') && it.count !== null) return m.schema_rows({ count: compact(it.count) });
    return it.detail;
  };

  const indexed = $derived.by(() => {
    let from = 0;
    return sections.map((s) => {
      const at = from;
      from += s.items.length;
      return { ...s, from: at };
    });
  });

  const empty = $derived.by(() => {
    if (flat.length) return null;
    if (palette.query.trim()) return m.pal_none({ query: palette.query.trim() });
    if (palette.mode === 'objects') return openSessions ? m.pal_objects_loading() : connections.list.length ? m.pal_objects_closed() : m.pal_objects_none();
    return m.pal_empty();
  });
</script>

<dialog class="sheet-dialog cmd-palette" bind:this={dialog} aria-label={m.pal_title()} oncancel={(e) => (e.preventDefault(), asked ? stopAsking() : acting ? (acting = null) : closePalette())} onclick={(e) => e.target === dialog && closePalette()}>
  <div class="pal-head">
    <div class="pal-modes" role="tablist" aria-label={m.pal_modes()}>
      {#each MODES as x (x.id)}
        <button type="button" role="tab" tabindex="-1" aria-selected={palette.mode === x.id} onclick={() => setMode(x.id)}>
          {x.label()}
          {#if others && others[x.id] > 0}<span class="pal-count">{others[x.id] >= 80 ? '80+' : others[x.id]}</span>{/if}
        </button>
      {/each}
      <span class="spacer"></span>
      <span class="pal-keyhint"><kbd>Tab</kbd> {m.pal_next_mode()}</span>
    </div>
    <label class="pal-field">
      {#if asked}
        {@const Glyph = commandIcon(asked.id, asked.icon)}
        <button class="icon-btn pal-back" type="button" aria-label={m.pal_back()} onclick={() => (stopAsking(), input?.focus())}><ArrowLeft size={15} strokeWidth={1.8} /></button>
        <span class="pal-asking"><Glyph size={13} strokeWidth={1.8} aria-hidden="true" />{asked.label()}</span>
      {:else if acting}
        <button class="icon-btn pal-back" type="button" aria-label={m.pal_back()} onclick={() => ((acting = null), input?.focus())}><ArrowLeft size={15} strokeWidth={1.8} /></button>
      {:else}
        <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      {/if}
      <input
        bind:this={input}
        bind:value={palette.query}
        {oninput}
        {onkeydown}
        type="text"
        role="combobox"
        spellcheck="false"
        autocomplete="off"
        aria-expanded="true"
        aria-controls="pal-list"
        aria-autocomplete="list"
        aria-activedescendant={asked ? (choices?.length ? `pal-choice-${choiceAt}` : undefined) : acting ? `pal-act-${actionAt}` : current ? `pal-row-${active}` : undefined}
        aria-label={m.pal_title()}
        placeholder={asked?.ask ? asked.ask.prompt() : acting ? acting.item.label : palette.mode === 'objects' ? m.pal_hint_objects() : palette.mode === 'library' ? m.pal_hint_library() : m.pal_hint_commands()}
        readonly={!!acting}
      />
    </label>
  </div>

  <div class="pal-list" id="pal-list" role="listbox" aria-label={acting ? m.pal_actions_of({ name: acting.item.label }) : m.pal_results()} bind:this={list}>
    {#if asked}
      {#if choices}
        {#if !choices.length}
          <p class="pal-empty">{palette.query.trim() ? m.pal_none({ query: palette.query.trim() }) : m.pal_ask_nothing()}</p>
        {:else}
          {#each choices as c, i (c.id)}
            {@const [a, b, rest] = parts(c.label)}
            {@const Glyph = c.icon ?? CornerDownRight}
            <div id="pal-choice-{i}" class="pal-row pal-choice" class:active={i === choiceAt} role="option" aria-selected={i === choiceAt} tabindex="-1" onpointermove={() => (choiceAt = i)} onclick={() => pick(i)} onkeydown={() => {}}>
              <span class="pal-icon"><Glyph size={14} strokeWidth={1.8} /></span>
              <span class="pal-label">{a}<mark>{b}</mark>{rest}</span>
              <span class="pal-where">{c.where ?? ''}</span>
              <span class="pal-detail">{#if i === choiceAt}<kbd>↵</kbd>{/if}</span>
            </div>
          {/each}
        {/if}
      {:else}
        <p class="pal-empty">{palette.query.trim() ? m.pal_ask_value_ready({ value: palette.query.trim() }) : m.pal_ask_value_hint()}</p>
      {/if}
    {:else if acting}
      <p class="pal-section caps" role="presentation">{kindLabel(acting.item.kind)} · {acting.item.label}</p>
      {#each acting.actions as a, i (a.id)}
        <div id="pal-act-{i}" class="pal-row pal-action" class:active={i === actionAt} class:danger={a.danger} role="option" aria-selected={i === actionAt} tabindex="-1" onpointermove={() => (actionAt = i)} onclick={() => acting && void run(acting.item, a)} onkeydown={() => {}}>
          <span class="pal-icon"><a.icon size={14} strokeWidth={1.8} /></span>
          <span class="pal-label">{a.label}</span>
          <span class="pal-where"></span>
          <span class="pal-detail">{#if i === 0}<kbd>↵</kbd>{:else if i === 1 && a.id !== 'pin' && a.id !== 'bookmark'}<kbd>{keys('Shift+Enter')}</kbd>{:else if a.id === 'pin'}<kbd>{keys('Mod+B')}</kbd>{:else if a.id === 'bookmark'}<kbd>{keys('Mod+D')}</kbd>{/if}</span>
        </div>
      {/each}
    {:else if empty}
      <p class="pal-empty">{empty}</p>
    {:else}
      {#each indexed as s (s.id)}
        <p class="pal-section caps" role="presentation">{SECTION[s.id]()}</p>
        {#each s.items as it, j (it.key)}
          {@const i = s.from + j}
          {@const [a, b, c] = parts(it.label)}
          {@const Glyph = it.kind === 'command' ? commandIcon(it.ref ?? '', commandOf(it.ref)?.icon) : KIND_ICON[it.kind]}
          <div id="pal-row-{i}" data-index={i} class="pal-row kind-{it.kind}" class:active={i === active} role="option" aria-selected={i === active} tabindex="-1" onpointermove={() => (active = i)} onclick={() => void run(it, actionsOf(it)[0])} oncontextmenu={(e) => (e.preventDefault(), (active = i), openActions(it))} onkeydown={() => {}}>
            <span class="pal-icon" data-tip={kindLabel(it.kind)}><Glyph size={14} strokeWidth={1.8} /></span>
            <span class="pal-label">{a}<mark>{b}</mark>{c}</span>
            <span class="pal-where">{it.where}</span>
            <span class="pal-detail">
              {#if isPinned(it.key)}<Pin size={11} strokeWidth={2} aria-label={m.pal_pinned_one()} />{:else if isBookmarked(it.key)}<Bookmark size={11} strokeWidth={2} aria-label={m.pal_bookmarked_one()} />{/if}
              {#if it.kind === 'command' && it.detail}<kbd>{detailOf(it)}</kbd>{:else}{detailOf(it)}{/if}
            </span>
          </div>
        {/each}
      {/each}
    {/if}
  </div>

  <footer class="pal-foot">
    {#if asked}
      <span><kbd>↵</kbd> {choices ? m.pal_do() : m.pal_ask_confirm()}</span>
      <span><kbd>Esc</kbd> {m.pal_back()}</span>
    {:else if acting}
      <span><kbd>↵</kbd> {m.pal_do()}</span>
      <span><kbd class="pal-arrow"><ArrowLeft size={10} strokeWidth={2} /></kbd> {m.pal_back()}</span>
    {:else if current}
      {#if currentActions[0]}<span><kbd>↵</kbd> {currentActions[0].label}</span>{/if}
      {#if currentActions[1] && currentActions[1].id !== 'pin'}<span><kbd>{keys('Shift+Enter')}</kbd> {currentActions[1].label}</span>{/if}
      <span><kbd class="pal-arrow"><ArrowRight size={10} strokeWidth={2} /></kbd> {m.pal_more()}</span>
      <span class="spacer"></span>
      <span><kbd>{keys('Mod+B')}</kbd> {isPinned(current.key) ? m.pal_unpin() : m.pal_pin()}</span>
    {:else}
      <span><kbd>&gt;</kbd> {m.pal_prefix_commands()}</span>
      <span class="spacer"></span>
      <span><kbd>Esc</kbd> {m.org_close()}</span>
    {/if}
  </footer>
</dialog>
