import { ApiError, type Api, type ConnectionProfile } from './api';
import { readPrefs } from './mock-notes';
import { gate, loosens } from './prodsafety';

export function prodGateMock(base: Api): Partial<Api> {
  const lowering = async (p: ConnectionProfile, confirm: string | null | undefined) => {
    const before = (await base.connectionsList()).find((c) => c.id === p.id);
    if (before?.environment !== 'production' || p.environment === 'production') return;
    const code = gate(readPrefs().safety.confirm_mode, true, before.name, confirm);
    if (code) throw new ApiError({ code });
  };
  return {
    preferencesSet: async (v, confirm) => {
      if (confirm == null && loosens(readPrefs().safety, v.safety)) throw new ApiError({ code: 'needs_confirmation' });
      return base.preferencesSet(v);
    },
    connectionSave: async (p, s, confirm) => {
      await lowering(p, confirm);
      await base.connectionSave(p, s);
    },
  };
}
