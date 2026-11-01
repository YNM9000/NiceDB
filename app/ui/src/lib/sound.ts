export const OUTCOMES = ['done', 'applied', 'error', 'warn', 'notify', 'unlock', 'lock', 'copy'] as const;
export const GESTURES = ['toggle', 'drop'] as const;
export const SOUND_NAMES = [...OUTCOMES, ...GESTURES] as const;
export type SoundName = (typeof SOUND_NAMES)[number];
export type Clip = SoundName | 'jingle';
export const CLIPS: readonly Clip[] = [...SOUND_NAMES, 'jingle'];

export const TIMBRES = ['marimba', 'ciotola'] as const;
export type Timbre = (typeof TIMBRES)[number];

export const ESSENTIAL: readonly SoundName[] = ['done', 'error', 'warn', 'applied', 'unlock', 'lock'];

export interface SoundConfig {
  on: boolean;
  volume: number;
  reduced: boolean;
  timbre: Timbre;
  gestures: boolean;
  jingle: boolean;
}

export interface JingleCue {
  land(inMs: number): void;
  cancel(): void;
}

const FILES = import.meta.glob<string>('../assets/sounds/*/*.wav', { query: '?url', import: 'default', eager: true });

export function clipUrl(timbre: Timbre, clip: Clip): string | undefined {
  return FILES[`../assets/sounds/${timbre}/${clip}.wav`];
}

const CEILING = 0.4;
const GAP_MS = 150;
const BURST_MS = 1500;
const GESTURE_YIELD_MS = 60;
const LEAD_S = 0.005;

const DEFAULTS: SoundConfig = { on: true, volume: 30, reduced: false, timbre: 'marimba', gestures: false, jingle: true };
const config: SoundConfig = { ...DEFAULTS };
let ctx: AudioContext | null = null;
let master: GainNode | null = null;
const banks = new Map<Timbre, Promise<Map<Clip, AudioBuffer>>>();
const ready = new Map<Timbre, Map<Clip, AudioBuffer>>();
const lastAt = new Map<SoundName, number>();
let lastAny = -Infinity;
let errorBurstUntil = -Infinity;

export const played: Clip[] = [];

const masterLevel = () => (CEILING * config.volume) / 100;
const automated = () => typeof navigator !== 'undefined' && navigator.webdriver;
const hidden = () => typeof document !== 'undefined' && document.visibilityState === 'hidden';
const reducedMotion = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
const audible = () => config.on && config.volume > 0;

export function configureSound(next: Partial<SoundConfig>): void {
  Object.assign(config, next);
  config.volume = Math.min(100, Math.max(0, config.volume));
  if (master) master.gain.value = masterLevel();
  if (config.on) void preload(config.timbre);
}

export const soundsEnabled = () => config.on;

function audio(): AudioContext | null {
  if (ctx) return ctx;
  if (automated()) return null;
  try {
    const C = globalThis.AudioContext ?? (globalThis as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!C) return null;
    ctx = new C();
    master = ctx.createGain();
    master.gain.value = masterLevel();
    master.connect(ctx.destination);
    return ctx;
  } catch {
    return null;
  }
}

async function decode(c: AudioContext, url: string): Promise<AudioBuffer> {
  const res = await fetch(url);
  return c.decodeAudioData(await res.arrayBuffer());
}

export function preload(timbre: Timbre): Promise<Map<Clip, AudioBuffer>> {
  const known = banks.get(timbre);
  if (known) return known;
  const c = audio();
  if (!c) return Promise.resolve(new Map());
  const bank = new Map<Clip, AudioBuffer>();
  const loading = Promise.all(
    CLIPS.map(async (clip) => {
      const url = clipUrl(timbre, clip);
      if (!url) return;
      try {
        bank.set(clip, await decode(c, url));
      } catch {
        return;
      }
    }),
  ).then(() => {
    ready.set(timbre, bank);
    return bank;
  });
  banks.set(timbre, loading);
  return loading;
}

function log(name: Clip) {
  played.push(name);
  if (played.length > 50) played.shift();
}

function start(c: AudioContext, clip: Clip, at: number): AudioBufferSourceNode | null {
  const buf = ready.get(config.timbre)?.get(clip);
  if (!buf || !master) return null;
  const src = c.createBufferSource();
  src.buffer = buf;
  src.connect(master);
  src.start(at);
  return src;
}

function render(clip: Clip): void {
  if (automated()) return log(clip);
  const c = audio();
  if (!c) return;
  if (c.state === 'suspended') void c.resume().catch(() => undefined);
  if (start(c, clip, c.currentTime + LEAD_S)) log(clip);
}

function quietNow(name: SoundName): boolean {
  if (!audible() || hidden()) return true;
  const gesture = (GESTURES as readonly SoundName[]).includes(name);
  if (gesture && !config.gestures) return true;
  return (config.reduced || reducedMotion()) && !ESSENTIAL.includes(name);
}

function throttled(name: SoundName, now: number): boolean {
  if (now - (lastAt.get(name) ?? -Infinity) < GAP_MS) return true;
  if ((GESTURES as readonly SoundName[]).includes(name) && now - lastAny < GESTURE_YIELD_MS) return true;
  if (name !== 'error') return false;
  const inBurst = now < errorBurstUntil;
  errorBurstUntil = now + BURST_MS;
  return inBurst;
}

export function play(name: SoundName): void {
  try {
    if (quietNow(name)) return;
    const now = Date.now();
    if (throttled(name, now)) return;
    lastAt.set(name, now);
    lastAny = now;
    render(name);
  } catch {
    return;
  }
}

export function preview(name: Clip): void {
  try {
    if (!audible()) return;
    if (automated()) return log(name);
    void preload(config.timbre).then(() => render(name));
  } catch {
    return;
  }
}

export const jingleAllowed = () => audible() && config.jingle && !config.reduced && !reducedMotion() && !hidden();

export function cueJingle(landInMs: number): JingleCue | null {
  try {
    if (!jingleAllowed()) return null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let done = false;
    const strike = (inMs: number) => {
      const due = Date.now() + inMs;
      timer = setTimeout(
        () => {
          if (done || !jingleAllowed()) return;
          done = true;
          if (automated()) return log('jingle');
          const c = audio();
          if (!c) return;
          const go = () => {
            if (start(c, 'jingle', c.currentTime + Math.max(LEAD_S, (due - Date.now()) / 1000))) log('jingle');
          };
          void preload(config.timbre).then(() => (c.state === 'running' ? go() : c.resume().then(go, () => undefined)));
        },
        Math.max(0, inMs - 80),
      );
    };
    strike(landInMs);
    return {
      cancel() {
        clearTimeout(timer);
        done = true;
      },
      land(inMs) {
        if (done) return;
        clearTimeout(timer);
        strike(inMs);
      },
    };
  } catch {
    return null;
  }
}

export function resetSound(): void {
  ctx = null;
  master = null;
  banks.clear();
  ready.clear();
  Object.assign(config, DEFAULTS);
  lastAt.clear();
  lastAny = -Infinity;
  errorBurstUntil = -Infinity;
  played.length = 0;
}

if (typeof window !== 'undefined') (window as unknown as { __ndSounds?: Clip[] }).__ndSounds = played;
