<script lang="ts">
  import { ICON } from '../lib/icons';
  import { selectAll, toggleComment } from '@codemirror/commands';
  import { EditorView } from '@codemirror/view';
  import { ClipboardCopy, ClipboardPaste, Scissors, TextSelect } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { copyText, keys, openMenu } from '../lib/contextmenu.svelte';
  import { m } from '../paraglide/messages';
  import type { Schema } from '../lib/api';
  import type { Describe } from '../lib/colinfo';
  import type { Dialect, RunScope } from '../lib/bindings';
  import { completion, completionOf, extensions, formatSql, inspection, keyboard, keysOf, language, languageFor, look, lookOf, setMarks, setParamSpots, snippetSource, snippetsOf, type NativeLanguage } from '../lib/editor';
  import { formatConsole } from '../lib/nosql2-lang';
  import { formatCommands } from '../lib/mql';
  import { formatRedis, type KeyLookup } from '../lib/redis';
  import { canRenameAlias, inspections, renameAlias } from '../lib/complete/cm';
  import { registerCommands } from '../lib/palette.svelte';
  import { updateSection } from '../lib/prefs.svelte';
  import { snippet } from '@codemirror/autocomplete';
  import { library } from '../lib/library.svelte';
  import { currentBindings, prefs } from '../lib/prefs.svelte';
  import { snippetsFor } from '../lib/snippets';

  interface Props {
    text: string;
    dialect: Dialect | null;
    native?: NativeLanguage;
    keyNames?: KeyLookup | null;
    schema: Schema | null;
    running: [number, number] | null;
    error: [number, number] | null;
    onchange: (text: string) => void;
    onrun: (scope: RunScope, dryRun?: boolean, keepGoing?: boolean) => void;
    oncancel: () => boolean;
    spots?: readonly (readonly [number, number])[];
    onselect?: () => void;
    product?: string;
    searchPath?: readonly string[] | null;
    describe?: Describe | null;
  }
  let { text, dialect, native = 'mql', keyNames = null, schema, running, error, onchange, onrun, oncancel, spots = [], onselect, product = '', searchPath = null, describe = null }: Props = $props();

  let host: HTMLDivElement;
  let view: EditorView | null = null;

  function scope(all: boolean): RunScope {
    if (!view) return { kind: 'script' };
    const sel = view.state.selection.main;
    if (!sel.empty) return { kind: 'selection', from: sel.from, to: sel.to };
    return all ? { kind: 'script' } : { kind: 'statement', cursor: sel.head };
  }

  export function runScope(all: boolean): RunScope {
    return scope(all);
  }

  export async function format() {
    if (!view) return;
    const sel = view.state.selection.main;
    const [from, to] = sel.empty ? [0, view.state.doc.length] : [sel.from, sel.to];
    const before = view.state.doc;
    try {
      const part = view.state.sliceDoc(from, to);
      const out = dialect ? await formatSql(part, dialect, prefs.value.editor) : native === 'redis' ? formatRedis(part) : native === 'search' ? formatConsole(part) : native === 'cql' ? part : formatCommands(part);
      if (view.state.doc === before) view.dispatch({ changes: { from, to, insert: out } });
    } catch {
      return;
    }
  }

  export function insert(sql: string) {
    if (!view) return;
    const end = view.state.doc.length;
    const prefix = end > 0 && !view.state.doc.toString().endsWith('\n\n') ? '\n\n' : '';
    view.dispatch({ changes: { from: end, insert: prefix + sql }, selection: { anchor: end + prefix.length + sql.length } });
    view.focus();
  }

  export function focus() {
    view?.focus();
  }

  export function renameAliasHere(): boolean {
    if (!view) return false;
    const done = renameAlias(view);
    view.focus();
    return done;
  }

  export function content() {
    return view?.state.doc.toString() ?? text;
  }

  export function selected() {
    if (!view) return '';
    const sel = view.state.selection.main;
    return sel.empty ? '' : view.state.sliceDoc(sel.from, sel.to);
  }

  export function insertAtCursor(value: string) {
    if (!view) return;
    view.dispatch(view.state.replaceSelection(value));
    view.focus();
  }

  export function insertSnippet(template: string) {
    if (!view) return;
    const sel = view.state.selection.main;
    snippet(template)(view, null, sel.from, sel.to);
    view.focus();
  }

  function oncontextmenu(e: MouseEvent) {
    const v = view;
    if (!v) return;
    let sel = v.state.selection.main;
    const renameKey = currentBindings().rename_alias;
    const at = e.clientX || e.clientY ? v.posAtCoords({ x: e.clientX, y: e.clientY }) : null;
    if (at !== null && (sel.empty || at < sel.from || at > sel.to)) {
      v.dispatch({ selection: { anchor: at } });
      sel = v.state.selection.main;
    }
    const text = v.state.sliceDoc(sel.from, sel.to);
    const cursor = sel.head;
    openMenu(e, [
      { label: m.editor_run_statement(), icon: ICON.run, shortcut: sel.empty ? keys('Mod+Enter') : undefined, run: () => onrun({ kind: 'statement', cursor }) },
      { label: m.editor_run_selection(), shortcut: sel.empty ? undefined : keys('Mod+Enter'), disabled: sel.empty, run: () => onrun({ kind: 'selection', from: sel.from, to: sel.to }) },
      { label: m.run_script(), shortcut: keys('Shift+Mod+Enter'), run: () => onrun({ kind: 'script' }) },
      { label: m.cmp_run_keep_going(), icon: ICON.runKeepGoing, run: () => onrun({ kind: 'script' }, false, true) },
      { label: m.tx_dry(), icon: ICON.dryRun, shortcut: currentBindings().dry_run ? keys(currentBindings().dry_run) : undefined, run: () => onrun(sel.empty ? { kind: 'statement', cursor } : { kind: 'selection', from: sel.from, to: sel.to }, true) },
      'separator',
      { label: m.run_format(), icon: ICON.format, shortcut: keys('Shift+Alt+F'), run: format },
      { label: m.editor_comment(), icon: ICON.comment, shortcut: keys('Mod+/'), run: () => toggleComment(v) },
      { label: m.key_rename_alias(), icon: ICON.rename, shortcut: renameKey ? keys(renameKey) : undefined, disabled: !canRenameAlias(v.state), run: () => renameAliasHere() },
      'separator',
      {
        label: m.menu_cut(),
        icon: Scissors,
        shortcut: keys('Mod+X'),
        disabled: sel.empty,
        run: async () => {
          await copyText(text);
          v.dispatch({ changes: { from: sel.from, to: sel.to }, selection: { anchor: sel.from } });
        },
      },
      { label: m.menu_copy(), icon: ClipboardCopy, shortcut: keys('Mod+C'), disabled: sel.empty, run: () => copyText(text) },
      {
        label: m.menu_paste(),
        icon: ClipboardPaste,
        shortcut: keys('Mod+V'),
        run: async () => {
          const clip = await navigator.clipboard.readText().catch(() => '');
          if (clip) v.dispatch(v.state.replaceSelection(clip));
          v.focus();
        },
      },
      { label: m.menu_select_all(), icon: TextSelect, shortcut: keys('Mod+A'), run: () => selectAll(v) },
    ]);
  }

  const handlers = {
    onChange: (t: string) => onchange(t),
    onRunStatement: () => onrun(scope(false)),
    onRunScript: () => onrun(scope(true)),
    onDryRun: () => onrun(scope(false), true),
    onCancel: () => oncancel(),
    onFormat: () => format(),
    onSelect: () => onselect?.(),
  };

  onMount(() => {
    view = new EditorView({
      parent: host,
      doc: text,
      extensions: extensions(dialect, schema, handlers, { prefs: prefs.value.editor, product, bindings: currentBindings(), snippets: dialect ? snippetsFor(dialect, library.snippets) : [], native, keys: keyNames }),
    });
    view.focus();
    return () => view?.destroy();
  });

  $effect(() => {
    view?.dispatch({ effects: setMarks.of({ running, error }) });
  });

  $effect(() => {
    view?.dispatch({ effects: setParamSpots.of(spots) });
  });

  $effect(() => {
    const upper = prefs.value.editor.uppercase_keywords;
    view?.dispatch({ effects: language.reconfigure(languageFor(dialect, schema, { product, upper, native, keys: keyNames, searchPath, describe })) });
  });

  $effect(() => {
    const on = prefs.value.editor.inspections;
    view?.dispatch({ effects: inspection.reconfigure(dialect ? inspections(dialect, schema, on, searchPath) : []) });
  });

  $effect(() =>
    registerCommands([
      { id: 'editor.rename_alias', label: m.key_rename_alias, group: m.pal_group_query, keys: () => currentBindings().rename_alias || null, run: () => renameAliasHere() },
      { id: 'editor.run_keep_going', label: m.cmp_run_keep_going, group: m.pal_group_query, run: () => onrun({ kind: 'script' }, false, true) },
      { id: 'editor.comment', label: m.pal_cmd_comment, group: m.pal_group_query, icon: ICON.comment, keys: () => 'Mod+/', run: () => view && toggleComment(view) },
      {
        id: 'editor.dry_run',
        label: m.pal_cmd_dry_run,
        group: m.pal_group_query,
        icon: ICON.dryRun,
        when: () => !running,
        run: () => {
          if (!view) return;
          const sel = view.state.selection.main;
          onrun(sel.empty ? { kind: 'statement', cursor: sel.head } : { kind: 'selection', from: sel.from, to: sel.to }, true);
        },
      },
      {
        id: 'editor.continue_on_error',
        label: () => (prefs.value.editor.continue_on_error ? m.cmp_stop_on_error() : m.cmp_continue_on_error()),
        group: m.pal_group_query,
        run: () => updateSection('editor', { continue_on_error: !prefs.value.editor.continue_on_error }),
      },
      {
        id: 'editor.inspections',
        label: () => (prefs.value.editor.inspections ? m.cmp_inspections_off() : m.cmp_inspections_on()),
        group: m.pal_group_query,
        run: () => updateSection('editor', { inspections: !prefs.value.editor.inspections }),
      },
    ]),
  );

  $effect(() => {
    const p = $state.snapshot(prefs.value.editor);
    view?.dispatch({ effects: [look.reconfigure(lookOf(p)), completion.reconfigure(completionOf(p))] });
  });

  $effect(() => {
    const b = currentBindings();
    view?.dispatch({ effects: keyboard.reconfigure(keysOf(b, handlers)) });
  });

  $effect(() => {
    const list = dialect ? snippetsFor(dialect, $state.snapshot(library.snippets)) : [];
    view?.dispatch({ effects: snippetSource.reconfigure(snippetsOf(list)) });
  });
</script>

<div class="editor" bind:this={host} {oncontextmenu} role="none"></div>

<style>
  .editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
</style>
