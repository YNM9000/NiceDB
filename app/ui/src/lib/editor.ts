import { autocompletion, closeBrackets, closeCompletion, closeBracketsKeymap, completionKeymap, snippetCompletion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';
import { addCursor, columnHover, completionSource, inspections, kindColumn, recency, renameAlias, sqlDialect } from './complete/cm';
import { engineOf } from './complete/dialects';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { bracketMatching, HighlightStyle, indentOnInput, indentUnit, LanguageSupport, syntaxHighlighting } from '@codemirror/language';
import { MSSQL, MySQL, PLSQL, PostgreSQL, SQLite, StandardSQL, type SQLDialect } from '@codemirror/lang-sql';
import { ClickHouseSql } from './clickhouse-sql';
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
import { Compartment, EditorState, StateEffect, StateField, type Extension } from '@codemirror/state';
import { crosshairCursor, Decoration, type DecorationSet, ViewPlugin, type ViewUpdate, drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers, placeholder, rectangularSelection } from '@codemirror/view';
import { tags as t } from '@lezer/highlight';
import type { Schema } from './api';
import type { Describe } from './colinfo';
import type { Dialect } from './bindings';
import { forEditor, type Bindings } from './shortcuts';
import type { Preferences } from './api';
import type { ReadySnippet } from './snippets';
import { mqlLanguage } from './mql';
import { redisLanguage, type KeyLookup } from './redis';
import { CqlDialect, cqlLanguage, PartiqlDialect, partiqlLanguage, searchLanguage } from './nosql-lang';
import type { Language } from './bindings';

export type NativeLanguage = Exclude<Language['kind'], 'sql'>;
import { m } from '../paraglide/messages';
import { scan } from './complete/scan';

export const language = new Compartment();
export const look = new Compartment();
export const completion = new Compartment();
export const keyboard = new Compartment();
export const snippetSource = new Compartment();
export const inspection = new Compartment();

const EDITOR_DIALECT: Record<Dialect, SQLDialect> = { postgres: PostgreSQL, sqlite: SQLite, mysql: MySQL, mssql: MSSQL, duckdb: PostgreSQL, clickhouse: ClickHouseSql, firebird: StandardSQL, hana: StandardSQL, db2: StandardSQL, ansi: StandardSQL, ansi_backtick: MySQL, cql: CqlDialect, partiql: PartiqlDialect, opensearch: MySQL, elasticsearch: StandardSQL, oracle: PLSQL };
const FORMAT_LANGUAGE = { postgres: 'postgresql', sqlite: 'sqlite', mysql: 'postgresql', mssql: 'transactsql', duckdb: 'duckdb', clickhouse: 'clickhouse', firebird: 'sql', hana: 'sql', db2: 'db2', ansi: 'sql', ansi_backtick: 'mysql', cql: 'sql', partiql: 'sql', opensearch: 'mysql', elasticsearch: 'sql', oracle: 'plsql' } as const satisfies Record<Dialect, string>;

export interface LanguageOptions {
  product?: string;
  upper?: boolean;
  native?: NativeLanguage;
  keys?: KeyLookup | null;
  searchPath?: readonly string[] | null;
  describe?: Describe | null;
}

export function sqlLanguage(dialect: Dialect, schema: Schema | null, opts: LanguageOptions = {}, completes = true): Extension {
  const lang = EDITOR_DIALECT[dialect].language;
  const env = { dialect, engine: engineOf(dialect, opts.product), schema, upper: opts.upper ?? false, searchPath: opts.searchPath ?? null, describe: opts.describe ?? null };
  return [new LanguageSupport(lang), sqlDialect.of(dialect), completes ? [lang.data.of({ autocomplete: completionSource(env) }), schema ? columnHover(env) : []] : [], dialect === 'mysql' ? delimiterMarks : []];
}

const MAX_DELIMITER_SCAN = 400_000;
const delimiterLine = Decoration.mark({ class: 'cm-delimiter' });
const delimiterEnd = Decoration.mark({ class: 'cm-delimiter-end' });

function delimiterDeco(doc: string): DecorationSet {
  if (doc.length > MAX_DELIMITER_SCAN || !/delimiter/i.test(doc)) return Decoration.none;
  const ranges = [];
  for (const t of scan(doc, 'mysql')) {
    if (t.k === 'comment' && /^delimiter\s/i.test(doc.slice(t.from, t.from + 10))) ranges.push(delimiterLine.range(t.from, t.to));
    else if (t.k === 'punct' && t.text === ';' && t.to > t.from && doc.slice(t.from, t.to) !== ';') ranges.push(delimiterEnd.range(t.from, t.to));
  }
  return Decoration.set(ranges, true);
}

const delimiterMarks = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = delimiterDeco(view.state.doc.toString());
    }
    update(u: ViewUpdate) {
      if (u.docChanged) this.decorations = delimiterDeco(u.state.doc.toString());
    }
  },
  { decorations: (v) => v.decorations },
);

