import { m } from '../paraglide/messages';
import type { GenSaved } from './api';
import type { ColumnGen_Serialize as ColumnGen, DataLocale, Dist, FakeColumn_Serialize as FakeColumn, Gen_Serialize as Gen, GenFault } from './bindings';

export type GenKind = Gen['kind'];

export const ROWS_MAX = 1_000_000;
export const ROW_CHOICES = [10, 100, 1000, 10_000] as const;
export const PARENT_ROWS = 20;

export const GROUPS: { label: () => string; kinds: GenKind[] }[] = [
  { label: m.gen_group_people, kinds: ['first_name', 'last_name', 'full_name', 'email', 'phone', 'codice_fiscale', 'birth_date'] },
  { label: m.gen_group_firms, kinds: ['company', 'partita_iva'] },
  { label: m.gen_group_places, kinds: ['street', 'city', 'province', 'postal_code'] },
  { label: m.gen_group_numbers, kinds: ['integer', 'sequence', 'decimal', 'boolean'] },
  { label: m.gen_group_dates, kinds: ['date', 'date_time', 'time'] },
  { label: m.gen_group_text, kinds: ['lorem', 'code', 'uuid', 'json', 'pick', 'fixed'] },
  { label: m.gen_group_row, kinds: ['pattern', 'expr'] },
  { label: m.gen_group_other, kinds: ['parent', 'auto', 'null'] },
];

export const KIND_LABEL: Record<GenKind, () => string> = {
  auto: m.gen_kind_auto,
  null: m.gen_kind_null,
  fixed: m.gen_kind_fixed,
  first_name: m.gen_kind_first_name,
  last_name: m.gen_kind_last_name,
  full_name: m.gen_kind_full_name,
  company: m.gen_kind_company,
  email: m.gen_kind_email,
  phone: m.gen_kind_phone,
  street: m.gen_kind_street,
  city: m.gen_kind_city,
  province: m.gen_kind_province,
  postal_code: m.gen_kind_postal_code,
  birth_date: m.gen_kind_birth_date,
  codice_fiscale: m.gen_kind_codice_fiscale,
  partita_iva: m.gen_kind_partita_iva,
  integer: m.gen_kind_integer,
  sequence: m.gen_kind_sequence,
  decimal: m.gen_kind_decimal,
  boolean: m.gen_kind_boolean,
  date: m.gen_kind_date,
  date_time: m.gen_kind_date_time,
  time: m.gen_kind_time,
  lorem: m.gen_kind_lorem,
  code: m.gen_kind_code,
  uuid: m.gen_kind_uuid,
  json: m.gen_kind_json,
  pick: m.gen_kind_pick,
  parent: m.gen_kind_parent,
  pattern: m.gen_kind_pattern,
  expr: m.gen_kind_expr,
};

export function kindsFor(c: FakeColumn): { label: () => string; kinds: GenKind[] }[] {
  return GROUPS.map((g) => ({
    label: g.label,
    kinds: g.kinds.filter((k) => {
      if (k === 'parent') return !!c.parent;
      if (k === 'auto') return c.auto !== null || c.default !== null || c.nullable;
      if (k === 'null') return c.nullable;
      return true;
    }),
  })).filter((g) => g.kinds.length);
}

export const locked = (c: FakeColumn) => c.auto !== null;

