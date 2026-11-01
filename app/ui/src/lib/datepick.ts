export interface Day {
  y: number;
  m: number;
  d: number;
}

const DAY_MS = 86_400_000;
const ISO = /^(\d{4})-(\d{2})-(\d{2})/;

const pad = (n: number, w = 2) => String(n).padStart(w, '0');
const ms = (x: Day) => Date.UTC(x.y, x.m - 1, x.d);
const fromMs = (t: number): Day => {
  const d = new Date(t);
  return { y: d.getUTCFullYear(), m: d.getUTCMonth() + 1, d: d.getUTCDate() };
};

export function parseDay(text: string | null | undefined): Day | null {
  const hit = ISO.exec((text ?? '').trim());
  if (!hit) return null;
  const day = { y: Number(hit[1]), m: Number(hit[2]), d: Number(hit[3]) };
  return day.y >= 1 && same(fromMs(ms(day)), day) ? day : null;
}

export const isDay = (text: string) => /^\d{4}-\d{2}-\d{2}$/.test(text.trim()) && parseDay(text) !== null;

export const formatDay = (x: Day) => `${pad(x.y, 4)}-${pad(x.m)}-${pad(x.d)}`;

export function withDay(text: string, day: Day): string {
  const rest = parseDay(text) ? text.trim().slice(10) : '';
  return formatDay(day) + rest;
}

export const same = (a: Day | null, b: Day | null) => !!a && !!b && a.y === b.y && a.m === b.m && a.d === b.d;
export const compare = (a: Day, b: Day) => ms(a) - ms(b);
export const addDays = (x: Day, n: number) => fromMs(ms(x) + n * DAY_MS);

export function addMonths(x: Day, n: number): Day {
  const index = x.y * 12 + (x.m - 1) + n;
  const y = Math.floor(index / 12);
  const m = index - y * 12 + 1;
  return { y, m, d: Math.min(x.d, daysIn(y, m)) };
}

export const daysIn = (y: number, m: number) => new Date(Date.UTC(y, m, 0)).getUTCDate();

export function clamp(x: Day, min: Day | null, max: Day | null): Day {
  if (min && compare(x, min) < 0) return min;
  if (max && compare(x, max) > 0) return max;
  return x;
}

export const outside = (x: Day, min: Day | null, max: Day | null) => (!!min && compare(x, min) < 0) || (!!max && compare(x, max) > 0);

export const weekStart = (locale: string) => (/^en-US\b/.test(locale) ? 0 : 1);

const weekday = (x: Day) => new Date(ms(x)).getUTCDay();

export function startOfWeek(x: Day, first: number): Day {
  return addDays(x, -((weekday(x) - first + 7) % 7));
}

export const endOfWeek = (x: Day, first: number) => addDays(startOfWeek(x, first), 6);

export function monthGrid(y: number, m: number, first: number): Day[][] {
  const start = startOfWeek({ y, m, d: 1 }, first);
  return Array.from({ length: 6 }, (_, w) => Array.from({ length: 7 }, (_, i) => addDays(start, w * 7 + i)));
}

export function today(now = new Date()): Day {
  return { y: now.getFullYear(), m: now.getMonth() + 1, d: now.getDate() };
}

const fmt = (locale: string, o: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, { ...o, timeZone: 'UTC' });

export const monthLabel = (y: number, m: number, locale: string) => fmt(locale, { month: 'long', year: 'numeric' }).format(ms({ y, m, d: 1 }));
export const dayLabel = (x: Day, locale: string) => fmt(locale, { dateStyle: 'full' }).format(ms(x));

export function weekdays(locale: string, first: number): { short: string; long: string }[] {
  const sunday = { y: 2023, m: 1, d: 1 };
  return Array.from({ length: 7 }, (_, i) => {
    const x = addDays(sunday, (first + i) % 7);
    return { short: fmt(locale, { weekday: 'short' }).format(ms(x)).replace(/\.$/, ''), long: fmt(locale, { weekday: 'long' }).format(ms(x)) };
  });
}

export type DayKey = 'ArrowLeft' | 'ArrowRight' | 'ArrowUp' | 'ArrowDown' | 'PageUp' | 'PageDown' | 'Home' | 'End';

export function moveDay(x: Day, key: string, shift: boolean, first: number): Day | null {
  switch (key as DayKey) {
    case 'ArrowLeft':
      return addDays(x, -1);
    case 'ArrowRight':
      return addDays(x, 1);
    case 'ArrowUp':
      return addDays(x, -7);
    case 'ArrowDown':
      return addDays(x, 7);
    case 'PageUp':
      return addMonths(x, shift ? -12 : -1);
    case 'PageDown':
      return addMonths(x, shift ? 12 : 1);
    case 'Home':
      return startOfWeek(x, first);
    case 'End':
      return endOfWeek(x, first);
    default:
      return null;
  }
}

export const isDateType = (type: string | null | undefined) => /\b(date\w*|smalldatetime|timestamp\w*)\b/i.test(type ?? '');
