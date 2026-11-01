import { cueJingle, type JingleCue } from './sound';

export type SplashMode = 'off' | 'on' | 'hold';

export const CEREMONY_MS = 3450;
export const FADE_MS = 300;
export const SKIP_FADE_MS = 140;
export const REDUCED_HOLD_MS = 800;
export const REDUCED_FADE_MS = 200;
export const STAGE_W = 1440;
export const STAGE_H = 900;

export function splashMode(search: string, webdriver: boolean): SplashMode {
  const flag = new URLSearchParams(search).get('splash');
  if (flag === 'hold') return 'hold';
  if (flag === '1') return 'on';
  if (flag === '0') return 'off';
  return webdriver ? 'off' : 'on';
}

export function splashPlan(reduced: boolean) {
  return reduced ? { holdMs: REDUCED_HOLD_MS, fadeMs: REDUCED_FADE_MS } : { holdMs: CEREMONY_MS, fadeMs: FADE_MS };
}

export const splashCue = (): JingleCue | null => cueJingle(CEREMONY_MS);
