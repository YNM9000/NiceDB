<script lang="ts">
  import { Plus } from '@lucide/svelte';
  import DriverMark from '../components/DriverMark.svelte';
  import { DRIVERS, type DriverKind } from '../lib/drivers';
  import { m } from '../paraglide/messages';

  let { onnew, hasConnections }: { onnew: () => void; hasConnections: boolean } = $props();

  const kindLabel: Record<DriverKind, () => string> = {
    relational: m.kind_relational,
    document: m.kind_document,
    keyvalue: m.kind_keyvalue,
    analytical: m.kind_analytical,
    widecolumn: m.kind_widecolumn,
    search: m.kind_search,
  };
</script>

<section class="screen connections">
  <div class="empty">
    <h2 class="display">{hasConnections ? m.conn_saved() : m.conn_empty_title()}</h2>
    <p>{hasConnections ? m.conn_pick() : m.conn_empty_body()}</p>
    <button class="btn primary" type="button" onclick={onnew}>
      <Plus size={15} strokeWidth={2} aria-hidden="true" />
      {m.conn_new()}
    </button>
  </div>

  <div class="drivers">
    <h3 class="caps">{m.drivers_title()}</h3>
    <ul>
      {#each DRIVERS as driver (driver.id)}
        <li class="driver">
          <DriverMark {driver} />
          <span class="driver-name">{driver.name}</span>
          <span class="driver-meta">
            {kindLabel[driver.kind]()} · {driver.where === 'file' ? m.where_file() : driver.where === 'cloud' ? m.where_cloud() : m.where_server()}
          </span>
        </li>
      {/each}
    </ul>
  </div>
</section>
