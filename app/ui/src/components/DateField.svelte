<script lang="ts" module>
  let uid = 0;
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import { clamp, dayLabel, formatDay, isDay, monthGrid, monthLabel, moveDay, outside, parseDay, same, today, weekdays, weekStart, withDay, addMonths, type Day } from '../lib/datepick';
  import { float } from '../lib/float';
  import { tag } from '../lib/format';
  import { ICON } from '../lib/icons';
  import { m } from '../paraglide/messages';

  interface Props {
    value: string;
    label: string;
    onchange?: (value: string) => void;
    min?: string;
    max?: string;
    strict?: boolean;
    placeholder?: string;
    class?: string;
    id?: string;
  }
  let { value = $bindable(), label, onchange, min, max, strict = false, placeholder = m.date_hint(), class: cls = '', id }: Props = $props();

  const key = ++uid;
  const locale = tag();
  const first = weekStart(locale);
  const names = weekdays(locale, first);
  const lo = $derived(parseDay(min));
  const hi = $derived(parseDay(max));

  let input = $state<HTMLInputElement>();
  let button = $state<HTMLButtonElement>();
  let grid = $state<HTMLTableElement>();
  let panel = $state<HTMLDivElement>();

  function portal(node: HTMLElement) {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }
  let open = $state(false);
  let active = $state<Day>(today());
  const now = today();
  const picked = $derived(parseDay(value));
  const weeks = $derived(monthGrid(active.y, active.m, first));
  const bad = $derived(!!value.trim() && !parseDay(value));

  async function show() {
    if (open) return;
    active = clamp(picked ?? now, lo, hi);
    open = true;
    await tick();
    focusActive();
  }

  function close(back = true) {
    if (!open) return;
    open = false;
    if (back) input?.focus();
  }

  function focusActive() {
    grid?.querySelector<HTMLButtonElement>(`[data-day="${formatDay(active)}"]`)?.focus();
  }

  async function go(next: Day) {
    active = clamp(next, lo, hi);
    await tick();
    focusActive();
  }

  function commit(next: string) {
    value = next;
    onchange?.(next);
  }

  function pick(day: Day) {
    if (outside(day, lo, hi)) return;
    commit(withDay(value, day));
    close();
  }

  function typed(e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const text = el.value.trim();
    if (strict && !isDay(text)) {
      el.value = value;
      return;
    }
    commit(text);
  }

  function inputKey(e: KeyboardEvent) {
    if ((e.altKey && e.key === 'ArrowDown') || (e.ctrlKey && e.key === ' ')) {
      e.preventDefault();
      void show();
    }
  }

  function gridKey(e: KeyboardEvent) {
    const next = moveDay(active, e.key, e.shiftKey, first);
    if (next) {
      e.preventDefault();
      void go(next);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      pick(active);
    }
  }

  function panelKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'Tab') {
      const stops = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not([disabled]):not([tabindex="-1"])')];
      const at = stops.indexOf(document.activeElement as HTMLElement);
      const to = e.shiftKey ? (at <= 0 ? stops.length - 1 : at - 1) : at === stops.length - 1 ? 0 : at + 1;
      e.preventDefault();
      stops[to]?.focus();
    }
  }

  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!panel?.contains(t) && !button?.contains(t)) close(false);
    };
    window.addEventListener('pointerdown', away, true);
    return () => window.removeEventListener('pointerdown', away, true);
  });

  const titleId = `ndd-${key}-title`;
  const Calendar = ICON.calendar;
  const Prev = ICON.prevMonth;
  const Next = ICON.nextMonth;
</script>

<span class="field ndd {cls}" class:open>
  <input
    bind:this={input}
    {id}
    class="mono"
    {value}
    {placeholder}
    aria-label={label}
    aria-invalid={bad || undefined}
    inputmode="numeric"
    spellcheck="false"
    autocomplete="off"
    onchange={typed}
    onkeydown={inputKey}
  />
  <button bind:this={button} class="ndd-open" type="button" aria-label={m.date_open({ field: label })} data-tip={m.date_open_tip()} aria-haspopup="dialog" aria-expanded={open} onclick={() => (open ? close() : void show())}>
    <Calendar size={13} strokeWidth={1.8} aria-hidden="true" />
  </button>
</span>

{#if open && button}
  <div
    bind:this={panel}
    class="ndd-cal float"
    role="dialog"
    aria-modal="false"
    aria-labelledby={titleId}
    tabindex="-1"
    use:portal
    use:float={{ anchor: button.parentElement ?? button, onaway: () => close(false) }}
    onkeydown={panelKey}
    onpointerdown={(e) => e.stopPropagation()}
  >
    <div class="ndd-head">
      <button class="ndd-step" type="button" aria-label={m.date_prev_month()} data-tip={m.date_prev_month()} onclick={() => void go(addMonths(active, -1))}><Prev size={14} strokeWidth={1.8} aria-hidden="true" /></button>
      <span class="ndd-title" id={titleId} aria-live="polite">{monthLabel(active.y, active.m, locale)}</span>
      <button class="ndd-step" type="button" aria-label={m.date_next_month()} data-tip={m.date_next_month()} onclick={() => void go(addMonths(active, 1))}><Next size={14} strokeWidth={1.8} aria-hidden="true" /></button>
    </div>
    <table bind:this={grid} class="ndd-grid" role="grid" aria-labelledby={titleId} onkeydown={gridKey}>
      <thead>
        <tr>
          {#each names as w (w.long)}<th scope="col" abbr={w.long}>{w.short}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each weeks as week, wi (wi)}
          <tr>
            {#each week as day (formatDay(day))}
              {@const off = outside(day, lo, hi)}
              {@const sel = same(day, picked)}
              <td role="gridcell" aria-selected={sel}>
                <button
                  type="button"
                  class="ndd-day"
                  class:other={day.m !== active.m}
                  class:today={same(day, now)}
                  class:sel
                  data-day={formatDay(day)}
                  tabindex={same(day, active) ? 0 : -1}
                  aria-label={dayLabel(day, locale)}
                  aria-current={same(day, now) ? 'date' : undefined}
                  disabled={off}
                  onclick={() => pick(day)}>{day.d}</button
                >
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="ndd-foot">
      <span class="ndd-keys" aria-hidden="true">{m.date_keys()}</span>
      <button class="btn ghost compact" type="button" disabled={outside(now, lo, hi)} onclick={() => pick(now)}>{m.date_today()}</button>
    </div>
  </div>
{/if}
