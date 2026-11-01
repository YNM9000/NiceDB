export interface Column {
  numeric: boolean | null;
  values: readonly (string | null)[];
}

export interface Stats {
  cells: number;
  nulls: number;
  count: number;
  numeric: boolean;
  sum: string | null;
  avg: number | null;
  min: string | null;
  max: string | null;
  distinct: number | null;
  scale: number;
}

const DECIMAL = /^[-+]?\d+(\.\d+)?$/;
const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
const MAX_EXACT_SCALE = 18;

const scaleOf = (v: string) => {
  const dot = v.indexOf('.');
  return dot < 0 ? 0 : v.length - dot - 1;
};

function scaled(v: string, scale: number): bigint {
  const neg = v.startsWith('-');
  const body = v.replace(/^[-+]/, '');
  const [int = '0', frac = ''] = body.split('.');
  const n = BigInt(int + frac.padEnd(scale, '0'));
  return neg ? -n : n;
}

function unscaled(n: bigint, scale: number): string {
  const neg = n < 0n;
  const digits = (neg ? -n : n).toString().padStart(scale + 1, '0');
  const int = digits.slice(0, digits.length - scale);
  const frac = scale ? `.${digits.slice(-scale)}` : '';
  return `${neg ? '-' : ''}${int}${frac}`;
}

function isNumericColumn(c: Column): boolean {
  if (c.numeric === false) return c.values.every((v) => v === null);
  for (const v of c.values) if (v !== null && !NUMBER.test(v.trim())) return false;
  return true;
}

export function aggregate(cols: readonly Column[]): Stats {
  let cells = 0;
  let nulls = 0;
  for (const c of cols) {
    cells += c.values.length;
    for (const v of c.values) if (v === null) nulls++;
  }
  const count = cells - nulls;
  const numeric = count > 0 && cols.every(isNumericColumn);
  const base: Stats = { cells, nulls, count, numeric, sum: null, avg: null, min: null, max: null, distinct: null, scale: 0 };
  if (!numeric) {
    const seen = new Set<string>();
    for (const c of cols) for (const v of c.values) if (v !== null) seen.add(v);
    return { ...base, distinct: seen.size };
  }
  let exact = true;
  let scale = 0;
  let min: string | null = null;
  let max: string | null = null;
  let minN = Number.POSITIVE_INFINITY;
  let maxN = Number.NEGATIVE_INFINITY;
  let float = 0;
  for (const c of cols)
    for (const raw of c.values) {
      if (raw === null) continue;
      const v = raw.trim();
      const n = Number(v);
      float += n;
      if (n < minN) [minN, min] = [n, v];
      if (n > maxN) [maxN, max] = [n, v];
      if (exact && DECIMAL.test(v)) scale = Math.max(scale, scaleOf(v));
      else exact = false;
    }
  exact &&= scale <= MAX_EXACT_SCALE;
  let sum: string;
  if (exact) {
    let total = 0n;
    for (const c of cols) for (const v of c.values) if (v !== null) total += scaled(v.trim(), scale);
    sum = unscaled(total, scale);
  } else sum = String(float);
  return { ...base, sum, avg: Number(sum) / count, min, max, scale: Math.min(scale, 6) };
}

export function formatDecimal(v: string, locale: string, maxFraction = 6): string {
  if (!DECIMAL.test(v)) {
    const n = Number(v);
    return Number.isFinite(n) ? new Intl.NumberFormat(locale, { maximumFractionDigits: maxFraction }).format(n) : v;
  }
  const neg = v.startsWith('-');
  const [int = '0', frac = ''] = v.replace(/^[-+]/, '').split('.');
  const sep = new Intl.NumberFormat(locale).formatToParts(1.5).find((p) => p.type === 'decimal')?.value ?? '.';
  const grouped = new Intl.NumberFormat(locale).format(BigInt(int));
  const cut = frac.slice(0, maxFraction);
  return `${neg ? '-' : ''}${grouped}${cut ? sep + cut : ''}`;
}

export function formatAvg(n: number, scale: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: Math.min(4, Math.max(2, scale + 1)) }).format(n);
}

const NOT_NUMBER = /char|text|string|uuid|date|time|interval|json|xml|bool|bit\b|bytea|blob|binary|enum|inet|cidr|mac|geo|point|array|\[\]/i;

export function numericHint(type: string | null, numeric: boolean): boolean | null {
  if (numeric) return true;
  return type && NOT_NUMBER.test(type) ? false : null;
}
