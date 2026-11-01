<script lang="ts">
  import { Plus } from '@lucide/svelte';
  import type { FilterOp, QuickFilter } from '../lib/bindings';
  import { m } from '../paraglide/messages';
  import Popover from './Popover.svelte';
  import { isDateType } from '../lib/datepick';
  import DateField from './DateField.svelte';
  import Select from './Select.svelte';

  interface Props {
    columns: { name: string; type_name: string }[];
    suggest: { column: string; value: string | null } | null;
    onadd: (f: QuickFilter) => void;
    open?: boolean;
  }
  let { columns, suggest, onadd, open = $bindable(false) }: Props = $props();
  let column = $state('');
  let op = $state<FilterOp>('eq');
  let value = $state('');

  const OPS: { value: FilterOp; label: () => string }[] = [
    { value: 'eq', label: m.op_eq },
    { value: 'ne', label: m.op_ne },
    { value: 'contains', label: m.op_contains },
    { value: 'lt', label: m.op_lt },
    { value: 'le', label: m.op_le },
    { value: 'gt', label: m.op_gt },
    { value: 'ge', label: m.op_ge },
    { value: 'is_null', label: m.op_is_null },
    { value: 'not_null', label: m.op_not_null },
  ];
  const unary = $derived(op === 'is_null' || op === 'not_null');
  const dated = $derived(isDateType(columns.find((c) => c.name === column)?.type_name) && op !== 'contains');

  $effect(() => {
    if (!open) return;
    column = suggest?.column ?? columns[0]?.name ?? '';
    op = suggest?.value === null ? 'is_null' : 'eq';
    value = suggest?.value ?? '';
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!column) return;
    onadd({ column, op, value: unary ? null : value });
    open = false;
  }
</script>

<Popover label={m.data_filter_add()} bind:open class="btn ghost dv-filter-add">
  {#snippet trigger()}
    <Plus size={13} strokeWidth={1.8} aria-hidden="true" />
    {m.data_filter_add()}
  {/snippet}
  <form class="dv-filter-form" onsubmit={submit}>
    <label class="form-field">
      <span class="caps">{m.data_filter_column()}</span>
      <Select class="field mono" label={m.data_filter_column()} bind:value={column} options={columns.map((c) => ({ value: c.name, label: c.name }))} />
    </label>
    <label class="form-field">
      <span class="caps">{m.data_filter_op()}</span>
      <Select class="field" label={m.data_filter_op()} bind:value={op} options={OPS.map((o) => ({ value: o.value, label: o.label() }))} />
    </label>
    {#if dated && !unary}
      <div class="form-field">
        <span class="caps" aria-hidden="true">{m.data_filter_value()}</span>
        <DateField bind:value label={m.data_filter_value()} />
      </div>
    {:else if !unary}
      <label class="form-field">
        <span class="caps">{m.data_filter_value()}</span>
        <span class="field"><input bind:value spellcheck="false" /></span>
      </label>
    {/if}
    <button class="btn primary" type="submit">{m.data_filter_ok()}</button>
  </form>
</Popover>
