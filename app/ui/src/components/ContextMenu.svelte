<script lang="ts">
  import { ChevronRight } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { closeMenu, contextMenu, type MenuAction } from '../lib/contextmenu.svelte';
  import { placeAtPoint, placeBeside, type Placed } from '../lib/menu-place';
  import { groups } from '../lib/contextmenu.svelte';

  interface Level {
    groups: MenuAction[][];
    items: MenuAction[];
    active: number;
    open: number | null;
    from: DOMRect | null;
    at: Placed | null;
  }

  let levels = $state<Level[]>([]);
  let panels = $state<HTMLDivElement[]>([]);
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let typed = '';
  let typedTimer: ReturnType<typeof setTimeout> | undefined;

  const id = (d: number, i: number) => `ctx-${d}-${i}`;
  const zoom = () => Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1;

  function level(g: MenuAction[][], from: DOMRect | null): Level {
    return { groups: g, items: g.flat(), active: -1, open: null, from, at: null };
  }

  $effect(() => {
    const open = contextMenu.open;
    clearTimeout(hoverTimer);
    levels = open ? [level(open.items, null)] : [];
    if (open) void tick().then(() => place(0));
  });

  async function place(d: number, focus = true) {
    const lv = levels[d];
    const el = panels[d];
    const open = contextMenu.open;
    if (!lv || !el || !open) return;
    const r = el.getBoundingClientRect();
    const z = zoom();
    const size = { w: r.width, h: el.scrollHeight * z };
    const view = { w: window.innerWidth, h: window.innerHeight };
    lv.at = lv.from ? placeBeside(lv.from, size, view) : placeAtPoint(open.x, open.y, size, view);
    await tick();
    if (focus) el.focus({ preventScroll: true });
  }

  const flatIndex = (lv: Level, g: number, i: number) => lv.groups.slice(0, g).reduce((n, x) => n + x.length, 0) + i;

  function step(lv: Level, dir: 1 | -1, from = lv.active) {
    const n = lv.items.length;
    for (let k = 1; k <= n; k++) {
      const i = (((from + dir * k) % n) + n) % n;
      if (!lv.items[i]?.disabled) return i;
    }
    return lv.active;
  }

  function truncate(d: number) {
    if (levels.length > d + 1) levels = levels.slice(0, d + 1);
    const lv = levels[d];
    if (lv) lv.open = null;
  }

  async function openSub(d: number, i: number, focusFirst: boolean) {
    const lv = levels[d];
    const item = lv?.items[i];
    if (!lv || !item?.items || item.disabled) return;
    if (lv.open === i && levels[d + 1]) {
      if (focusFirst) focusLevel(d + 1, true);
      return;
    }
    truncate(d);
    const row = document.getElementById(id(d, i));
    lv.open = i;
    levels = [...levels, level(groups(item.items), row?.getBoundingClientRect() ?? null)];
    await tick();
    await place(d + 1, focusFirst);
    if (focusFirst) focusLevel(d + 1, true);
  }

  function focusLevel(d: number, first = false) {
    const lv = levels[d];
    if (!lv) return;
    if (first && lv.active < 0) lv.active = step(lv, 1, -1);
    panels[d]?.focus({ preventScroll: true });
  }

  function activate(d: number, i: number) {
    const item = levels[d]?.items[i];
    if (!item || item.disabled) return;
    if (item.items) return void openSub(d, i, true);
    clearTimeout(hoverTimer);
    closeMenu(true);
    void Promise.resolve(item.run?.()).catch(() => {});
  }

  function onkeydown(e: KeyboardEvent, d: number) {
    const lv = levels[d];
    if (!lv) return;
    switch (e.key) {
      case 'ArrowDown':
        lv.active = step(lv, 1);
        break;
      case 'ArrowUp':
        lv.active = step(lv, -1);
        break;
      case 'Home':
        lv.active = step(lv, 1, -1);
        break;
      case 'End':
        lv.active = step(lv, -1, lv.items.length);
        break;
      case 'ArrowRight':
        if (lv.items[lv.active]?.items) void openSub(d, lv.active, true);
        break;
      case 'ArrowLeft':
        if (d === 0) break;
        truncate(d - 1);
        focusLevel(d - 1);
        break;
      case 'Escape':
        if (d === 0) closeMenu(true);
        else {
          truncate(d - 1);
          focusLevel(d - 1);
        }
        break;
      case 'Enter':
      case ' ':
        if (lv.active >= 0) activate(d, lv.active);
        break;
      case 'Tab':
        closeMenu(true);
        break;
      default:
        if (e.key.length !== 1 || e.ctrlKey || e.metaKey || e.altKey) return;
        clearTimeout(typedTimer);
        typed += e.key.toLowerCase();
        typedTimer = setTimeout(() => (typed = ''), 600);
        {
          const n = lv.items.length;
          const start = typed.length === 1 ? lv.active + 1 : Math.max(0, lv.active);
          for (let k = 0; k < n; k++) {
            const i = (start + k) % n;
            const it = lv.items[i];
            if (it && !it.disabled && it.label.toLowerCase().startsWith(typed)) {
              lv.active = i;
              break;
            }
          }
        }
    }
    e.preventDefault();
    e.stopPropagation();
  }

  function hover(d: number, i: number) {
    const lv = levels[d];
    if (!lv) return;
    lv.active = lv.items[i]?.disabled ? -1 : i;
    clearTimeout(hoverTimer);
    if (lv.open === i) return;
    hoverTimer = setTimeout(() => {
      if (lv.items[i]?.items) void openSub(d, i, false);
      else if (lv.open !== null) {
        truncate(d);
        panels[d]?.focus({ preventScroll: true });
      }
    }, 140);
  }

  $effect(() => {
    if (!contextMenu.open) return;
    const inside = (t: EventTarget | null) => t instanceof Node && panels.some((p) => p?.contains(t));
    const down = (e: PointerEvent) => {
      if (!inside(e.target)) closeMenu(false);
    };
    const scroll = (e: Event) => {
      if (!inside(e.target)) closeMenu(false);
    };
    const away = () => closeMenu(false);
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('wheel', scroll, { capture: true, passive: true });
    window.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', away);
    window.addEventListener('blur', away);
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('wheel', scroll, true);
      window.removeEventListener('scroll', scroll, true);
      window.removeEventListener('resize', away);
      window.removeEventListener('blur', away);
    };
  });

  function style(at: Placed | null) {
    if (!at) return 'visibility: hidden; left: 0; top: 0;';
    const z = zoom();
    return `left: ${at.x / z}px; top: ${at.y / z}px; max-height: ${at.maxHeight / z}px;`;
  }
