import type { FakeRequest, GenDone, GenPreview, GenProfile, Progress, Schema } from './api';
import type { Check_Serialize as Check, ColumnGen_Serialize as ColumnGen, DataLocale, FakeColumn_Serialize as FakeColumn, GenRule, Gen_Serialize as Gen, Shape_Serialize as Shape } from './bindings';
import { derivedValue, englishValue, spread, weightedPick } from './mock-gen-profiles';

const FIRST = ['Giulia', 'Marco', 'Chiara', 'Luca', 'Sara', 'Paolo', 'Elena', 'Davide', 'Marta', 'Stefano'];
const LAST = ['Rossi', 'Colombo', 'Ferrari', 'Bianchi', 'Gatti', 'Locatelli', 'Bonomi', 'Pedrali'];
const CITIES: [string, string, string][] = [['Bergamo', 'BG', '24121'], ['Brescia', 'BS', '25121'], ['Milano', 'MI', '20121'], ['Cremona', 'CR', '26100'], ['Lecco', 'LC', '23900']];
const WORDS = ['ordine', 'consegna', 'cliente', 'magazzino', 'fattura', 'sconto', 'pallet', 'urgente', 'verifica', 'nota'];
const FIRMS = ['Officine Orione S.r.l.', 'Termoidraulica Gatti S.n.c.', 'Vela Logistica S.p.A.', 'Rossi & Bonomi S.a.s.', 'Ceramiche Levante S.r.l.'];

function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function shapeOf(type: string): Shape {
  const t = type.toLowerCase();
  if (/^int|serial|bigint|smallint/.test(t)) return { kind: 'int', min: -2147483648, max: 2147483647 };
  if (/^(numeric|decimal)/.test(t)) return { kind: 'decimal', precision: 12, scale: 2 };
  if (/^bool/.test(t)) return { kind: 'bool' };
  if (/^date$/.test(t)) return { kind: 'date' };
  if (/^timestamp/.test(t)) return { kind: 'date_time' };
  if (/^jsonb?$/.test(t)) return { kind: 'json' };
  if (/^bytea/.test(t)) return { kind: 'binary' };
  return { kind: 'text', max: t === 'bpchar' ? 11 : null };
}

function guess(table: string, name: string, shape: Shape, pk: boolean, check: Check | null): Gen {
  if (check?.kind === 'values') return { kind: 'pick', values: check.values, weights: [] };
  if (pk && shape.kind === 'int') return { kind: 'sequence', start: 2401 };
  if (pk) return { kind: 'code', len: 8 };
  if (/ragione_sociale|azienda/.test(name)) return { kind: 'company' };
  if (/partita_iva|piva/.test(name)) return { kind: 'partita_iva' };
  if (/citta/.test(name)) return { kind: 'city' };
  if (/provincia/.test(name)) return { kind: 'province' };
  if (/email/.test(name)) return { kind: 'email' };
  if (/^nome$/.test(name)) return table === 'clienti' ? { kind: 'company' } : { kind: 'first_name' };
  if (/scadenza/.test(name)) return { kind: 'date', from: '2025-01-01', to: '2027-12-31' };
  if (/sconto/.test(name)) return { kind: 'decimal', min: 0, max: 50, scale: 2 };
  if (/quantita|qta/.test(name)) return { kind: 'integer', min: 1, max: 100 };
  switch (shape.kind) {
    case 'int':
      return { kind: 'integer', min: 1, max: 1000 };
    case 'decimal':
      return { kind: 'decimal', min: 1, max: 2000, scale: shape.scale };
    case 'bool':
      return { kind: 'boolean' };
    case 'date':
      return { kind: 'date', from: '2022-01-01', to: '2026-06-30' };
    case 'date_time':
      return { kind: 'date_time', from: '2022-01-01', to: '2026-06-30' };
    case 'json':
      return { kind: 'json' };
    case 'binary':
      return { kind: 'null' };
    default:
      return { kind: 'lorem', min_words: 2, max_words: 6 };
  }
}

const LINKS: Record<string, { column: string; table: string }> = {
  fatture: { column: 'cliente_id', table: 'clienti' },
  ordini: { column: 'cliente_id', table: 'clienti' },
};
const CHECKS: Record<string, Check> = {
  'righe_ordine.sconto': { kind: 'range', min: 0, max: 50, min_open: false, max_open: false },
  'fatture.numero': { kind: 'values', values: [] },
};

const RULES: Record<string, GenRule[]> = {
  righe_ordine: [
    { expr: 'CHECK ((sconto >= 0) AND (sconto <= 50))', kind: 'column' },
    { expr: 'CHECK ((sconto <= prezzo))', kind: 'row' },
    { expr: 'CHECK (prezzo_valido(prezzo, quantita))', kind: 'server' },
  ],
  fatture: [{ expr: 'CHECK ((scadenza >= data))', kind: 'row' }],
};

