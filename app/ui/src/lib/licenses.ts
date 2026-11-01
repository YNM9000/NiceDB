export type LicenceSource = 'crate' | 'npm' | 'font' | 'sound' | 'icon';

export interface LicenceEntry {
  source: LicenceSource;
  name: string;
  version: string;
  license: string;
  url: string;
  texts: string[];
  standard?: true;
}

export interface Licences {
  app: { license: string; text: string };
  entries: LicenceEntry[];
  texts: Record<string, string>;
}

export interface LicenceGroup {
  license: string;
  entries: LicenceEntry[];
}

export async function loadLicences(): Promise<Licences> {
  const mod = await import('../assets/licenses.json');
  return mod.default as Licences;
}

const fold = (s: string) => s.toLocaleLowerCase().normalize('NFD').replace(/\p{M}/gu, '');

export function groupLicences(entries: LicenceEntry[], query = ''): LicenceGroup[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  const groups = new Map<string, LicenceEntry[]>();
  for (const e of entries) {
    const hay = fold(`${e.name} ${e.version} ${e.license} ${e.source}`);
    if (!words.every((w) => hay.includes(w))) continue;
    const list = groups.get(e.license) ?? [];
    list.push(e);
    groups.set(e.license, list);
  }
  return [...groups]
    .map(([license, list]) => ({ license, entries: list.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version)) }))
    .sort((a, b) => b.entries.length - a.entries.length || a.license.localeCompare(b.license));
}

export const entryKey = (e: LicenceEntry) => `${e.source}:${e.name}@${e.version}`;
