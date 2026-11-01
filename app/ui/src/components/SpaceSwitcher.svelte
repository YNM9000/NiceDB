<script lang="ts">
  import { Check, ChevronDown, Copy, Layers, PenLine, Plus, Trash2 } from '@lucide/svelte';
  import { contextMenu, keys, openMenu, type MenuEntry } from '../lib/contextmenu.svelte';
  import { currentSpace, duplicateSpace, orgUi, spaceSize, spaces, switchSpace } from '../lib/org.svelte';
  import { m } from '../paraglide/messages';

  let button = $state<HTMLButtonElement>();
  const space = $derived(currentSpace());
  const open = $derived(!!contextMenu.open && contextMenu.open.label === m.org_spaces());

  function show() {
    if (!button) return;
    const b = button.getBoundingClientRect();
    const w = currentSpace();
    const entries: MenuEntry[] = [
      { label: m.org_space_all(), icon: spaces.active === null ? Check : Layers, shortcut: keys('Ctrl+Alt+1'), hint: m.org_count_conns({ count: spaceSize(null) }), run: () => switchSpace(null) },
      ...spaces.list.map((x, i) => ({
        label: x.name,
        icon: x.id === spaces.active ? Check : undefined,
        shortcut: i < 8 ? keys(`Ctrl+Alt+${i + 2}`) : undefined,
        hint: m.org_count_conns({ count: spaceSize(x) }),
        run: () => switchSpace(x.id),
      })),
      'separator',
      { label: m.org_space_new_dots(), icon: Plus, run: () => (orgUi.space = { id: null, preset: null }) },
      w && { label: m.org_space_edit({ name: w.name }), icon: PenLine, run: () => (orgUi.space = { id: w.id, preset: null }) },
      w && { label: m.org_space_duplicate(), icon: Copy, run: () => duplicateSpace(w.id) },
      w && { label: m.org_space_delete_dots(), icon: Trash2, danger: true, run: () => (orgUi.space = { id: w.id, preset: null, remove: true }) },
    ];
    openMenu({ x: b.left, y: b.bottom + 4 }, entries, m.org_spaces());
  }
</script>

<button class="space-switch" class:scoped={!!space} type="button" aria-haspopup="menu" aria-expanded={open} data-tip={m.org_space_switch()} bind:this={button} onclick={show}>
  <Layers size={14} strokeWidth={1.7} aria-hidden="true" />
  <span class="space-name">{space?.name ?? m.org_space_all()}</span>
  <ChevronDown size={13} strokeWidth={1.8} aria-hidden="true" />
</button>
