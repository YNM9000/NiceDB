export interface Scrollable {
  getBoundingClientRect(): { left: number; top: number };
  clientLeft: number;
  clientTop: number;
  clientWidth: number;
  clientHeight: number;
}

export function onScrollbar(el: Scrollable, x: number, y: number): boolean {
  const box = el.getBoundingClientRect();
  return x >= box.left + el.clientLeft + el.clientWidth || y >= box.top + el.clientTop + el.clientHeight;
}
