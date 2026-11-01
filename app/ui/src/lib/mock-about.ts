import { snapshot } from './mock-seclab';
import { UpdateError, type AppInfo, type InstallOutcome, type UpdatePrefs, type UpdateProgress, type UpdateStatus } from './about-api';

export const MOCK_UPDATE = {
  current: '0.1.0',
  version: '0.2.0',
  size: 24 * 1024 * 1024,
  notes: {
    it: "Aggiornamenti automatici e una pagina Informazioni con le licenze.\nIl diagramma ER ricorda lo zoom per ogni database.",
    en: 'Automatic updates and an About page with the licences.\nThe ER diagram remembers the zoom for each database.',
  },
};

export interface AboutMockOptions {
  update?: 'none' | 'available' | 'saved' | 'offline';
  speed?: number;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function aboutMock(opts: AboutMockOptions = {}) {
  const mode = opts.update ?? 'none';
  const speed = opts.speed ?? 1;
  let prefs: UpdatePrefs = { check_on_start: true, last_check: Date.UTC(2026, 9, 30, 9, 12) };
  let found = false;
  let downloaded = false;
  let installed = false;
  const info: AppInfo = { version: MOCK_UPDATE.current, commit: '257f8c9a1e', built: '2026-10-30', platform: 'linux-x86_64', update_url: 'https://github.com/maxib0n/NiceDB/releases/latest/download/latest.json' };

  return {
    appInfo: async (): Promise<AppInfo> => ({ ...info }),
    openLink: async (url: string): Promise<void> => {
      if (typeof window !== 'undefined') ((window as unknown as { __ndLinks?: string[] }).__ndLinks ??= []).push(url);
    },
    updateCheck: async (auto: boolean): Promise<UpdateStatus> => {
      if (auto && !prefs.check_on_start) return { status: 'skipped' };
      if (auto && mode === 'none') return { status: 'skipped' };
      await sleep(400 * speed);
      if (mode === 'offline') throw new UpdateError({ code: 'network', detail: 'github.com: no route to host' });
      prefs = { ...prefs, last_check: Date.now() };
      found = mode !== 'none';
      if (!found) return { status: 'up_to_date', current: MOCK_UPDATE.current };
      return { status: 'available', current: MOCK_UPDATE.current, version: MOCK_UPDATE.version, notes: { ...MOCK_UPDATE.notes }, pub_date: '2026-10-30T09:00:00Z' };
    },
    updateDownload: async (onProgress: (p: UpdateProgress) => void): Promise<void> => {
      if (!found) throw new UpdateError({ code: 'no_update' });
      const step = MOCK_UPDATE.size / 8;
      for (let done = step; done <= MOCK_UPDATE.size; done += step) {
        await sleep(120 * speed);
        onProgress({ downloaded: done, total: MOCK_UPDATE.size });
      }
      downloaded = true;
    },
    updateInstall: async (): Promise<InstallOutcome> => {
      if (!downloaded) throw new UpdateError({ code: 'not_downloaded' });
      await sleep(200 * speed);
      if (mode === 'saved') return { mode: 'downloaded', path: `/home/ada/Scaricati/NiceDB-linux-x86_64-${MOCK_UPDATE.version}.deb` };
      installed = true;
      return { mode: 'replaced', path: '/home/ada/Applicazioni/NiceDB-linux-x86_64' };
    },
    updateRestart: async (): Promise<void> => {
      if (!installed) throw new UpdateError({ code: 'not_downloaded' });
      if (typeof location !== 'undefined' && !import.meta.env.VITEST) location.reload();
    },
    seclabVersionsUpdate: async (): Promise<string> => {
      if (mode === 'offline') throw new UpdateError({ code: 'network', detail: 'offline' });
      snapshot.date = '2026-11-01';
      return snapshot.date;
    },
    updatePrefsGet: async (): Promise<UpdatePrefs> => ({ ...prefs }),
    updatePrefsSet: async (checkOnStart: boolean): Promise<UpdatePrefs> => {
      prefs = { ...prefs, check_on_start: checkOnStart };
      return { ...prefs };
    },
  };
}

export function aboutMockFromUrl() {
  const p = new URLSearchParams(typeof location === 'undefined' ? '' : location.search).get('update');
  const update = p === 'available' || p === 'saved' || p === 'offline' ? p : 'none';
  return aboutMock({ update });
}
