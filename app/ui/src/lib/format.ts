import { getLocale } from '../paraglide/runtime';

export const tag = () => (getLocale() === 'it' ? 'it-IT' : 'en-GB');

export const num = (n: number) => new Intl.NumberFormat(tag()).format(n);
export const compact = (n: number) => new Intl.NumberFormat(getLocale() === 'it' ? 'it-IT' : 'en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(n);
export const when = (ms: number) => new Intl.DateTimeFormat(tag(), { dateStyle: 'short', timeStyle: 'short' }).format(new Date(ms));