function profileOf(schema: Schema, namespace: string, name: string): GenProfile {
  const rel = schema.namespaces.find((n) => n.name === namespace)?.tables.find((t) => t.name === name);
  if (!rel) throw new Error(`not found: ${name}`);
  const link = LINKS[name];
  const parentRows = link ? (schema.namespaces[0]?.tables.find((t) => t.name === link.table)?.estimated_rows ?? 0) : 0;
  const columns: FakeColumn[] = rel.columns.map((c) => {
    const shape = shapeOf(c.type_name);
    const raw = CHECKS[`${name}.${c.name}`] ?? null;
    const check = raw?.kind === 'values' && !raw.values.length ? null : raw;
    const parent = link?.column === c.name ? { fk: 0, namespace, table: link.table, column: 'id' } : null;
    const identity = c.primary_key && name === 'clienti';
    const fallback = name === 'fatture' && c.name === 'data' ? 'CURRENT_DATE' : null;
    const generator: Gen = identity || fallback ? { kind: 'auto' } : parent ? { kind: 'parent', fk: 0, column: 0 } : guess(name, c.name, shape, c.primary_key, check);
    const suggested: ColumnGen = { name: c.name, generator, null_ratio: c.nullable && !parent && generator.kind !== 'auto' && generator.kind !== 'null' ? 0.1 : 0, max_len: shape.kind === 'text' ? shape.max : null, locale: null, dist: { kind: 'uniform' } };
    return {
      name: c.name,
      type_name: c.type_name,
      nullable: c.nullable,
      primary_key: c.primary_key,
      unique: c.primary_key,
      auto: identity ? 'identity' : null,
      default: fallback,
      check,
      parent,
      shape,
      suggested,
    };
  });
  return {
    namespace,
    name,
    columns,
    foreign_keys: link ? [{ namespace, table: link.table, columns: [link.column], ref_columns: ['id'], parent_rows: parentRows, own: false }] : [],
    unique: [[rel.columns[0]?.name ?? 'id']],
    ancestors: link ? [{ namespace, name: link.table }] : [],
    rules: RULES[name] ?? [],
    estimated_rows: rel.estimated_rows,
  };
}

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

function piva(r: () => number): string {
  const d = Array.from({ length: 7 }, () => Math.floor(r() * 10)).concat([0, 1, 6]);
  const sum = d.reduce((s, v, i) => s + (i % 2 === 0 ? v : v * 2 > 9 ? v * 2 - 9 : v * 2), 0);
  return d.join('') + String((10 - (sum % 10)) % 10);
}

function value(c: ColumnGen, locale: DataLocale, r: () => number, row: number, pick: <T>(a: readonly T[]) => T, city: [string, string, string], person: number): string | null {
  const g = c.generator;
  const en = englishValue(g, c.locale ?? locale, r, person);
  if (en !== undefined) return en;
  const between = (a: number, b: number) => a + Math.floor(r() * (b - a + 1));
  const day = (from: string, to: string) => {
    const a = Date.parse(from);
    const b = Date.parse(to);
    return new Date(a + Math.floor(r() * Math.max(1, (b - a) / 86400000)) * 86400000).toISOString().slice(0, 10);
  };
  switch (g.kind) {
    case 'auto':
    case 'null':
      return null;
    case 'fixed':
      return g.value;
    case 'first_name':
      return pick(FIRST);
    case 'last_name':
      return pick(LAST);
    case 'full_name':
      return `${pick(FIRST)} ${pick(LAST)}`;
    case 'company':
      return pick(FIRMS);
    case 'email':
      return `${pick(FIRST).toLowerCase()}.${pick(LAST).toLowerCase()}@example.com`;
    case 'phone':
      return `+39 3${between(20, 99)} ${between(100, 999)} ${between(1000, 9999)}`;
    case 'street':
      return `Via ${pick(['Roma', 'Garibaldi', 'Mazzini', 'dei Mille'])}, ${between(1, 120)}`;
    case 'city':
      return city[0];
    case 'province':
      return city[1];
    case 'postal_code':
      return city[2];
    case 'birth_date':
      return day('1950-01-01', '2004-12-31');
    case 'codice_fiscale':
      return `RSSMRA${between(50, 99)}A${pad(between(1, 28))}H501${'ABCDEFGHIJ'[between(0, 9)]}`;
    case 'partita_iva':
      return piva(r);
    case 'integer':
      return String(spread(r, c.dist, g.min ?? 0, g.max ?? 100, row));
    case 'sequence':
      return String((g.start ?? 1) + row);
    case 'decimal':
      return (spread(r, c.dist, Math.round((g.min ?? 0) * 100), Math.round((g.max ?? 100) * 100), row) / 100).toFixed(g.scale);
    case 'boolean':
      return r() < 0.5 ? 'true' : 'false';
    case 'date':
      if (c.dist?.kind === 'sequential') return new Date(Date.parse(g.from) + spread(r, c.dist, 0, Math.max(0, (Date.parse(g.to) - Date.parse(g.from)) / 86400000), row) * 86400000).toISOString().slice(0, 10);
      return day(g.from, g.to);
    case 'date_time':
      return `${day(g.from, g.to)} ${pad(between(7, 19))}:${pad(between(0, 59))}:${pad(between(0, 59))}`;
    case 'time':
      return `${pad(between(7, 19))}:${pad(between(0, 59))}:00`;
    case 'lorem': {
      const n = between(g.min_words, Math.max(g.min_words, g.max_words));
      const words = Array.from({ length: n }, () => pick(WORDS)).join(' ');
      return words.charAt(0).toUpperCase() + words.slice(1);
    }
    case 'code':
      return Array.from({ length: g.len }, () => 'ABCDEFGHJKLMNPQRSTUVWXYZ0123456789'[between(0, 33)]).join('');
    case 'uuid':
      return `${between(0x10000000, 0xffffffff).toString(16)}-${between(0x1000, 0xffff).toString(16)}-4${between(0x100, 0xfff).toString(16)}-a${between(0x100, 0xfff).toString(16)}-${between(0x100000, 0xffffff).toString(16)}${between(0x100000, 0xffffff).toString(16)}`;
    case 'json':
      return `{"nota": "${pick(WORDS)}", "valore": ${between(1, 100)}}`;
    case 'pick':
      return weightedPick(r, g.values, g.weights);
    case 'pattern':
    case 'expr':
      return null;
    case 'parent':
      return String(between(1, 2400));
  }
}