</script>

{#if contextMenu.open}
  {#each levels as lv, d (d)}
    <div
      class="ctx-menu"
      class:sub={d > 0}
      role="menu"
      tabindex="-1"
      aria-label={d === 0 ? contextMenu.open.label : lv.items[levels[d - 1]?.open ?? -1]?.label}
      aria-activedescendant={lv.active >= 0 ? id(d, lv.active) : undefined}
      style={style(lv.at)}
      bind:this={panels[d]}
      onkeydown={(e) => onkeydown(e, d)}
      oncontextmenu={(e) => e.preventDefault()}
    >
      {#each lv.groups as group, g (g)}
        {#if g > 0}<div class="ctx-sep" role="separator"></div>{/if}
        {#each group as item, k (k)}
          {@const i = flatIndex(lv, g, k)}
          <div
            id={id(d, i)}
            class="ctx-item"
            class:active={lv.active === i}
            class:danger={item.danger}
            class:open={lv.open === i}
            role="menuitem"
            tabindex="-1"
            aria-disabled={item.disabled ? 'true' : undefined}
            aria-haspopup={item.items ? 'menu' : undefined}
            aria-expanded={item.items ? lv.open === i : undefined}
            onpointerenter={() => hover(d, i)}
            onpointerdown={(e) => e.preventDefault()}
            onclick={() => activate(d, i)}
            onkeydown={() => {}}
          >
            <span class="ctx-icon" aria-hidden="true">{#if item.icon}<item.icon size={14} strokeWidth={1.7} />{/if}</span>
            <span class="ctx-label">{item.label}</span>
            {#if item.hint}<span class="ctx-hint">{item.hint}</span>{/if}
            {#if item.items}
              <span class="ctx-chev" aria-hidden="true"><ChevronRight size={13} strokeWidth={1.8} /></span>
            {:else if item.shortcut}
              <kbd class="ctx-key">{item.shortcut}</kbd>
            {/if}
          </div>
        {/each}
      {/each}
    </div>
  {/each}
{/if}