export function switchKind(c: FakeColumn, from: Gen, kind: GenKind): Gen {
  const s = c.suggested.generator;
  if (s.kind === kind) return structuredClone(s);
  const shape = c.shape;
  switch (kind) {
    case 'integer': {
      const [min, max] = shape.kind === 'int' ? [Math.max(shape.min ?? 1, 1), Math.min(shape.max ?? 1000, 1000)] : [1, 1000];
      return { kind, min, max: Math.max(min, max) };
    }
    case 'sequence':
      return { kind, start: 1 };
    case 'decimal':
      return { kind, min: 0, max: 1000, scale: shape.kind === 'decimal' ? shape.scale : 2 };
    case 'date':
    case 'date_time':
      return from.kind === 'date' || from.kind === 'date_time' ? { kind, from: from.from, to: from.to } : { kind, from: '2022-01-01', to: '2026-06-30' };
    case 'lorem':
      return { kind, min_words: 2, max_words: 8 };
    case 'code':
      return { kind, len: Math.min(shape.kind === 'text' && shape.max ? shape.max : 8, 10) };
    case 'pick':
      return { kind, values: c.check?.kind === 'values' ? [...c.check.values] : shape.kind === 'enum' ? [...shape.values] : from.kind === 'pick' ? from.values : [], weights: from.kind === 'pick' ? from.weights : [] };
    case 'pattern':
      return { kind, pattern: from.kind === 'pattern' ? from.pattern : shape.kind === 'text' && shape.max !== null && shape.max < 13 ? '???-####' : 'AB-{year}-{0000}' };
    case 'expr':
      return { kind, expr: from.kind === 'expr' ? from.expr : '', scale: shape.kind === 'decimal' ? shape.scale : shape.kind === 'int' ? 0 : 2 };
    case 'fixed':
      return { kind, value: '' };
    case 'parent':
      return c.parent ? { kind, fk: c.parent.fk, column: s.kind === 'parent' ? s.column : 0 } : { kind: 'null' };
    default:
      return { kind } as Gen;
  }
}

export const ranged = (g: Gen) => g.kind === 'integer' || g.kind === 'decimal' || g.kind === 'date' || g.kind === 'date_time';

export const localized = (g: Gen) => ['first_name', 'last_name', 'full_name', 'company', 'email', 'phone', 'street', 'city', 'province', 'postal_code', 'birth_date', 'lorem', 'json'].includes(g.kind);

export const LOCALES: { id: DataLocale; label: () => string; short: string }[] = [
  { id: 'it', label: m.gen_locale_it, short: 'IT' },
  { id: 'uk', label: m.gen_locale_uk, short: 'UK' },
  { id: 'us', label: m.gen_locale_us, short: 'US' },
];

export type DistKind = Dist['kind'];

export const DISTS: { id: DistKind; label: () => string }[] = [
  { id: 'uniform', label: m.gen_dist_uniform },
  { id: 'normal', label: m.gen_dist_normal },
  { id: 'low', label: m.gen_dist_low },
  { id: 'high', label: m.gen_dist_high },
  { id: 'sequential', label: m.gen_dist_sequential },
];

export const distOf = (kind: DistKind, step = 1): Dist => (kind === 'sequential' ? { kind, step } : { kind });

export const stepUnit = (g: Gen) => (g.kind === 'date' ? m.gen_step_days() : g.kind === 'date_time' ? m.gen_step_hours() : m.gen_step_units());

export const provinceLabel = (l: DataLocale) => (l === 'it' ? m.gen_kind_province() : l === 'uk' ? m.gen_kind_county() : m.gen_kind_state());

const WEIGHT = /\*\s*(\d+(?:[.,]\d+)?)\s*$/;

export function parseWeighted(text: string): { values: string[]; weights: number[] } {
  const values: string[] = [];
  const weights: number[] = [];
  for (const raw of parseList(text)) {
    const w = WEIGHT.exec(raw);
    const value = w ? raw.slice(0, w.index).trim() : raw;
    if (!value) continue;
    values.push(value);
    weights.push(w ? Number((w[1] ?? '1').replace(',', '.')) : 1);
  }
  return { values, weights: weights.some((w) => w !== 1) ? weights : [] };
}

export const weightedText = (values: readonly string[], weights: readonly (number | null)[]) => values.map((v, i) => (weights.length && (weights[i] ?? 1) !== 1 ? `${v}*${weights[i] ?? 1}` : v)).join(', ');

export function profilesFor(all: readonly GenSaved[], table: { connectionId: string; namespace: string; name: string }) {
  const own = (p: GenSaved) => p.connection_id === table.connectionId && p.namespace === table.namespace && p.table === table.name;
  return { own: all.filter(own), other: all.filter((p) => !own(p)) };
}