export function languageFor(dialect: Dialect | null, schema: Schema | null, opts: LanguageOptions = {}, completes = true): Extension {
  if (dialect) return sqlLanguage(dialect, schema, opts, completes);
  switch (opts.native) {
    case 'redis':
      return redisLanguage(opts.keys ?? null, completes);
    case 'cql':
      return cqlLanguage(schema, completes);
    case 'partiql':
      return partiqlLanguage(schema, completes);
    case 'search':
      return searchLanguage(schema, completes);
    default:
      return mqlLanguage(schema, completes);
  }
}

export const setMarks = StateEffect.define<{ running: [number, number] | null; error: [number, number] | null }>();

const marks = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(setMarks)) continue;
      const ranges = [];
      const len = tr.state.doc.length;
      if (e.value.running) {
        const [a, b] = e.value.running;
        if (b > a && b <= len) ranges.push(Decoration.mark({ class: 'cm-running' }).range(a, b));
      }
      if (e.value.error) {
        const [a, b] = e.value.error;
        if (b > a && b <= len) ranges.push(Decoration.mark({ class: 'cm-error-mark' }).range(a, b));
      }
      deco = Decoration.set(ranges, true);
    }
    return deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

export const setParamSpots = StateEffect.define<readonly (readonly [number, number])[]>();

const paramMark = Decoration.mark({ class: 'cm-param' });

const paramSpots = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(setParamSpots)) continue;
      const len = tr.state.doc.length;
      deco = Decoration.set(
        e.value.filter(([a, b]) => b > a && b <= len).map(([a, b]) => paramMark.range(a, b)),
        true,
      );
    }
    return deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.operatorKeyword, t.modifier], color: 'var(--nd-syn-keyword)', fontWeight: 'var(--nd-syn-keyword-weight)' },
  { tag: [t.function(t.variableName), t.standard(t.name)], color: 'var(--nd-syn-function)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--nd-syn-string)' },
  { tag: [t.number, t.bool, t.null], color: 'var(--nd-syn-number)' },
  { tag: [t.lineComment, t.blockComment], color: 'var(--nd-syn-comment)', fontStyle: 'italic' },
  { tag: [t.typeName], color: 'var(--nd-syn-function)' },
  { tag: [t.punctuation, t.operator], color: 'var(--nd-ink-2)' },
  { tag: [t.special(t.name), t.quote], color: 'var(--nd-ink)' },
]);

