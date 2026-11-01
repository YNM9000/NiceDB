<script lang="ts">
  import { clampWidth, keyWidth, maxWidth, PANES, type Pane } from '../lib/resize';
  import { workspace } from '../lib/workspace.svelte';

  let { pane, side, label }: { pane: Pane; side: 'start' | 'end'; label: string } = $props();

  let edge = $state<HTMLElement>();
  let now = $state(0);
  let limit = $state(0);
  let dragging = $state(false);

  const target = () => edge?.parentElement ?? null;
  const sign = $derived(side === 'end' ? 1 : -1);

  function measure() {
    const el = target();
    if (!el) return;
    const sidebar = document.querySelector<HTMLElement>('.sidebar');
    const sheet = document.querySelector<HTMLElement>('.sheet');
    const total = (sidebar?.offsetWidth ?? 0) + (sheet?.offsetWidth ?? window.innerWidth);
    let others = 0;
    for (const r of document.querySelectorAll<HTMLElement>('.resizer')) if (r !== edge && r.parentElement) others += r.parentElement.offsetWidth;
    now = el.offsetWidth;
    limit = maxWidth(pane, total, others);
  }

  $effect(() => {
    const el = target();
    if (!el) return;
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    const shell = el.closest('.shell');
    if (shell) ro.observe(shell);
    return () => ro.disconnect();
  });

  function set(width: number) {
    workspace.widths = { ...workspace.widths, [pane]: clampWidth(pane, width, limit) };
  }

  function down(e: PointerEvent) {
    const el = target();
    if (e.button !== 0 || !edge || !el) return;
    e.preventDefault();
    edge.focus();
    edge.setPointerCapture(e.pointerId);
    measure();
    const rect = el.getBoundingClientRect().width;
    const ratio = rect ? el.offsetWidth / rect : 1;
    const start = el.offsetWidth;
    const x0 = e.clientX;
    dragging = true;
    document.documentElement.classList.add('col-resizing');
    let frame = 0;
    const move = (ev: PointerEvent) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => set(start + (ev.clientX - x0) * ratio * sign));
    };
    const up = () => {
      cancelAnimationFrame(frame);
      dragging = false;
      document.documentElement.classList.remove('col-resizing');
      edge?.removeEventListener('pointermove', move);
      edge?.removeEventListener('pointerup', up);
      edge?.removeEventListener('pointercancel', up);
    };
    edge.addEventListener('pointermove', move);
    edge.addEventListener('pointerup', up);
    edge.addEventListener('pointercancel', up);
  }

  function key(e: KeyboardEvent) {
    measure();
    const next = keyWidth(pane, now, limit, e.key, e.shiftKey, side === 'end' ? 'ArrowRight' : 'ArrowLeft');
    if (next === null) return;
    e.preventDefault();
    set(next);
  }

  function reset() {
    const rest = { ...workspace.widths };
    delete rest[pane];
    workspace.widths = rest;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  bind:this={edge}
  class="resizer resizer-{side}"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={now}
  aria-valuemin={PANES[pane].min}
  aria-valuemax={limit}
  tabindex="0"
  data-pane={pane}
  onpointerdown={down}
  ondblclick={reset}
  onkeydown={key}
  onfocus={measure}
></div>
