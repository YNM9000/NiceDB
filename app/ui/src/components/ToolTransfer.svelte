<script lang="ts">
  import { FileDown, FileUp } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { askExport, askImport } from '../lib/tools.svelte';
  import type { sessionFor, Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Select from './Select.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const tables = $derived((session.schema?.namespaces ?? []).flatMap((n) => [...n.tables, ...n.views].map((t) => ({ ns: n.name, name: t.name, rows: t.estimated_rows }))));
  let picked = $state(untrack(() => (tab.table ? `${tab.table.namespace}.${tab.table.name}` : '')));

  $effect(() => {
    if (!tables.some((t) => `${t.ns}.${t.name}` === picked)) picked = tables[0] ? `${tables[0].ns}.${tables[0].name}` : '';
  });

  const chosen = $derived(tables.find((t) => `${t.ns}.${t.name}` === picked));
  const many = $derived((session.schema?.namespaces.length ?? 1) > 1);
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_transfer()}</h2>
    <p class="hint">{m.tool_transfer_lead()}</p>
  </div>
</header>

<div class="tool-scroll">
  <div class="bk-cards">
    <section class="bk-card" aria-labelledby="tr-export">
      <h3 id="tr-export" class="bk-title"><FileDown size={15} strokeWidth={1.7} aria-hidden="true" /> {m.tool_export_title()}</h3>
      <p class="hint">{m.tool_export_card()}</p>
      <label class="form-field">
        <span class="caps">{m.tool_kind_table()}</span>
        <Select class="tool-select" label={m.tool_kind_table()} bind:value={picked} disabled={!tables.length} options={tables.map((t) => ({ value: `${t.ns}.${t.name}`, label: many ? `${t.ns}.${t.name}` : t.name }))} />
      </label>
      <div class="bk-actions">
        <button
          class="btn primary"
          type="button"
          disabled={!chosen}
          onclick={() => chosen && askExport({ connectionId: tab.connectionId, source: { kind: 'table', namespace: chosen.ns, name: chosen.name }, label: chosen.name, expected: chosen.rows })}
          >{m.tool_export_dots()}</button
        >
      </div>
    </section>
    <section class="bk-card" aria-labelledby="tr-import">
      <h3 id="tr-import" class="bk-title"><FileUp size={15} strokeWidth={1.7} aria-hidden="true" /> {m.tool_import_title()}</h3>
      <p class="hint">{m.tool_import_card()}</p>
      <ul class="tr-formats">
        <li><b>CSV · TSV</b> {m.tool_fmt_text()}</li>
        <li><b>JSON</b> {m.tool_fmt_json()}</li>
        <li><b>XLSX</b> {m.tool_fmt_xlsx()}</li>
        <li><b>Parquet</b> {m.tool_fmt_parquet()}</li>
      </ul>
      <div class="bk-actions">
        <button class="btn primary" type="button" onclick={() => askImport({ connectionId: tab.connectionId, table: null })}>{m.tool_import_dots()}</button>
      </div>
    </section>
  </div>
</div>
