<script lang="ts">
  import { Check, ChevronDown, History, Play, TriangleAlert } from '@lucide/svelte';
  import { paramKindLabel } from '../lib/api';
  import { openMenu } from '../lib/contextmenu.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { fits, historyOf, isOn, KINDS, slotLabel, typed, type ParamEntry } from '../lib/params';
  import { clearParams, paramsOf } from '../lib/params.svelte';
  import type { ParamKind, ParamSlot } from '../lib/bindings';
  import { m } from '../paraglide/messages';

  let { tabId, onrun, onleave }: { tabId: string; onrun: () => void; onleave: () => void } = $props();

  const p = $derived(paramsOf(tabId));
  const slots = $derived(p.slots);
  const anyRaw = $derived(slots.some((s) => p.entries[s.name]?.kind === 'raw'));
  let bar = $state<HTMLDivElement>();

  const blank: ParamEntry = { kind: 'text', value: '', chosen: false };
  const entryOf = (s: ParamSlot) => p.entries[s.name] ?? blank;

  function set(s: ParamSlot, next: ParamEntry) {
    p.entries[s.name] = next;
    if (p.fault?.name === s.name) p.fault = null;
  }

  function hint(kind: ParamKind) {
    return kind === 'date' ? m.params_date_hint() : kind === 'raw' ? m.params_raw_hint() : '';
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.isComposing) {
      e.preventDefault();
      onrun();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onleave();
    }
  }

  function anchor(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: r.left, y: r.bottom + 4 };
  }

  function pickKind(e: MouseEvent, s: ParamSlot) {
    const cur = entryOf(s);
    openMenu(
      anchor(e),
      KINDS.map((kind) => ({
        label: paramKindLabel(kind),
        icon: kind === cur.kind ? Check : undefined,
        hint: kind === 'raw' ? m.params_raw_hint() : undefined,
        run: () => set(s, { ...cur, kind, value: kind === 'boolean' ? String(isOn(cur.value)) : cur.value, chosen: true }),
      })),
      m.params_kind_of({ name: slotLabel(s) }),
    );
  }

  function pickPast(e: MouseEvent, s: ParamSlot) {
    const past = historyOf(s.name, p.history);
    const item = (h: (typeof past)[number]) => ({
      label: h.kind === 'null' ? 'NULL' : h.value || ' ',
      hint: paramKindLabel(h.kind),
      run: () => set(s, { kind: h.kind, value: h.value, chosen: true }),
    });
    const here = past.filter((h) => h.here).map(item);
    const other = past.filter((h) => !h.here).map(item);
    openMenu(
      anchor(e),
      past.length ? [...(here.length ? [{ label: m.params_history_here(), disabled: true }, ...here] : []), ...(other.length ? ['separator' as const, { label: m.params_history_other(), disabled: true }, ...other] : [])] : [{ label: m.params_history_empty(), disabled: true }],
      m.params_history_of({ name: slotLabel(s) }),
    );
  }

  function width(value: string, kind: ParamKind) {
    const base = kind === 'date' ? 11 : 9;
    return `calc(${Math.min(30, Math.max(base, value.length + 1))}ch + 18px)`;
  }

  export function focus() {
    const first = p.fault?.name ?? p.asked[0];
    const target = (first && bar?.querySelector<HTMLElement>(`[data-param="${CSS.escape(first)}"]`)) || bar?.querySelector<HTMLElement>('[data-param]');
    target?.focus();
  }

  $effect(() => {
    if (p.asked.length || p.fault) queueMicrotask(focus);
  });

  $effect(() =>
    registerCommands([
      { id: 'query.params_focus', label: m.pal_params_focus, group: m.pal_group_query, when: () => slots.length > 0, run: () => setTimeout(focus, 0) },
      { id: 'query.params_clear', label: m.pal_params_clear, group: m.pal_group_query, when: () => slots.length > 0, run: () => clearParams(tabId) },
    ]),
  );
</script>

{#if slots.length}
  <div class="params-bar" class:asking={p.asked.length > 0} bind:this={bar} role="group" aria-label={m.params_title()}>
    <div class="prm-row">
      <span class="prm-title caps">{m.params_title()}</span>
      <div class="prm-fields">
        {#each slots as s (s.name)}
          {@const e = entryOf(s)}
          {@const bad = !fits(e) || p.fault?.name === s.name}
          {@const past = historyOf(s.name, p.history).length > 0}
          <div class="prm-field" class:asked={p.asked.includes(s.name)} class:bad class:raw={e.kind === 'raw'} data-tip={bad ? m.params_bad({ kind: paramKindLabel(e.kind).toLowerCase() }) : undefined}>
            <span class="prm-name">{slotLabel(s)}</span>
            {#if e.kind === 'boolean'}
              <button
                class="prm-bool"
                type="button"
                data-param={s.name}
                role="switch"
                aria-checked={isOn(e.value)}
                aria-label={m.params_value_of({ name: slotLabel(s) })}
                onclick={() => set(s, { ...e, value: String(!isOn(e.value)) })}
                onkeydown={key}
              >
                <span class:on={isOn(e.value)}>{m.params_true()}</span><span class:on={!isOn(e.value)}>{m.params_false()}</span>
              </button>
            {:else if e.kind === 'null'}
              <button class="prm-null" type="button" data-param={s.name} aria-label={m.params_value_of({ name: slotLabel(s) })} onclick={() => set(s, { ...e, kind: 'text', chosen: true })} onkeydown={key}>NULL</button>
            {:else}
              <input
                class="prm-input"
                data-param={s.name}
                value={e.value}
                spellcheck="false"
                autocomplete="off"
                placeholder={hint(e.kind)}
                aria-label={m.params_value_of({ name: slotLabel(s) })}
                aria-invalid={bad}
                style:width={width(e.value, e.kind)}
                oninput={(ev) => set(s, typed(e, ev.currentTarget.value))}
                onkeydown={key}
              />
            {/if}
            {#if past}
              <button class="prm-past" type="button" data-tip={m.params_history_of({ name: slotLabel(s) })} aria-label={m.params_history_of({ name: slotLabel(s) })} onclick={(ev) => pickPast(ev, s)}>
                <History size={12} strokeWidth={1.8} aria-hidden="true" />
              </button>
            {/if}
            <button class="prm-kind" type="button" aria-haspopup="menu" aria-label={`${m.params_kind_of({ name: slotLabel(s) })}: ${paramKindLabel(e.kind)}`} onclick={(ev) => pickKind(ev, s)}>
              {paramKindLabel(e.kind)}
              <ChevronDown size={11} strokeWidth={2} aria-hidden="true" />
            </button>
          </div>
        {/each}
      </div>
      <span class="prm-enter">{m.params_enter()}</span>
      <button class="btn ghost prm-go" type="button" onclick={onrun}>
        <Play size={12} strokeWidth={2} aria-hidden="true" />
        {m.params_run()}
      </button>
    </div>
    {#if anyRaw || p.asked.length || p.fault}
      <div class="prm-notes">
        {#if p.fault}<p class="prm-note err" role="alert">{p.fault.message}</p>
        {:else if p.asked.length}<p class="prm-note ask" role="status">{m.params_asked()}</p>{/if}
        {#if anyRaw}<p class="prm-note warn"><TriangleAlert size={13} strokeWidth={1.9} aria-hidden="true" />{m.params_raw_warning()}</p>{/if}
      </div>
    {/if}
  </div>
{/if}
