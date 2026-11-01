<script lang="ts" module>
  export interface SelectOption<T> {
    value: T;
    label: string;
    group?: string;
    disabled?: boolean;
    indent?: number;
  }
  let uid = 0;
</script>

<script lang="ts" generics="T">
  import { ChevronDown } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { float } from '../lib/float';

  interface Props {
    value: T;
    options: readonly SelectOption<T>[];
    onchange?: (value: T) => void;
    label?: string;
    labelledby?: string;
    id?: string;
    tip?: string;
    placeholder?: string;
    disabled?: boolean;
    keep?: boolean;
    class?: string;
  }
  let { value = $bindable(), options, onchange, label, labelledby, id, tip, placeholder = '', disabled = false, keep = true, class: cls = '' }: Props = $props();

  const list = `ndsel-${++uid}`;
  let open = $state(false);
  let active = $state(-1);
  let trigger = $state<HTMLButtonElement>();
  let typed = '';
  let typedAt = 0;

  const chosen = $derived(options.findIndex((o) => Object.is(o.value, value)));
  const groups = $derived.by(() => {
    const out: { name: string | undefined; items: { o: SelectOption<T>; i: number }[] }[] = [];
    options.forEach((o, i) => {
      const last = out.at(-1);
      if (last && last.name === o.group) last.items.push({ o, i });
      else out.push({ name: o.group, items: [{ o, i }] });
    });
    return out;
  });

  function show(at = chosen) {
    if (disabled || open) return;
    active = at >= 0 ? at : step(-1, 1);
    open = true;
    void tick().then(reveal);
  }

  function close() {
    open = false;
    active = -1;
  }

  function choose(i: number) {
    const o = options[i];
    close();
    if (!o || o.disabled) return;
    if (!Object.is(o.value, value)) {
      if (keep) value = o.value;
      onchange?.(o.value);
    }
  }

  function step(from: number, dir: 1 | -1, far = 1) {
    let at = from;
    let moved = 0;
    for (let i = from + dir; i >= 0 && i < options.length; i += dir) {
      if (options[i]?.disabled) continue;
      at = i;
      if (++moved >= far) break;
    }
    return at;
  }

  function reveal() {
    document.getElementById(`${list}-${active}`)?.scrollIntoView({ block: 'nearest' });
  }

  function move(to: number) {
    active = to;
    void tick().then(reveal);
  }

  function find(char: string) {
    const now = Date.now();
    typed = now - typedAt > 700 ? char : typed + char;
    typedAt = now;
    const norm = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().trim();
    const want = norm(typed);
    const start = typed.length === 1 ? Math.max(active, chosen) + 1 : Math.max(active, 0);
    for (let k = 0; k < options.length; k++) {
      const i = (start + k) % options.length;
      const o = options[i];
      if (o && !o.disabled && norm(o.label).startsWith(want)) return i;
    }
    return -1;
  }

  function onkeydown(e: KeyboardEvent) {
    const k = e.key;
    if (!open) {
      if (k === 'ArrowDown' || k === 'ArrowUp' || k === 'Enter' || k === ' ' || k === 'F4') {
        e.preventDefault();
        show();
      } else if (k === 'Home' || k === 'End') {
        e.preventDefault();
        show(k === 'Home' ? step(-1, 1) : step(options.length, -1));
      } else if (k.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const hit = find(k);
        if (hit >= 0) {
          e.preventDefault();
          show(hit);
        }
      }
      return;
    }
    if (k === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (k === 'ArrowDown' || k === 'ArrowUp') {
      e.preventDefault();
      if (e.altKey) choose(active);
      else move(step(active, k === 'ArrowDown' ? 1 : -1));
    } else if (k === 'PageDown' || k === 'PageUp') {
      e.preventDefault();
      move(step(active, k === 'PageDown' ? 1 : -1, 8));
    } else if (k === 'Home' || k === 'End') {
      e.preventDefault();
      move(k === 'Home' ? step(-1, 1) : step(options.length, -1));
    } else if (k === 'Enter' || (k === ' ' && Date.now() - typedAt > 700)) {
      e.preventDefault();
      choose(active);
    } else if (k === 'Tab') {
      choose(active);
    } else if (k.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      const hit = find(k);
      if (hit >= 0) move(hit);
    }
  }

  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const t = e.target as Node;
      if (trigger?.contains(t) || document.getElementById(list)?.contains(t)) return;
      close();
    };
    window.addEventListener('pointerdown', away, true);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('pointerdown', away, true);
      window.removeEventListener('blur', close);
    };
  });
</script>

<button
  bind:this={trigger}
  type="button"
  role="combobox"
  class="ndsel {cls}"
  class:open
  data-value={String(value)}
  {id}
  data-tip={tip}
  {disabled}
  aria-label={label}
  aria-labelledby={labelledby}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={open ? list : undefined}
  aria-activedescendant={open && active >= 0 ? `${list}-${active}` : undefined}
  onclick={(e) => {
    e.preventDefault();
    if (open) close();
    else show();
  }}
  onblur={() => open && close()}
  {onkeydown}
>
  <span class="ndsel-value" class:ndsel-empty={chosen < 0}>{chosen >= 0 ? options[chosen]?.label : placeholder}</span>
  <span class="ndsel-caret" aria-hidden="true"><ChevronDown size={12} strokeWidth={2} /></span>
</button>

{#if open && trigger}
  <div
    class="ndsel-list float"
    id={list}
    role="listbox"
    tabindex="-1"
    aria-label={label}
    aria-labelledby={labelledby}
    use:float={{ anchor: trigger, match: true, onaway: close }}
    onpointerdown={(e) => e.preventDefault()}
    onclick={(e) => e.preventDefault()}
    onkeydown={() => {}}
  >
    {#each groups as g, gi (gi)}
      {#if g.name}
        <div class="ndsel-group" role="group" aria-label={g.name}>
          <div class="ndsel-group-name" aria-hidden="true">{g.name}</div>
          {#each g.items as { o, i } (i)}
            {@render row(o, i)}
          {/each}
        </div>
      {:else}
        {#each g.items as { o, i } (i)}
          {@render row(o, i)}
        {/each}
      {/if}
    {/each}
  </div>
{/if}

{#snippet row(o: SelectOption<T>, i: number)}
  <div
    id="{list}-{i}"
    class="ndsel-opt"
    class:active={i === active}
    data-value={String(o.value)}
    role="option"
    tabindex="-1"
    aria-selected={i === chosen}
    aria-disabled={o.disabled ? 'true' : undefined}
    onpointermove={() => !o.disabled && (active = i)}
    onclick={() => choose(i)}
    onkeydown={() => {}}
  >
    <span class="ndsel-mark" aria-hidden="true"></span>
    <span class="ndsel-text" style:--ndsel-indent={o.indent || null}>{o.label}</span>
  </div>
{/snippet}