export function applyProfile(columns: readonly FakeColumn[], saved: GenSaved): Record<string, ColumnGen> {
  const byName = new Map(saved.columns.map((c) => [c.name.toLowerCase(), c]));
  return Object.fromEntries(
    columns.map((c) => {
      const s = byName.get(c.name.toLowerCase());
      if (!s || locked(c) || (s.generator.kind === 'parent' && !c.parent)) return [c.name, structuredClone(c.suggested)];
      return [c.name, { ...structuredClone(s), name: c.name, max_len: c.suggested.max_len }];
    }),
  );
}

export const parseList = (text: string) =>
  text
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean);

export const percent = (ratio: number | null | undefined) => Math.round((ratio ?? 0) * 100);

export const ratio = (pct: number) => Math.min(100, Math.max(0, Number.isFinite(pct) ? pct : 0)) / 100;

export function columnNote(c: FakeColumn): string | null {
  if (c.auto === 'identity') return m.gen_note_identity();
  if (c.auto === 'computed') return m.gen_note_computed();
  if (c.parent) return m.gen_note_parent({ table: c.parent.table, column: c.parent.column });
  if (c.check?.kind === 'values') return m.gen_note_check_values({ n: c.check.values.length, count: String(c.check.values.length) });
  if (c.check?.kind === 'range') return m.gen_note_check_range({ range: rangeText(c.check.min, c.check.max, c.check.min_open, c.check.max_open) });
  if (c.default !== null) return m.gen_note_default({ value: c.default.length > 28 ? `${c.default.slice(0, 28)}…` : c.default });
  return null;
}

function rangeText(min: number | null, max: number | null, minOpen: boolean, maxOpen: boolean): string {
  const lo = min === null ? null : `${minOpen ? '>' : '≥'} ${min}`;
  const hi = max === null ? null : `${maxOpen ? '<' : '≤'} ${max}`;
  return [lo, hi].filter(Boolean).join(', ');
}

export function recipe(columns: readonly FakeColumn[], chosen: Readonly<Record<string, ColumnGen>>): ColumnGen[] {
  return columns.map((c) => (locked(c) ? c.suggested : (chosen[c.name] ?? c.suggested)));
}

export function faultText(f: GenFault): string {
  switch (f.kind) {
    case 'empty_parent':
      return m.gen_fault_empty_parent({ table: f.table });
    case 'no_unique':
      return m.gen_fault_no_unique({ columns: f.columns.join(', ') });
    case 'row':
      return m.gen_fault_row({ table: f.table, row: String(f.row ?? 0), message: f.message });
    case 'batch':
      return m.gen_fault_batch({ table: f.table, from: String(f.from ?? 0), to: String(f.to ?? 0), message: f.message });
    case 'expr':
      return m.gen_fault_expr({ column: f.column, message: f.message });
    case 'check':
      return m.gen_fault_check({ expr: f.expr });
    case 'server':
      return f.message;
  }
}

export const newSeed = (random = Math.random) => 10_000 + Math.floor(random() * 90_000);

let seed: { namespace: string; name: string } | null = null;

export const seedGenerate = (namespace: string, name: string) => (seed = { namespace, name });

export function takeGenerateSeed(): { namespace: string; name: string } | null {
  const s = seed;
  seed = null;
  return s;
}

export function generateCommand(tab: { view: string; tool: string }, table: () => { namespace: string; name: string; view: boolean } | null) {
  return [
    {
      id: 'data.generate',
      label: () => m.pal_cmd_generate_table({ table: table()?.name ?? '' }),
      group: m.pal_group_tools,
      when: () => tab.view === 'data' && !!table() && !table()?.view,
      run: () => {
        const t = table();
        if (!t) return;
        seedGenerate(t.namespace, t.name);
        tab.view = 'tools';
        tab.tool = 'generate';
      },
    },
  ];
}
