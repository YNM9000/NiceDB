<script lang="ts">
  import type { Edition, Fact, Limit } from '../lib/bindings';
  import { DRIVERS } from '../lib/drivers';
  import { m } from '../paraglide/messages';
  import DriverMark from './DriverMark.svelte';

  let { edition }: { edition: Edition } = $props();

  const driver = $derived(DRIVERS.find((d) => d.id === edition.flavor) ?? null);
  const base = $derived(edition.facts.find((f) => f.kind === 'base')?.value ?? null);

  const LIMIT: Record<Limit, () => string> = {
    pooled_session: m.limit_pooled_session,
    scale_to_zero: m.limit_scale_to_zero,
    managed_schemas: m.limit_managed_schemas,
    foreign_keys_optional: m.limit_foreign_keys_optional,
    deploy_requests: m.limit_deploy_requests,
    no_routines: m.limit_no_routines,
    distributed_locks: m.limit_distributed_locks,
    own_dump_tool: m.limit_own_dump_tool,
    hypertable_dump: m.limit_hypertable_dump,
    dashboard_users: m.limit_dashboard_users,
    own_plans: m.limit_own_plans,
  };

  function fact(f: Fact): [string, string] | null {
    switch (f.kind) {
      case 'base':
        return null;
      case 'endpoint':
        return [m.ed_endpoint(), f.value];
      case 'branch':
        return [m.ed_branch(), f.value];
      case 'project':
        return [m.ed_project(), f.value];
      case 'region':
        return [m.ed_region(), f.value];
      case 'pooled':
        return [m.ed_pooler(), f.value === 'transaction' ? m.ed_pooled_transaction() : m.ed_pooled_session()];
      case 'nodes':
        return [m.ed_nodes(), String(f.value)];
      case 'store':
        return [m.ed_store(), f.value];
      case 'extension':
        return [f.value.name, f.value.version];
    }
  }

  const facts = $derived(edition.facts.map(fact).filter((f): f is [string, string] => f !== null));
</script>

<section class="edition" aria-label={m.ed_aria({ product: driver?.name ?? edition.flavor })}>
  <header class="edition-head">
    {#if driver}<span class="edition-mark"><DriverMark {driver} size={16} /></span>{/if}
    <span class="edition-name">{driver?.name ?? edition.flavor}</span>
    {#if edition.version}<span class="edition-version">{edition.version}</span>{/if}
  </header>
  {#if base}<p class="edition-base">{m.ed_on({ base })}</p>{/if}
  {#if facts.length}
    <dl class="edition-facts">
      {#each facts as [k, v] (k)}
        <dt>{k}</dt>
        <dd data-tip={v}>{v}</dd>
      {/each}
    </dl>
  {/if}
  {#if edition.limits.length}
    <h3 class="caps edition-sub">{m.ed_limits()}</h3>
    <ul class="edition-limits">
      {#each edition.limits as l (l)}<li>{LIMIT[l]()}</li>{/each}
    </ul>
  {/if}
</section>
