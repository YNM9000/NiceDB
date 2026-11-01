const FOCUSABLE = 'input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), [tabindex="0"]';

export function focusFirst(container: ParentNode | null | undefined) {
  container?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
}

export function escapeTo(root: Element | null | undefined, opener: HTMLElement | null | undefined) {
  if (root?.contains(document.activeElement)) opener?.focus();
}

export function roveMenu(e: KeyboardEvent, menu: HTMLElement) {
  const items = [...menu.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([disabled])')];
  if (!items.length) return;
  const at = items.indexOf(document.activeElement as HTMLElement);
  const next = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: items.length - 1 }[e.key];
  if (next === undefined) return;
  e.preventDefault();
  items[(next + items.length) % items.length]?.focus();
}

export function installKeyboardMode(root: HTMLElement = document.documentElement) {
  let pointer = false;
  addEventListener('keydown', (e) => {
    pointer = false;
    document.activeElement?.removeAttribute('data-quiet-focus');
    if (e.key === 'Tab') root.setAttribute('data-kbd', '');
  }, true);
  addEventListener('pointerdown', () => {
    pointer = true;
    root.removeAttribute('data-kbd');
  }, true);
  addEventListener('focusin', (e) => {
    if (pointer && e.target instanceof Element && !e.target.matches('input, textarea, select, [contenteditable]')) e.target.setAttribute('data-quiet-focus', '');
  }, true);
  addEventListener('focusout', (e) => e.target instanceof Element && e.target.removeAttribute('data-quiet-focus'), true);
}

export function mirrorInvalid(root: Node = document.body) {
  const marked = new WeakSet<Element>();
  const sync = (field: Element) => {
    const bad = field.classList.contains('invalid');
    for (const c of field.querySelectorAll('input, select, textarea, [role=combobox]')) {
      if (bad && !c.hasAttribute('aria-invalid')) {
        c.setAttribute('aria-invalid', 'true');
        marked.add(c);
      } else if (!bad && marked.has(c) && !c.closest('.invalid')) {
        c.removeAttribute('aria-invalid');
        marked.delete(c);
      }
    }
  };
  new MutationObserver((list) => {
    for (const r of list) if (r.target instanceof Element && r.target.classList.contains('form-field')) sync(r.target);
  }).observe(root, { attributes: true, attributeFilter: ['class'], subtree: true });
}
