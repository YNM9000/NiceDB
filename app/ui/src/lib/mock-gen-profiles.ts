import type { GenProfiles, GenSaved } from './api';
import type { DataLocale, Dist, Gen_Serialize as Gen } from './bindings';

const FIRST = ['Oliver', 'Amelia', 'George', 'Isla', 'Harry', 'Ava', 'Jack', 'Grace', 'Noah', 'Emily'];
const LAST: Record<'uk' | 'us', string[]> = {
  uk: ['Smith', 'Jones', 'Taylor', 'Brown', 'Davies', 'Evans', 'Wright', 'Hughes'],
  us: ['Johnson', 'Garcia', 'Miller', 'Martinez', 'Anderson', 'Clark', 'Nguyen', 'Rivera'],
};
const UK: [string, string, string][] = [['Manchester', 'Greater Manchester', 'M'], ['Leeds', 'West Yorkshire', 'LS'], ['Bristol', 'Bristol', 'BS'], ['York', 'North Yorkshire', 'YO'], ['Oxford', 'Oxfordshire', 'OX']];
const US: [string, string, string, string][] = [['Boston', 'MA', '021', '617'], ['Austin', 'TX', '787', '512'], ['Seattle', 'WA', '981', '206'], ['Denver', 'CO', '802', '303'], ['Chicago', 'IL', '606', '312']];
const FIRMS: Record<'uk' | 'us', string[]> = {
  uk: ['Kestrel Logistics Ltd', 'Harbour Joinery Ltd', 'Taylor & Hughes LLP', 'Oakfield Foods PLC'],
  us: ['Summit Analytics Inc.', 'Redwood Supplies LLC', 'Garcia & Clark Corp.', 'Pinecrest Software Inc.'],
};
const STREETS = ['Station Road', 'High Street', 'Mill Lane', 'Maple Avenue', 'Church Street', 'Park Drive'];
const WORDS = ['order', 'delivery', 'invoice', 'urgent', 'pallet', 'review', 'pending', 'stock'];
const INWARD = 'ABDEFGHJLNPQRSTUWXYZ';

type Draw = () => number;
const between = (r: Draw, a: number, b: number) => a + Math.floor(r() * (b - a + 1));
const pickOf = <T>(r: Draw, a: readonly T[]): T => a[Math.floor(r() * a.length)] as T;
const plain = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, '');

export function englishValue(g: Gen, locale: DataLocale, r: Draw, person: number): string | undefined {
  if (locale === 'it') return undefined;
  const first = FIRST[person % FIRST.length] ?? 'Oliver';
  const last = LAST[locale][person % LAST[locale].length] ?? 'Smith';
  const uk = UK[person % UK.length] ?? UK[0]!;
  const us = US[person % US.length] ?? US[0]!;
  switch (g.kind) {
    case 'first_name':
      return first;
    case 'last_name':
      return last;
    case 'full_name':
      return `${first} ${last}`;
    case 'email':
      return `${plain(first)}.${plain(last)}@example.com`;
    case 'company':
      return pickOf(r, FIRMS[locale]);
    case 'phone':
      return locale === 'uk' ? `+44 7700 900${between(r, 100, 999)}` : `+1 (${us[3]}) 555-01${String(between(r, 0, 99)).padStart(2, '0')}`;
    case 'street':
      return `${between(r, 1, locale === 'uk' ? 240 : 9800)} ${pickOf(r, STREETS)}`;
    case 'city':
      return locale === 'uk' ? uk[0] : us[0];
    case 'province':
      return locale === 'uk' ? uk[1] : us[1];
    case 'postal_code':
      return locale === 'uk' ? `${uk[2]}${between(r, 1, 20)} ${between(r, 1, 9)}${pickOf(r, [...INWARD])}${pickOf(r, [...INWARD])}` : `${us[2]}${String(between(r, 0, 99)).padStart(2, '0')}`;
    case 'lorem': {
      const n = between(r, g.min_words, Math.max(g.min_words, g.max_words));
      const text = Array.from({ length: n }, () => pickOf(r, WORDS)).join(' ');
      return text.charAt(0).toUpperCase() + text.slice(1);
    }
    default:
      return undefined;
  }
}

export function weightedPick(r: Draw, values: readonly string[], weights: readonly (number | null)[]): string | null {
  if (!values.length) return null;
  const total = values.reduce((s, _, i) => s + Math.max(0, weights[i] ?? 1), 0);
  if (!weights.length || total <= 0) return pickOf(r, values);
  let at = r() * total;
  for (const [i, v] of values.entries()) {
    at -= Math.max(0, weights[i] ?? 1);
    if (at < 0) return v;
  }
  return values.at(-1) ?? null;
}

export function spread(r: Draw, dist: Dist | undefined, lo: number, hi: number, row: number): number {
  const span = hi - lo + 1;
  switch (dist?.kind) {
    case 'sequential':
      return lo + (((row * (dist.step ?? 1)) % span) + span) % span;
    case 'normal': {
      const u = (r() + r() + r() + r() + r() + r()) / 6;
      return lo + Math.floor(u * span);
    }
    case 'low':
      return lo + Math.floor(r() ** 3 * span);
    case 'high':
      return lo + Math.floor((1 - r() ** 3) * span);
    default:
      return lo + Math.floor(r() * span);
  }
}

