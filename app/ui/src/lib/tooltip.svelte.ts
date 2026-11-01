export const TIP_DELAY = 500;
export const TIP_WARM_DELAY = 80;
const WARM_FOR = 600;
const TIP_CHARS = 500;

export const tipText = (text: string) => (text.length > TIP_CHARS ? `${text.slice(0, TIP_CHARS).trimEnd()}…` : text);

class TipState {
  anchor = $state<HTMLElement | null>(null);
  text = $state('');
}

export const tip = new TipState();

let pending: HTMLElement | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
let hiddenAt = 0;
let described: HTMLElement | null = null;
let spoken: HTMLElement | null = null;
let dismissed: HTMLElement | null = null;

type Box = { left: number; right: number; top: number; bottom: number };

export function overTip(x: number, y: number, a: Box, t: Box): boolean {
  if (x >= t.left && x <= t.right && y >= t.top && y <= t.bottom) return true;
  const across = x >= Math.min(a.left, t.left) && x <= Math.max(a.right, t.right);
  if (t.top >= a.bottom - 1) return across && y >= a.bottom - 1 && y <= t.top + 1;
  if (t.bottom <= a.top + 1) return across && y >= t.bottom - 1 && y <= a.top + 1;
  return false;
}

function holding(e: PointerEvent): boolean {
  const t = tip.anchor && e.target instanceof Node && !tip.anchor.contains(e.target) ? document.getElementById('nd-tip') : null;
  return !!t && !!tip.anchor && overTip(e.clientX, e.clientY, tip.anchor.getBoundingClientRect(), t.getBoundingClientRect());
}

export function adoptTitle(node: EventTarget | null) {
  const el = node instanceof Element ? node.closest<HTMLElement>('[title]') : null;
  if (!el) return;
  const text = el.getAttribute('title') ?? '';
  el.removeAttribute('title');
  if (text.trim() && !el.dataset.tip) el.dataset.tip = text;
}

export function tipTarget(node: EventTarget | null): HTMLElement | null {
  adoptTitle(node);
  const el = node instanceof Element ? node.closest<HTMLElement>('[data-tip]') : null;
  return el?.dataset.tip?.trim() ? el : null;
}

function show(el: HTMLElement) {
  pending = null;
  const text = el.dataset.tip?.trim() ?? '';
  if (!el.isConnected || !text) return;
  tip.text = tipText(text);
  tip.anchor = el;
  if (!el.hasAttribute('aria-describedby') && el.getAttribute('aria-label') !== text) {
    el.setAttribute('aria-describedby', 'nd-tip');
    described = el;
  }
}

function schedule(el: HTMLElement) {
  if (el === tip.anchor || el === pending) return;
  clearTimeout(timer);
  const warm = tip.anchor !== null || performance.now() - hiddenAt < WARM_FOR;
  if (tip.anchor) hide();
  pending = el;
  timer = setTimeout(() => show(el), warm ? TIP_WARM_DELAY : TIP_DELAY);
}

export function hide() {
  clearTimeout(timer);
  pending = null;
  if (described) {
    described.removeAttribute('aria-describedby');
    described = null;
  }
  if (!tip.anchor) return;
  hiddenAt = performance.now();
  tip.anchor = null;
}

export function installTooltips(doc: Document = document): () => void {
  const move = (e: PointerEvent) => {
    if (e.pointerType === 'touch') return;
    if (holding(e)) {
      if (pending) (clearTimeout(timer), (pending = null));
      return;
    }
    const el = tipTarget(e.target);
    if (el !== dismissed) dismissed = null;
    if (el && el !== dismissed) schedule(el);
    else if (tip.anchor || pending) hide();
  };
  const focus = (e: FocusEvent) => {
    const el = tipTarget(e.target);
    if (el && el === e.target && !el.hasAttribute('aria-description') && el.getAttribute('aria-label') !== el.dataset.tip) {
      el.setAttribute('aria-description', tipText(el.dataset.tip ?? ''));
      spoken = el;
    }
    if (el && e.target instanceof Element && e.target.matches(':focus-visible')) schedule(el);
  };
  const key = (e: KeyboardEvent) => {
    if (e.key === 'Escape') dismissed = tip.anchor ?? pending;
    if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') hide();
  };
  const away = () => {
    spoken?.removeAttribute('aria-description');
    spoken = null;
    hide();
  };
  const leave = (e: PointerEvent) => {
    if (!e.relatedTarget) hide();
  };
  const opts = { capture: true, passive: true } as const;
  doc.addEventListener('pointermove', move, opts);
  doc.addEventListener('pointerout', leave, opts);
  doc.addEventListener('focusin', focus, true);
  doc.addEventListener('focusout', away, true);
  doc.addEventListener('pointerdown', away, opts);
  doc.addEventListener('keydown', key, true);
  doc.addEventListener('wheel', away, opts);
  window.addEventListener('blur', away);
  return () => {
    hide();
    doc.removeEventListener('pointermove', move, opts);
    doc.removeEventListener('pointerout', leave, opts);
    doc.removeEventListener('focusin', focus, true);
    doc.removeEventListener('focusout', away, true);
    doc.removeEventListener('pointerdown', away, opts);
    doc.removeEventListener('keydown', key, true);
    doc.removeEventListener('wheel', away, opts);
    window.removeEventListener('blur', away);
  };
}
