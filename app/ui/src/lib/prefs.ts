import type { Locale } from './bindings';
import type { Preferences } from './api';

export const defaultPreferences: Preferences = {
  editor: { font_size: 13, tab_width: 2, autocomplete: true, uppercase_keywords: true, inspections: true, continue_on_error: false },
  results: { row_cap: 2_000_000, null_display: 'null', dates: 'raw', numbers: 'raw', monitor_ms: 5000 },
  safety: { timeout_local: 0, timeout_development: 0, timeout_staging: 300, timeout_production: 60, unlock_minutes: 10, unlock_mode: 'timed', confirm_mode: 'typed' },
  keys: {},
  drivers: { db2_clidriver: '' },
};

export const FONT_SIZES = [11, 12, 13, 14, 15, 16, 18] as const;
export const TAB_WIDTHS = [2, 3, 4, 8] as const;
export const ROW_CAPS = [1_000, 10_000, 100_000, 1_000_000, 2_000_000] as const;
export const TIMEOUTS = [0, 15, 30, 60, 300, 900, 3600] as const;
export const UNLOCKS = [2, 5, 10, 30, 60] as const;

export type NullDisplay = Preferences['results']['null_display'];

export function nullText(mode: NullDisplay): string {
  return mode === 'dash' ? '—' : mode === 'empty' ? '' : 'NULL';
}

const DATE = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?)(Z|[+-]\d{2}(?::?\d{2})?)?)?$/;
const NUMBER = /^(-?)(0|[1-9]\d*)(\.\d+)?$/;

export function showDate(v: string): string {
  const m = DATE.exec(v);
  if (!m) return v;
  const [, y, mo, d, time, zone] = m;
  const day = `${d}/${mo}/${y}`;
  return time ? `${day} ${time}${zone ?? ''}` : day;
}

export function showNumber(v: string, locale: Locale): string {
  const m = NUMBER.exec(v);
  if (!m) return v;
  const [, sign, int, frac] = m;
  const group = locale === 'it' ? '.' : ',';
  const point = locale === 'it' ? ',' : '.';
  const grouped = (int ?? '').replace(/\B(?=(\d{3})+$)/g, group);
  return `${sign}${grouped}${frac ? point + frac.slice(1) : ''}`;
}

const DATE_TYPE = /^(date|datetime|datetime2|smalldatetime|timestamp|timestamptz|timestamp without time zone|timestamp with time zone)\b/i;

export const isDateType = (type: string | null) => !!type && DATE_TYPE.test(type);

export interface CellStyle {
  nulls: NullDisplay;
  dates: boolean;
  numbers: boolean;
  locale: Locale;
}

export function cellText(value: string, type: string | null, numeric: boolean, style: CellStyle): string {
  if (style.dates && isDateType(type)) return showDate(value);
  if (style.numbers && numeric) return showNumber(value, style.locale);
  return value;
}