const FILTERS: Record<string, (v: string) => string> = {
  lower: (v) => v.toLowerCase(),
  upper: (v) => v.toUpperCase(),
  initial: (v) => v.charAt(0),
  slug: plain,
  year: (v) => v.slice(0, 4),
  month: (v) => v.slice(5, 7),
  day: (v) => v.slice(8, 10),
  trim: (v) => v.trim(),
};

export function derivedValue(g: Gen, r: Draw, row: number, values: Record<string, string | null>, year = 2026): string | null | undefined {
  const find = (name: string) => Object.keys(values).find((k) => k.toLowerCase() === name.trim().toLowerCase());
  if (g.kind === 'pattern') {
    return g.pattern
      .replace(/\{([^}]*)\}|\\(.)|[#?*]/g, (all, inner: string | undefined, esc: string | undefined) => {
        if (esc !== undefined) return esc;
        if (inner === undefined) return all === '#' ? String(between(r, 0, 9)) : all === '?' ? pickOf(r, [...'ABCDEFGHJKLMNPQRSTUVWXYZ']) : pickOf(r, [...'ABCDEFGHJKLMNPQRSTUVWXYZ0123456789']);
        if (/^0+$/.test(inner)) return String(row + 1).padStart(inner.length, '0');
        if (/^(year|anno)$/i.test(inner)) return String(year);
        if (/^(n|row|riga)$/i.test(inner)) return String(row + 1);
        const [head = '', ...filters] = inner.split('|');
        const key = find(head);
        if (!key) return all;
        return filters.reduce((v, f) => FILTERS[f.trim()]?.(v) ?? v, values[key] ?? '');
      });
  }
  if (g.kind === 'expr') {
    const v = calc(g.expr, (name) => {
      const key = find(name);
      return key ? Number(values[key] ?? Number.NaN) : Number.NaN;
    });
    return v === null ? null : v.toFixed(g.scale);
  }
  return undefined;
}

const DEMO: GenSaved[] = [
  {
    id: 'demo-fatture',
    name: 'Fatture dell’anno',
    connection_id: 'demo-pg',
    database: null,
    namespace: 'public',
    table: 'fatture',
    locale: 'it',
    seed: 24_816,
    rows: 500,
    parents: null,
    columns: [{ name: 'numero', generator: { kind: 'pattern', pattern: 'FT-{year}-{0000}' }, null_ratio: 0, max_len: null, locale: null, dist: { kind: 'uniform' } }],
    saved_ms: 0,
  },
];

export function genProfilesMock(seed: boolean) {
  let saved: GenSaved[] = seed ? structuredClone(DEMO) : [];
  const out = (): GenProfiles => ({ profiles: structuredClone(saved) });
  return {
    genProfilesGet: async (): Promise<GenProfiles> => out(),
    genProfileSave: async (p: GenSaved): Promise<GenProfiles> => {
      saved = [structuredClone(p), ...saved.filter((x) => x.id !== p.id)];
      return out();
    },
    genProfileDelete: async (id: string): Promise<GenProfiles> => {
      saved = saved.filter((x) => x.id !== id);
      return out();
    },
  };
}

function calc(text: string, col: (name: string) => number): number | null {
  const tokens = text.match(/\d+(?:\.\d+)?|[A-Za-z_][A-Za-z0-9_]*|[-+*/%(),]/g) ?? [];
  let at = 0;
  const peek = () => tokens[at];
  const sum = (): number => {
    let v = product();
    while (peek() === '+' || peek() === '-') v = tokens[at++] === '+' ? v + product() : v - product();
    return v;
  };
  const product = (): number => {
    let v = unary();
    for (let op = peek(); op === '*' || op === '/' || op === '%'; op = peek()) {
      at++;
      const b = unary();
      v = op === '*' ? v * b : op === '/' ? v / b : v % b;
    }
    return v;
  };
  const unary = (): number => (peek() === '-' ? (at++, -unary()) : atom());
  const atom = (): number => {
    const t = tokens[at++] ?? '';
    if (t === '(') {
      const v = sum();
      at++;
      return v;
    }
    if (/^\d/.test(t)) return Number(t);
    if (peek() === '(') {
      at++;
      const args: number[] = [];
      while (peek() !== ')' && at < tokens.length) {
        args.push(sum());
        if (peek() === ',') at++;
      }
      at++;
      const f = t.toLowerCase();
      const [a = Number.NaN, b = 0] = args;
      if (f === 'round') return Math.round(a * 10 ** b) / 10 ** b;
      if (f === 'min') return Math.min(...args);
      if (f === 'max') return Math.max(...args);
      if (f === 'abs') return Math.abs(a);
      return Number.NaN;
    }
    return col(t);
  };
  const v = sum();
  return Number.isFinite(v) && at >= tokens.length ? v : null;
}
