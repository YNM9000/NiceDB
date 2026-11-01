import { float } from './float';

export interface SuggestOptions {
  options: readonly (string | number)[];
  label?: string;
}

let uid = 0;

export function suggest(input: HTMLInputElement, initial: SuggestOptions) {
  let opts = initial;
  const id = `suggest-${++uid}`;
  let list: HTMLDivElement | null = null;
  let placed: ReturnType<typeof float> | null = null;
  let shown: string[] = [];
  let active = -1;

  input.setAttribute('role', 'combobox');
  input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-expanded', 'false');
  input.setAttribute('autocomplete', 'off');

  const matches = (all: boolean) => {
    const q = input.value.trim().toLowerCase();
    const values = opts.options.map(String);
    if (all || !q) return values;
    return values.filter((v) => v.toLowerCase().includes(q) && v.toLowerCase() !== q);
  };

  function close() {
    placed?.destroy();
    placed = null;
    list?.remove();
    list = null;
    active = -1;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-controls');
    input.removeAttribute('aria-activedescendant');
  }

  function render() {
    if (!list) return;
    list.replaceChildren(
      ...shown.map((v, i) => {
        const row = document.createElement('div');
        row.id = `${id}-${i}`;
        row.className = i === active ? 'ndsel-opt active' : 'ndsel-opt';
        row.setAttribute('role', 'option');
        row.setAttribute('aria-selected', String(v === input.value));
        const mark = document.createElement('span');
        mark.className = 'ndsel-mark';
        const text = document.createElement('span');
        text.className = 'ndsel-text';
        text.textContent = v;
        row.append(mark, text);
        row.addEventListener('pointermove', () => {
          if (active === i) return;
          active = i;
          render();
        });
        row.addEventListener('click', () => pick(v));
        return row;
      }),
    );
    if (active >= 0) {
      input.setAttribute('aria-activedescendant', `${id}-${active}`);
      document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
    } else input.removeAttribute('aria-activedescendant');
  }

  function open(all: boolean) {
    shown = matches(all);
    if (!shown.length) return close();
    if (!list) {
      list = document.createElement('div');
      list.id = id;
      list.className = 'ndsel-list float suggest-list';
      list.setAttribute('role', 'listbox');
      if (opts.label) list.setAttribute('aria-label', opts.label);
      list.addEventListener('pointerdown', (e) => e.preventDefault());
      list.addEventListener('mousedown', (e) => e.preventDefault());
      (input.closest('dialog') ?? document.body).append(list);
      input.setAttribute('aria-expanded', 'true');
      input.setAttribute('aria-controls', id);
      render();
      placed = float(list, { anchor: input.closest<HTMLElement>('.field') ?? input, match: true, onaway: close });
      return;
    }
    active = Math.min(active, shown.length - 1);
    render();
    placed?.update({ anchor: input.closest<HTMLElement>('.field') ?? input, match: true, onaway: close });
  }

  function pick(v: string) {
    input.value = v;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
    close();
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!list) {
        if (e.key !== 'ArrowDown' || (input.type === 'number' && !e.altKey)) return;
        e.preventDefault();
        open(true);
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      active = e.key === 'ArrowDown' ? Math.min(shown.length - 1, active + 1) : Math.max(0, active - 1);
      render();
    } else if (e.key === 'Enter' && list && active >= 0) {
      e.preventDefault();
      e.stopPropagation();
      const v = shown[active];
      if (v !== undefined) pick(v);
    } else if (e.key === 'Escape' && list) {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  const oninput = (e: Event) => {
    if (e.isTrusted) open(false);
  };
  const onpointer = () => (list ? close() : open(true));
  input.addEventListener('keydown', onkey);
  input.addEventListener('input', oninput);
  input.addEventListener('pointerdown', onpointer);
  input.addEventListener('blur', close);

  return {
    update(next: SuggestOptions) {
      opts = next;
      if (list) open(false);
    },
    destroy() {
      close();
      input.removeEventListener('keydown', onkey);
      input.removeEventListener('input', oninput);
      input.removeEventListener('pointerdown', onpointer);
      input.removeEventListener('blur', close);
    },
  };
}
