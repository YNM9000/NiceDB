import type { ConnectionProfile } from './api';

type Json = Record<string, unknown>;

const top = (path: string) => path.split('.')[0] ?? path;

function asObject(v: unknown): Json {
  return v && typeof v === 'object' ? (v as Json) : {};
}

export function fieldValues(p: ConnectionProfile): Map<string, string> {
  const out = new Map<string, string>();
  const target = asObject(p.target);
  for (const [k, v] of Object.entries(target)) {
    if (k === 'driver') continue;
    const raw = Object.entries(p.raw_fields)
      .filter(([path]) => top(path) === k)
      .sort(([a], [b]) => a.localeCompare(b));
    out.set(k, JSON.stringify([v ?? null, raw]));
  }
  for (const [k, v] of Object.entries(asObject(p.session))) out.set(`session.${k}`, JSON.stringify(v ?? null));
  return out;
}

export function overridesOf(own: ConnectionProfile, model: ConnectionProfile): string[] {
  if (own.target.driver !== model.target.driver) return [];
  const mine = fieldValues(own);
  const theirs = fieldValues(model);
  return [...mine.keys()].filter((k) => mine.get(k) !== theirs.get(k));
}

export function follow(model: ConnectionProfile, own: ConnectionProfile, overrides: string[]): ConnectionProfile {
  const same = model.target.driver === own.target.driver;
  const mine = (k: string) => same && overrides.includes(k);
  const target: Json = { ...asObject(structuredClone(model.target)) };
  const ownTarget = asObject(own.target);
  for (const k of overrides) if (same && !k.startsWith('session.') && k !== 'driver') target[k] = structuredClone(ownTarget[k] ?? null);
  const session: Json = { ...asObject(structuredClone(model.session)) };
  const ownSession = asObject(own.session);
  for (const k of overrides) if (k.startsWith('session.')) session[k.slice(8)] = structuredClone(ownSession[k.slice(8)] ?? null);
  const raw: Record<string, string> = {};
  for (const [k, v] of Object.entries(model.raw_fields)) if (!mine(top(k))) raw[k] = v;
  for (const [k, v] of Object.entries(own.raw_fields)) if (mine(top(k))) raw[k] = v;
  return { ...own, target: target as ConnectionProfile['target'], session: session as ConnectionProfile['session'], raw_fields: raw };
}

export const isModel = (p: ConnectionProfile) => p.template.kind === 'model';

export function modelOf(p: ConnectionProfile, list: ConnectionProfile[]): ConnectionProfile | undefined {
  const t = p.template;
  return t.kind === 'follows' ? list.find((c) => c.id === t.id && isModel(c)) : undefined;
}

export const followersOf = (id: string, list: ConnectionProfile[]) => list.filter((c) => c.template.kind === 'follows' && c.template.id === id);
