import { WORDMARK_TEXT } from './brand-glyphs';
import { wordmarkGeometry } from './wordmark';

export const TRAVEL_MS = 720;
export const SKIP_TRAVEL_MS = 260;
export const TRAVEL_EASE = 'cubic-bezier(0.7, 0, 0.12, 1)';
export const SKIP_EASE = 'cubic-bezier(0.3, 0, 0.1, 1)';

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Placement {
  x: number;
  y: number;
  em: number;
}

export function glyphPlacement(box: Box, size: number, text = false): Placement {
  const g = wordmarkGeometry(size, text);
  const k = box.width / g.width;
  return { x: box.left + g.origin.x * k, y: box.top + g.origin.y * k, em: g.em * k };
}

export function flipTransform(from: Box, to: Box): string {
  return `translate(${from.left - to.left}px, ${from.top - to.top}px) scale(${from.width / to.width}, ${from.height / to.height})`;
}

export function placementTransform(p: Placement, unit: number): string {
  return `translate(${p.x / unit}px, ${p.y / unit}px) scale(${p.em / unit})`;
}

export function pickTarget(doc: Document): SVGSVGElement | null {
  for (const sel of ['.wizard .titlebar .wordmark', '.shell .titlebar .wordmark']) {
    const el = doc.querySelector<SVGSVGElement>(sel);
    if (el && el.getBoundingClientRect().width > 4) return el;
  }
  return null;
}

const NS = 'http://www.w3.org/2000/svg';

export interface Flight {
  done: Promise<void>;
  cancel(): void;
}

export function flyMark(source: SVGSVGElement, rules: HTMLElement[], target: SVGSVGElement, ms: number, ease: string): Flight {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return { done: Promise.resolve(), cancel() {} };
  const html = document.documentElement;
  const layer = document.body.appendChild(document.createElement('div'));
  layer.className = 'mark-flight';
  const probe = layer.appendChild(document.createElement('i'));
  probe.style.cssText = 'position:absolute;left:0;top:0;width:1000px;height:1px';
  const unit = probe.getBoundingClientRect().width / 1000 || 1;
  probe.remove();

  const from = glyphPlacement(source.getBoundingClientRect(), Number(source.dataset.size), source.dataset.text !== undefined);
  const to = glyphPlacement(target.getBoundingClientRect(), Number(target.dataset.size), target.dataset.text !== undefined);
  const ink = getComputedStyle(target).color;
  const svg = layer.appendChild(document.createElementNS(NS, 'svg'));
  svg.setAttribute('class', 'mf-glyphs');
  svg.setAttribute('width', '1');
  svg.setAttribute('height', '1');
  const landed = svg.appendChild(document.createElementNS(NS, 'path'));
  landed.setAttribute('d', WORDMARK_TEXT.d);
  landed.setAttribute('fill', ink);
  const lifted = svg.appendChild(document.createElementNS(NS, 'path'));
  lifted.setAttribute('d', WORDMARK_TEXT.d);
  lifted.setAttribute('fill', getComputedStyle(source).color);

  const anims: Animation[] = [];
  const timing = { duration: ms, easing: ease, fill: 'forwards' as const };
  anims.push(svg.animate([{ transform: placementTransform(from, unit) }, { transform: placementTransform(to, unit) }], timing));
  anims.push(lifted.animate([{ opacity: 1 }, { opacity: 0 }], { duration: ms * 0.6, easing: 'ease-in-out', fill: 'forwards' }));

  const targets = [...target.querySelectorAll<SVGRectElement>('.wordmark-rule')];
  targets.forEach((rect, i) => {
    const src = rules[i];
    if (!src) return;
    const t = rect.getBoundingClientRect();
    const box = { left: t.left / unit, top: t.top / unit, width: t.width / unit, height: t.height / unit };
    const s = src.getBoundingClientRect();
    const el = layer.appendChild(document.createElement('i'));
    el.className = 'mf-rule';
    el.style.cssText = `left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;background:${getComputedStyle(rect).fill}`;
    const start = { left: s.left / unit, top: s.top / unit, width: s.width / unit, height: s.height / unit };
    anims.push(el.animate([{ transform: flipTransform(start, box) }, { transform: 'none' }], timing));
  });

  html.dataset.markFlying = '';
  const cleanup = () => {
    delete html.dataset.markFlying;
    const settle = layer.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 90, easing: 'linear', fill: 'forwards' });
    return settle.finished.then(
      () => layer.remove(),
      () => layer.remove(),
    );
  };
  const done = (anims[0]?.finished ?? Promise.resolve()).then(cleanup, cleanup);
  return {
    done: done.then(() => undefined),
    cancel() {
      anims.forEach((a) => a.cancel());
      delete html.dataset.markFlying;
      layer.remove();
    },
  };
}
