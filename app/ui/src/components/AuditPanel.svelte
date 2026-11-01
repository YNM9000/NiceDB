<script lang="ts">
  import { Download, X } from '@lucide/svelte';
  import { api, type AuditEntry } from '../lib/api';
  import { auditCsv, auditFile, auditJson } from '../lib/audit';
  import { connections } from '../lib/connections.svelte';
  import { num, when } from '../lib/format';
  import { m } from '../paraglide/messages';
  import Menu from './Menu.svelte';
  import Resizer from './Resizer.svelte';

  let { connectionId, rev, onclose }: { connectionId: string; rev: number; onclose: () => void } = $props();
  let entries = $state<AuditEntry[]>([]);
  let failure = $state<string | null>(null);

  async function exportAs(kind: 'csv' | 'json') {
    failure = null;
    try {
      const all = await api.auditList(connectionId, 100_000);
      const text = kind === 'csv' ? auditCsv(all) : auditJson(all);
      const name = connections.list.find((c) => c.id === connectionId)?.name ?? connectionId;
      await api.exportSave(auditFile(name, kind), new TextEncoder().encode(text));
    } catch (e) {
      failure = m.audit_export_failed({ message: e instanceof Error ? e.message : String(e) });
    }
  }

  $effect(() => {
    void rev;
    void api
      .auditList(connectionId, 200)
      .then((e) => (entries = e))
      .catch(() => (entries = []));
  });
</script>

<aside class="history audit" aria-label={m.audit_title()}>
  <Resizer pane="audit" side="start" label={m.resize_audit()} />
  <header class="history-head">
    <h2 class="caps">{m.audit_title()}</h2>
    <span class="spacer"></span>
    <Menu
      label={m.audit_export()}
      disabled={entries.length === 0}
      items={[
        { value: 'csv', label: m.audit_export_csv() },
        { value: 'json', label: m.audit_export_json() },
      ] as const}
      onpick={(v) => void exportAs(v)}
    >
      <Download size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.audit_export()}
    </Menu>
    <button class="icon-btn" type="button" aria-label={m.audit_close()} onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>
  {#if failure}<p class="audit-failure" role="alert">{failure}</p>{/if}
  {#if entries.length === 0}
    <p class="history-empty">{m.audit_empty()}</p>
  {:else}
    <ul class="audit-list">
      {#each entries as e (e.id)}
        <li class:err={!!e.error}>
          <span class="audit-meta">{when(e.at ?? 0)} · {e.who || '—'}{#if e.database} · {e.database}{/if}{#if e.rows !== null} · {e.rows === 1 ? m.audit_row_one() : m.audit_rows({ count: num(e.rows) })}{/if}</span>
          <code>{e.sql.length > 400 ? `${e.sql.slice(0, 400)}…` : e.sql}</code>
          {#if e.error}<span class="audit-err">{e.error}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</aside>
