import { WORDMARK_DISPLAY, WORDMARK_TEXT } from './brand-glyphs';

export function wordmarkGeometry(size: number, text = false) {
  const cut = size >= 36 && !text ? WORDMARK_DISPLAY : WORDMARK_TEXT;
  const s = size / 1000;
  const base = Math.ceil(size * 0.73);
  const t = Math.max(1, Math.round(size * 0.026));
  const g = Math.max(1, Math.round(size * 0.032));
  const y1 = base + Math.max(3, Math.round(size * 0.13));
  const y2 = y1 + t + g;
  const x0 = Math.round((cut.rule0 - cut.x0) * s);
  const x1 = Math.round((cut.rule1 - cut.x0) * s);
  return {
    d: cut.d,
    origin: { x: -cut.x0 * s, y: base },
    em: s,
    width: Math.ceil((cut.w - cut.x0) * s),
    height: y2 + t,
    glyphs: `translate(${-cut.x0 * s} ${base}) scale(${s})`,
    rules: [
      { x: x0, y: y1, w: x1 - x0, h: t },
      { x: x0, y: y2, w: x1 - x0, h: t },
    ],
  };
}
