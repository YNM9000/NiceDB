export interface Size {
  w: number;
  h: number;
}

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Placed {
  x: number;
  y: number;
  maxHeight: number;
}

const MARGIN = 6;

function axis(start: number, size: number, limit: number, flipTo: number): number {
  if (start + size <= limit - MARGIN) return start;
  const flipped = flipTo - size;
  if (flipped >= MARGIN) return flipped;
  return Math.max(MARGIN, limit - MARGIN - size);
}

export function placeAtPoint(x: number, y: number, menu: Size, view: Size): Placed {
  const maxHeight = view.h - 2 * MARGIN;
  const h = Math.min(menu.h, maxHeight);
  return { x: axis(x, menu.w, view.w, x), y: axis(y, h, view.h, y), maxHeight };
}

export function placeBeside(item: Rect, menu: Size, view: Size, inset = 4): Placed {
  const maxHeight = view.h - 2 * MARGIN;
  const h = Math.min(menu.h, maxHeight);
  const x = axis(item.right - inset, menu.w, view.w, item.left + inset);
  const y = Math.min(Math.max(MARGIN, item.top - inset), view.h - MARGIN - h);
  return { x, y: Math.max(MARGIN, y), maxHeight };
}

export function placeBelow(anchor: Rect, menu: Size, view: Size): Placed {
  const maxHeight = view.h - 2 * MARGIN;
  const h = Math.min(menu.h, maxHeight);
  const below = anchor.bottom + 2;
  const y = below + h <= view.h - MARGIN ? below : anchor.top - 2 - h >= MARGIN ? anchor.top - 2 - h : Math.max(MARGIN, view.h - MARGIN - h);
  const x = Math.min(Math.max(MARGIN, anchor.left), view.w - MARGIN - menu.w);
  return { x: Math.max(MARGIN, x), y, maxHeight };
}

export function placeAround(anchor: Rect, menu: Size, view: Size, gap = 2): Placed {
  const below = view.h - MARGIN - (anchor.bottom + gap);
  const above = anchor.top - gap - MARGIN;
  const down = menu.h <= below || (menu.h > above && below >= above);
  const maxHeight = Math.max(0, Math.min(menu.h, down ? below : above));
  const x = Math.max(MARGIN, Math.min(anchor.left, view.w - MARGIN - menu.w));
  return { x, y: down ? anchor.bottom + gap : anchor.top - gap - maxHeight, maxHeight };
}

export function placeTip(anchor: Rect, tip: Size, view: Size, gap = 6): Placed {
  const maxHeight = view.h - 2 * MARGIN;
  const h = Math.min(tip.h, maxHeight);
  const below = anchor.bottom + gap;
  const above = anchor.top - gap - h;
  const y = below + h <= view.h - MARGIN || above < MARGIN ? Math.min(below, view.h - MARGIN - h) : above;
  const x = Math.min(Math.max(MARGIN, (anchor.left + anchor.right - tip.w) / 2), view.w - MARGIN - tip.w);
  return { x: Math.max(MARGIN, x), y: Math.max(MARGIN, y), maxHeight };
}