function rowsOf(req: FakeRequest, count: number, offset = 0): { columns: string[]; rows: (string | null)[][] } {
  const cols: ColumnGen[] = req.columns
    .filter((c) => c.generator.kind !== 'auto')
    .map((c) => ({ ...c, generator: c.generator.kind === 'pick' ? { ...c.generator, weights: c.generator.weights ?? [] } : c.generator, max_len: c.max_len ?? null, locale: c.locale ?? null, dist: c.dist ?? { kind: 'uniform' } }));
  const rows = Array.from({ length: count }, (_, i) => {
    const r = rng(req.seed * 7919 + i + offset);
    const pick = <T>(a: readonly T[]): T => a[Math.floor(r() * a.length)] as T;
    const city = pick(CITIES);
    const person = Math.floor(r() * 1000);
    const locale = req.locale ?? 'it';
    const row = cols.map((c) => (r() < (c.null_ratio ?? 0) ? null : value(c, locale, r, i + offset, pick, city, person)));
    const named = Object.fromEntries(cols.map((c, j) => [c.name, row[j] ?? null]));
    cols.forEach((c, j) => {
      const v = derivedValue(c.generator, r, i + offset, named);
      if (v !== undefined) named[c.name] = row[j] = v;
    });
    return row;
  });
  return { columns: cols.map((c) => c.name), rows };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function generateMock<T extends { jobCancel: (id: string) => Promise<void> }>(schema: (sid: string) => Promise<Schema>, search: T) {
  const cancel = search.jobCancel;
  const stopped = new Set<string>();
  return {
    ...search,
    genProfile: async (sid: string, namespace: string, name: string): Promise<GenProfile> => profileOf(await schema(sid), namespace, name),
    genPreview: async (req: FakeRequest): Promise<GenPreview> => {
      const { columns, rows } = rowsOf(req, Math.min(20, req.rows));
      return { columns, rows, waiting: [], fault: null };
    },
    genRun: async (req: FakeRequest, onProgress: (p: Progress) => void): Promise<GenDone> => {
      const started = Date.now();
      const p = profileOf(await schema(req.session_id), req.namespace, req.name);
      const steps = [...(req.parents ? p.ancestors.map((a) => ({ name: a.name, rows: req.parents ?? 0 })) : []), { name: req.name, rows: req.rows }];
      const total = steps.reduce((s, x) => s + x.rows, 0);
      const slow = typeof location !== 'undefined' && new URLSearchParams(location.search).has('slow');
      let done = 0;
      for (const s of steps) {
        for (let at = 0; at < s.rows; at += 1000) {
          if (stopped.delete(req.job_id)) return { tables: [], inserted: 0, committed: false, kept: false, cancelled: true, fault: null, elapsed_ms: Date.now() - started };
          done += Math.min(1000, s.rows - at);
          onProgress({ done, total, unit: 'rows', rows: done, bytes: 0, stage: s.name });
          await sleep(slow ? 400 : 5);
        }
      }
      const broken = req.columns.find((c) => c.generator.kind === 'fixed' && c.generator.value === '');
      if (broken) {
        return { tables: [], inserted: 0, committed: false, kept: false, cancelled: false, fault: { kind: 'row', table: req.name, row: 1, message: `null value in column "${broken.name}" violates not-null constraint` }, elapsed_ms: Date.now() - started };
      }
      return { tables: steps.map((s) => ({ namespace: req.namespace, name: s.name, rows: s.rows })), inserted: total, committed: true, kept: false, cancelled: false, fault: null, elapsed_ms: 840 };
    },
    jobCancel: async (id: string) => {
      stopped.add(id);
      await cancel(id);
    },
  };
}