const theme = EditorView.theme({
  '&': { height: '100%', color: 'var(--nd-ink)', backgroundColor: 'transparent' },
  '.cm-scroller': { fontFamily: 'var(--nd-font-mono)', lineHeight: '20px' },
  '.cm-content': { caretColor: 'var(--nd-ink)', padding: '10px 0' },
  '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--nd-ink-3)', border: 'none', paddingLeft: '6px' },
  '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--nd-ink) 3.5%, transparent)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--nd-ink)' },
  '.cm-cursor': { borderLeftColor: 'var(--nd-ink)', borderLeftWidth: '2px' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': { backgroundColor: 'var(--nd-sel) !important' },
  '.cm-selectionMatch': { backgroundColor: 'color-mix(in srgb, var(--nd-accent) 14%, transparent)' },
  '.cm-matchingBracket': { outline: '1px solid var(--nd-rule)', backgroundColor: 'transparent' },
  '.cm-running': { backgroundColor: 'color-mix(in srgb, var(--nd-pending) 13%, transparent)' },
  '.cm-param': { color: 'var(--nd-accent)', backgroundColor: 'color-mix(in srgb, var(--nd-accent) 9%, transparent)', borderRadius: '2px', boxShadow: '0 1px 0 color-mix(in srgb, var(--nd-accent) 55%, transparent)' },
  '.cm-delimiter, .cm-delimiter *': { color: 'var(--nd-syn-keyword)', fontWeight: 'var(--nd-syn-keyword-weight)', fontStyle: 'normal' },
  '.cm-delimiter-end, .cm-delimiter-end *': { color: 'var(--nd-accent)', fontWeight: '600' },
  '.cm-tooltip.cm-tooltip-autocomplete': { overflow: 'visible' },
  '.cm-tooltip.cm-completionInfo': { padding: '0', maxWidth: '380px', marginLeft: '4px' },
  '.cm-error-mark': { textDecoration: 'underline wavy var(--nd-error)', textUnderlineOffset: '3px' },
  '.cm-tooltip': { border: 'none', borderRadius: 'var(--nd-radius-lg)', backgroundColor: 'var(--nd-float)', color: 'var(--nd-ink)', boxShadow: 'var(--nd-float-shadow)', overflow: 'hidden' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--nd-font-mono)', fontSize: '12.5px', maxHeight: '17em', minWidth: '16em', borderRadius: 'inherit' },
  '.cm-tooltip-autocomplete > ul > li': { padding: '3px 10px !important' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--nd-accent)', color: 'var(--nd-accent-ink)' },
  '.cm-completionDetail': { color: 'inherit', opacity: '0.65', fontStyle: 'normal', marginLeft: '1.2em', fontFamily: 'var(--nd-font-ui)', fontSize: '11.5px' },
  '.cm-completionMatchedText': { textDecoration: 'none', fontWeight: '600' },
  '.cm-kind': { display: 'inline-block', width: '1.5em', marginRight: '7px', textAlign: 'center', fontFamily: 'var(--nd-font-mono)', fontSize: '10.5px', fontWeight: '600', color: 'var(--nd-ink-3)', borderRadius: '2px', boxShadow: 'inset 0 0 0 1px color-mix(in srgb, currentColor 35%, transparent)' },
  '.cm-kind-keyword': { boxShadow: 'none' },
  '.cm-kind-table, .cm-kind-view, .cm-kind-cte': { color: 'var(--nd-accent)' },
  '.cm-kind-column': { color: 'var(--nd-syn-string)' },
  '.cm-kind-function, .cm-kind-type, .cm-kind-snippet': { color: 'var(--nd-syn-function)' },
  '.cm-kind-join': { color: 'var(--nd-ok)' },
  '.cm-kind-schema, .cm-kind-database, .cm-kind-alias': { color: 'var(--nd-syn-keyword)' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected] .cm-kind': { color: 'inherit' },
  '.cm-unknown': { textDecoration: 'underline dotted var(--nd-warn)', textDecorationThickness: '1.5px', textUnderlineOffset: '3px' },
  '.cm-unknown-tip': { padding: '7px 11px 8px', maxWidth: '340px', fontFamily: 'var(--nd-font-ui)', fontSize: '12px', lineHeight: '1.45' },
  '.cm-unknown-head': { color: 'var(--nd-ink)', fontWeight: '600' },
  '.cm-unknown-note': { color: 'var(--nd-ink-3)', marginTop: '2px' },
  '.cm-snippetField': { backgroundColor: 'color-mix(in srgb, var(--nd-accent) 12%, transparent)', borderRadius: '2px', boxShadow: 'inset 0 -1px 0 color-mix(in srgb, var(--nd-accent) 60%, transparent)' },
  '.cm-snippetFieldPosition': { borderLeft: '1.5px dotted var(--nd-accent)' },
  '.cm-cursor-secondary': { borderLeftColor: 'var(--nd-accent)', borderLeftWidth: '2px' },
  '.cm-panels': { backgroundColor: 'var(--nd-surface-2)', color: 'var(--nd-ink)', borderColor: 'var(--nd-rule)' },
  '.cm-panel input, .cm-panel button': { font: 'inherit', fontSize: '12px' },
  '.cm-panel.cm-search': { padding: '4px 34px 4px 10px' },
  '.cm-panel.cm-search > *': { margin: '3px 6px 3px 0', verticalAlign: 'middle' },
  '.cm-panel.cm-search label': { display: 'inline-flex', alignItems: 'center', gap: '5px', marginRight: '12px', fontSize: '12px', color: 'var(--nd-ink-2)' },
  '.cm-panel.cm-search input[type=checkbox]': { margin: '0' },
  '.cm-textfield': { width: '220px', height: 'var(--nd-ctl-h)', padding: '0 8px', border: 'none', borderRadius: 'var(--nd-radius)', backgroundColor: 'var(--nd-well)', color: 'var(--nd-ink)', boxShadow: 'var(--nd-well-edge)', outline: 'none' },
  '.cm-textfield:focus': { boxShadow: 'var(--nd-well-edge-focus)' },
  '.cm-button': { height: 'var(--nd-ctl-h)', padding: '0 10px', border: 'none', borderRadius: 'var(--nd-radius)', backgroundImage: 'none', backgroundColor: 'var(--nd-raised)', color: 'var(--nd-ink)', boxShadow: 'inset 0 0 0 1px var(--nd-rule)', cursor: 'pointer' },
  '.cm-button:hover': { backgroundColor: 'var(--nd-hover)', boxShadow: 'inset 0 0 0 1px var(--nd-ink-3)' },
  '.cm-button:active': { backgroundImage: 'none', transform: 'translateY(0.5px)' },
  '.cm-panel.cm-search [name=close]': { top: '7px', right: '8px', width: '22px', height: '22px', padding: '0', borderRadius: 'var(--nd-radius)', color: 'var(--nd-ink-2)', fontSize: '15px', lineHeight: '20px' },
  '.cm-panel.cm-search [name=close]:hover': { color: 'var(--nd-ink)', backgroundColor: 'var(--nd-hover)' },
});

