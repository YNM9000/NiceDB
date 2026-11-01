export function tabKey(e: KeyboardEvent) {
  const list = e.currentTarget;
  if (!(list instanceof HTMLElement)) return;
  const tabs = [...list.querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])')];
  const at = tabs.findIndex((t) => t === document.activeElement);
  const from = at >= 0 ? at : tabs.findIndex((t) => t.getAttribute('aria-selected') === 'true');
  const n = tabs.length;
  const to = { ArrowRight: from + 1, ArrowLeft: from - 1, Home: 0, End: n - 1 }[e.key];
  if (to === undefined || n === 0) return;
  e.preventDefault();
  const next = tabs[(to + n) % n];
  next?.click();
  next?.focus();
}
