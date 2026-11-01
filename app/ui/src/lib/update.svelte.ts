import { api } from './api';
import { UpdateError, type InstallOutcome, type UpdatePrefs, type UpdateStatus } from './about-api';
import { m } from '../paraglide/messages';

export type UpdatePhase = 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'installing' | 'ready' | 'saved' | 'error';

type Available = Extract<UpdateStatus, { status: 'available' }>;

export function updateErrorText(e: unknown): string {
  if (!(e instanceof UpdateError)) return e instanceof Error ? e.message : String(e);
  const f = e.fault;
  switch (f.code) {
    case 'network':
      return m.upd_err_network({ detail: f.detail });
    case 'invalid':
      return m.upd_err_invalid({ detail: f.detail });
    case 'signature':
      return m.upd_err_signature();
    case 'no_update':
    case 'not_downloaded':
      return m.upd_err_none();
    case 'install':
      return m.upd_err_install({ detail: f.detail });
    case 'restart':
      return m.upd_err_restart({ detail: f.detail });
    case 'link':
      return m.upd_err_link({ detail: f.detail });
  }
}

class Update {
  phase = $state<UpdatePhase>('idle');
  release = $state<Available | null>(null);
  progress = $state<number | null>(null);
  outcome = $state<InstallOutcome | null>(null);
  error = $state<string | null>(null);
  prefs = $state<UpdatePrefs | null>(null);
  dismissed = $state(false);
  beforeRestart: () => Promise<boolean> = async () => true;

  get busy(): boolean {
    return this.phase === 'checking' || this.phase === 'downloading' || this.phase === 'installing';
  }

  get banner(): boolean {
    return this.release !== null && !this.dismissed && this.phase !== 'checking' && this.phase !== 'up_to_date';
  }

  private apply(s: UpdateStatus): void {
    if (s.status === 'available') {
      this.release = s;
      this.phase = 'available';
    } else if (s.status === 'up_to_date') {
      this.release = null;
      this.phase = 'up_to_date';
    }
  }

  async autoCheck(): Promise<void> {
    try {
      this.prefs = await api.updatePrefsGet();
      const s = await api.updateCheck(true);
      if (s.status === 'available') this.apply(s);
    } catch {
      return;
    }
  }

  async loadPrefs(): Promise<void> {
    try {
      this.prefs = await api.updatePrefsGet();
    } catch {
      return;
    }
  }

  async check(): Promise<void> {
    if (this.busy) return;
    this.phase = 'checking';
    this.error = null;
    this.dismissed = false;
    try {
      this.apply(await api.updateCheck(false));
      this.prefs = await api.updatePrefsGet();
    } catch (e) {
      this.fail(e);
    }
  }

  async install(restart = true): Promise<void> {
    if (!this.release || this.busy) return;
    this.phase = 'downloading';
    this.progress = 0;
    this.error = null;
    try {
      await api.updateDownload((p) => {
        this.progress = p.total ? Math.min(1, (p.downloaded ?? 0) / p.total) : null;
      });
      this.phase = 'installing';
      this.outcome = await api.updateInstall();
      this.phase = this.outcome.mode === 'replaced' ? 'ready' : 'saved';
      if (this.phase === 'ready' && restart) await this.restart();
    } catch (e) {
      this.fail(e);
    }
  }

  async restart(): Promise<void> {
    try {
      if (!(await this.beforeRestart())) return;
      await api.updateRestart();
    } catch (e) {
      this.fail(e);
    }
  }

  async setCheckOnStart(on: boolean): Promise<void> {
    try {
      this.prefs = await api.updatePrefsSet(on);
    } catch (e) {
      this.fail(e);
    }
  }

  later(): void {
    this.dismissed = true;
  }

  private fail(e: unknown): void {
    this.error = updateErrorText(e);
    this.phase = 'error';
  }

  reset(): void {
    this.phase = 'idle';
    this.release = null;
    this.progress = null;
    this.outcome = null;
    this.error = null;
    this.prefs = null;
    this.dismissed = false;
  }
}

export const update = new Update();