const fieldTheme = EditorView.theme({
  '&': { height: 'auto', minHeight: '78px', maxHeight: '190px' },
  '.cm-scroller': { fontSize: '12.5px', lineHeight: '19px', overflow: 'auto' },
  '.cm-content': { padding: '7px 10px' },
  '.cm-placeholder': { color: 'var(--nd-ink-3)', fontStyle: 'normal' },
  '&.cm-focused': { outline: 'none' },
});

export function fieldExtensions(dialect: Dialect | null, label: string, hint: string, onChange: (text: string) => void): Extension[] {
  return [
    history(),
    drawSelection(),
    bracketMatching(),
    closeBrackets(),
    languageFor(dialect, null, {}, false),
    syntaxHighlighting(highlight),
    theme,
    fieldTheme,
    placeholder(hint),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({ 'aria-label': label, 'aria-multiline': 'true' }),
    keymap.of([...closeBracketsKeymap, ...historyKeymap, ...defaultKeymap]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) onChange(u.state.doc.toString());
    }),
  ];
}

export async function formatSql(text: string, dialect: Dialect, options: Pick<Preferences['editor'], 'uppercase_keywords' | 'tab_width'> = { uppercase_keywords: true, tab_width: 2 }): Promise<string> {
  const { format } = await import('sql-formatter');
  return format(text, { language: FORMAT_LANGUAGE[dialect], keywordCase: options.uppercase_keywords ? 'upper' : 'preserve', tabWidth: options.tab_width });
}

export function lookOf(p: Preferences['editor']): Extension {
  return [
    EditorView.theme({ '&': { fontSize: `${p.font_size}px` }, '.cm-scroller': { lineHeight: `${Math.round(p.font_size * 1.55)}px` } }),
    EditorState.tabSize.of(p.tab_width),
    indentUnit.of(' '.repeat(p.tab_width)),
  ];
}

const COMPLETION_DELAY_MS = 25;

export const completionOf = (p: Preferences['editor']): Extension => (p.autocomplete ? autocompletion({ activateOnTyping: true, activateOnTypingDelay: COMPLETION_DELAY_MS, icons: false, addToOptions: [kindColumn], maxRenderedOptions: 80 }) : []);

