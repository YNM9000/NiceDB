import { placeAround, type Placed, type Rect, type Size } from './menu-place';

export interface FloatOptions {
  anchor: HTMLElement;
  match?: boolean;
  onaway?: () => void;
  place?: (anchor: Rect, size: Size, view: Size) => Placed;
}

const topLayer = typeof HTMLElement !== 'undefined' && 'showPopover' in HTMLElement.prototype;
const zoom = () => Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1;

export function float(node: HTMLElement, options: FloatOptions) {
  let opts = options;
  Object.assign(node.style, { position: 'fixed', right: 'auto', bottom: 'auto', margin: '0', overflowY: 'auto', overscrollBehavior: 'contain' });
  if (topLayer) {
    node.setAttribute('popover', 'manual');
    node.showPopover();
  }

  function place() {
    const z = zoom();
    const a = opts.anchor.getBoundingClientRect();
    if (opts.match) node.style.minWidth = `${a.width / z}px`;
    node.style.maxHeight = '';
    const cap = Number.parseFloat(getComputedStyle(node).maxHeight);
    const size = { w: node.offsetWidth * z, h: Math.min(node.scrollHeight, Number.isFinite(cap) ? cap : Infinity) * z };
    const at = (opts.place ?? placeAround)(a, size, { w: window.innerWidth, h: window.innerHeight });
    node.style.left = `${at.x / z}px`;
    node.style.top = `${at.y / z}px`;
    node.style.maxHeight = `${at.maxHeight / z}px`;
    node.dataset.side = at.y < a.top ? 'up' : 'down';
  }

  const scroll = (e: Event) => {
    if (e.target instanceof Node && node.contains(e.target)) return;
    opts.onaway?.();
  };
  place();
  window.addEventListener('resize', place);
  window.addEventListener('scroll', scroll, true);

  return {
    update(next: FloatOptions) {
      opts = next;
      place();
    },
    destroy() {
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', scroll, true);
      if (topLayer && node.matches(':popover-open')) node.hidePopover();
    },
  };
}
