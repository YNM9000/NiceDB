<script lang="ts">
  import { m } from '../paraglide/messages';

  const rows = [
    { d: 'M2 9H44M33 9L44 3M33 9L44 15', label: m.er_leg_many },
    { d: 'M2 9H44M36 4V14', label: m.er_leg_one },
    { d: 'M2 9H44M33 4V14M37 4V14', label: m.er_leg_one_only },
    { d: 'M2 9H25.5M32.5 9H44M37 4V14', ring: true, label: m.er_leg_zero_one },
  ];
</script>

<details class="er-legend" open>
  <summary>{m.er_legend()}</summary>
  <ul>
    {#each rows as r (r.d)}
      <li>
        <svg width="46" height="18" viewBox="0 0 46 18" aria-hidden="true">
          <path d={r.d} fill="none" stroke="currentColor" stroke-width="1.2" />
          {#if r.ring}<circle cx="29" cy="9" r="3.5" fill="none" stroke="currentColor" stroke-width="1.2" />{/if}
        </svg>
        <span>{r.label()}</span>
      </li>
    {/each}
    <li><span class="er-leg-key"><u>id</u></span><span>{m.er_leg_pk()}</span></li>
    <li><span class="er-leg-key">FK</span><span>{m.er_leg_fk()}</span></li>
  </ul>
</details>