export function snippetsOf(list: readonly ReadySnippet[]): Extension {
  const options = list.map((s) => snippetCompletion(s.body, { label: s.trigger, detail: s.name, type: 'snippet', boost: -2 }));
  const source = (ctx: CompletionContext): CompletionResult | null => {
    const word = ctx.matchBefore(/\w+/);
    if (!word && !ctx.explicit) return null;
    if (ctx.state.sliceDoc((word?.from ?? ctx.pos) - 1, word?.from ?? ctx.pos) === '.') return null;
    return { from: word ? word.from : ctx.pos, options, validFor: /^\w*$/ };
  };
  return EditorState.languageData.of(() => [{ autocomplete: source }]);
}

export function keysOf(b: Bindings, h: EditorHandlers): Extension {
  const run = (key: string, f: () => unknown) => (key ? [{ key: forEditor(key), run: (view: EditorView) => (closeCompletion(view), f(), true) }] : []);
  return keymap.of([
    ...run(b.run_statement, h.onRunStatement),
    ...run(b.run_script, h.onRunScript),
    ...(h.onDryRun ? run(b.dry_run, h.onDryRun) : []),
    ...(b.cancel ? [{ key: forEditor(b.cancel), run: () => h.onCancel() }] : []),
    ...run(b.format, h.onFormat),
    ...(b.rename_alias ? [{ key: forEditor(b.rename_alias), run: renameAlias }] : []),
  ]);
}

export interface EditorHandlers {
  onChange: (text: string) => void;
  onRunStatement: () => void;
  onRunScript: () => void;
  onDryRun?: () => void;
  onCancel: () => boolean;
  onFormat: () => void;
  onSelect?: () => void;
}

export interface EditorSetup {
  prefs: Preferences['editor'];
  product?: string;
  bindings: Bindings;
  snippets: readonly ReadySnippet[];
  native?: NativeLanguage;
  keys?: KeyLookup | null;
}

export function extensions(dialect: Dialect | null, schema: Schema | null, h: EditorHandlers, setup: EditorSetup): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    history(),
    drawSelection(),
    EditorState.allowMultipleSelections.of(true),
    rectangularSelection(),
    crosshairCursor(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    completion.of(completionOf(setup.prefs)),
    highlightActiveLine(),
    highlightSelectionMatches(),
    language.of(languageFor(dialect, schema, { product: setup.product, upper: setup.prefs.uppercase_keywords, native: setup.native, keys: setup.keys })),
    inspection.of(dialect ? inspections(dialect, schema, setup.prefs.inspections) : []),
    recency,
    syntaxHighlighting(highlight),
    theme,
    marks,
    paramSpots,
    look.of(lookOf(setup.prefs)),
    snippetSource.of(snippetsOf(setup.snippets)),
    keyboard.of(keysOf(setup.bindings, h)),
    keymap.of([
      { key: 'Mod-Alt-ArrowUp', run: addCursor(false) },
      { key: 'Mod-Alt-ArrowDown', run: addCursor(true) },
      ...closeBracketsKeymap,
      ...completionKeymap,
      ...searchKeymap,
      ...historyKeymap,
      indentWithTab,
      ...defaultKeymap,
    ]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) h.onChange(u.state.doc.toString());
      if (u.docChanged || u.selectionSet) h.onSelect?.();
    }),
    phrases(),
  ];
}

const phrases = (): Extension =>
  EditorState.phrases.of({
    Find: m.cm_find(),
    Replace: m.cm_replace(),
    next: m.cm_next(),
    previous: m.cm_previous(),
    all: m.cm_all(),
    'match case': m.cm_match_case(),
    regexp: m.cm_regexp(),
    'by word': m.cm_by_word(),
    replace: m.cm_replace_one(),
    'replace all': m.cm_replace_all(),
    close: m.cm_close(),
    'current match': m.cm_current(),
    'on line': m.cm_on_line(),
    'replaced $ matches': m.cm_replaced_n(),
    'replaced match on line $': m.cm_replaced_line(),
    'Go to line': m.cm_goto(),
    go: m.cm_go(),
  });

export function viewerExtensions(dialect: Dialect | null, prefs: Preferences['editor']): Extension[] {
  return [lineNumbers(), drawSelection(), highlightSelectionMatches(), EditorState.readOnly.of(true), language.of(languageFor(dialect, null, {}, false)), syntaxHighlighting(highlight), theme, look.of(lookOf(prefs)), keymap.of([...searchKeymap, ...defaultKeymap]), phrases()];
}
