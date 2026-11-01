export const PANES = {
  sidebar: { min: 200, max: 520 },
  panel: { min: 260, max: 640 },
  history: { min: 220, max: 560 },
  audit: { min: 240, max: 600 },
  related: { min: 260, max: 600 },
  value: { min: 280, max: 720 },
  tools: { min: 168, max: 360 },
} as const;

export type Pane = keyof typeof PANES;
export type Widths = Partial<Record<Pane, number>>;

export const MAIN_MIN = 360;
export const SHEET_MIN = 620;
export const STEP = 16;
export const BIG_STEP = 64;

const isPane = (k: string): k is Pane => Object.prototype.hasOwnProperty.call(PANES, k);

export function maxWidth(pane: Pane, total: number, others: number): number {
  const { min, max } = PANES[pane];
  const room = total - others - MAIN_MIN;
  return Math.max(min, Math.min(max, Math.floor(pane === 'sidebar' ? Math.min(room, total - SHEET_MIN) : room)));
}

export function clampWidth(pane: Pane, width: number, limit: number = PANES[pane].max): number {
  const { min } = PANES[pane];
  return Math.round(Math.min(Math.max(width, min), Math.max(min, limit)));
}

export function cleanWidths(raw: Partial<Record<string, number>> | null | undefined): Widths {
  const out: Widths = {};
  for (const [k, v] of Object.entries(raw ?? {})) if (isPane(k) && typeof v === 'number' && Number.isFinite(v)) out[k] = clampWidth(k, v);
  return out;
}

export function keyWidth(pane: Pane, width: number, limit: number, key: string, shift: boolean, grows: 'ArrowLeft' | 'ArrowRight'): number | null {
  const step = shift ? BIG_STEP : STEP;
  if (key === 'Home') return PANES[pane].min;
  if (key === 'End') return clampWidth(pane, limit, limit);
  if (key !== 'ArrowLeft' && key !== 'ArrowRight') return null;
  return clampWidth(pane, width + (key === grows ? step : -step), limit);
}

export function widthVars(widths: Widths): [string, string | null][] {
  return (Object.keys(PANES) as Pane[]).map((p) => {
    const w = widths[p];
    return [`--nd-w-${p}`, w === undefined ? null : `${w}px`];
  });
}
