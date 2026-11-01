<script lang="ts">
  import { Pencil, Plus, Trash2 } from '@lucide/svelte';
  import type { ClickhouseAccess_Serialize as ClickhouseAccess, Principal_Serialize as Principal, ProfileSetting_Serialize as ProfileSetting, Quota } from '../lib/bindings';
  import type { Schema } from '../lib/api';
  import { appliesTo, policyAlter, policyCreate, policyDrop, profileAlter, profileCreate, profileDrop, quotaAlter, quotaCreate, quotaDrop } from '../lib/ch-access';
  import { quotaCap, quotaWindow } from '../lib/quotas';
  import { m } from '../paraglide/messages';
  import AccessSqlDialog from './AccessSqlDialog.svelte';

  interface Props {
    principal: Principal;
    quotas: Quota[];
    access: ClickhouseAccess | null;
    schema: Schema | null;
    connectionId: string;
    onchanged: () => void;
  }
  let { principal, quotas, access, schema, connectionId, onchanged }: Props = $props();

  let open = $state<{ title: string; subject: string | null; sql: string; warn: string | null; danger: boolean } | null>(null);

  const mineQuotas = $derived(quotas.filter((q) => appliesTo(q, principal.name)));
  const profiles = $derived((access?.profiles ?? []).filter((p) => appliesTo(p, principal.name)));
  const policies = $derived((access?.policies ?? []).filter((r) => appliesTo(r, principal.name)));
  const firstTable = $derived.by(() => {
    const ns = schema?.namespaces.find((n) => n.tables.length) ?? null;
    return { database: ns?.name ?? schema?.current_database ?? 'default', table: ns?.tables[0]?.name ?? 'table' };
  });

  const ask = (title: string, subject: string | null, sql: string, drop = false) => (open = { title, subject, sql, warn: drop ? m.chq_drop_note() : null, danger: drop });

  function settingText(s: ProfileSetting): string {
    const bounds = [s.min !== null ? `≥ ${s.min}` : null, s.max !== null ? `≤ ${s.max}` : null].filter(Boolean).join(' ');
    return [s.value !== null ? `${s.name} = ${s.value}` : s.name, bounds].filter(Boolean).join(' ');
  }
</script>

<div class="tool-subhead">
  <h4 class="caps tool-sub">{m.tool_quotas()}</h4>
  <span class="spacer"></span>
  <button class="btn ghost small" type="button" onclick={() => ask(m.chq_quota_new(), principal.name, quotaCreate(m.chq_quota_name(), principal.name))}><Plus size={12} strokeWidth={2} aria-hidden="true" /> {m.chq_quota_new()}</button>
</div>
<table class="tool-table quotas">
  <thead>
    <tr>
      <th>{m.tool_quota()}</th>
      <th>{m.tool_quota_window()}</th>
      <th>{m.tool_quota_caps()}</th>
      <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
    </tr>
  </thead>
  <tbody>
    {#each mineQuotas as q (q.name)}
      {#each q.limits.length ? q.limits : [null] as l, i (i)}
        <tr>
          <td class="mono">{i === 0 ? q.name : ''}</td>
          <td>{l && l.seconds !== null ? quotaWindow(l.seconds) : '—'}</td>
          <td>
            <span class="tool-privs">
              {#each l?.caps ?? [] as c (c.what)}<span class="tool-priv">{quotaCap(c)}</span>{:else}<span class="sub">{m.tool_quota_none()}</span>{/each}
            </span>
          </td>
          <td class="acts">
            {#if i === 0}
              <button class="icon-btn" type="button" aria-label={m.chq_edit()} data-tip={m.chq_edit()} onclick={() => ask(m.chq_quota_edit(), q.name, quotaAlter(q))}><Pencil size={13} strokeWidth={1.8} /></button>
              <button class="icon-btn danger" type="button" aria-label={m.chq_drop()} data-tip={m.chq_drop()} onclick={() => ask(m.chq_quota_drop(), q.name, quotaDrop(q), true)}><Trash2 size={13} strokeWidth={1.8} /></button>
            {/if}
          </td>
        </tr>
      {/each}
    {:else}
      <tr><td colspan="4" class="tool-none">{m.chq_no_quotas()}</td></tr>
    {/each}
  </tbody>
</table>

<div class="tool-subhead">
  <h4 class="caps tool-sub">{m.chq_profiles()}</h4>
  <span class="spacer"></span>
  <button class="btn ghost small" type="button" onclick={() => ask(m.chq_profile_new(), principal.name, profileCreate(m.chq_profile_name(), principal.name))}><Plus size={12} strokeWidth={2} aria-hidden="true" /> {m.chq_profile_new()}</button>
</div>
<table class="tool-table ch-profiles">
  <thead>
    <tr>
      <th>{m.chq_profile()}</th>
      <th>{m.chq_settings()}</th>
      <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
    </tr>
  </thead>
  <tbody>
    {#each profiles as p (p.name)}
      <tr>
        <td class="mono">{p.name}{#if p.all}<span class="sub"> · {m.chq_everyone()}</span>{/if}</td>
        <td>
          <span class="tool-privs">
            {#each p.settings as s (s.name)}<span class="tool-priv">{settingText(s)}{#if s.writability}<span class="ch-lock">{s.writability}</span>{/if}</span>{/each}
            {#each p.inherits as i (i)}<span class="tool-priv inherit">{m.chq_inherits({ name: i })}</span>{/each}
            {#if !p.settings.length && !p.inherits.length}<span class="sub">{m.chq_no_settings()}</span>{/if}
          </span>
        </td>
        <td class="acts">
          <button class="icon-btn" type="button" aria-label={m.chq_edit()} data-tip={m.chq_edit()} onclick={() => ask(m.chq_profile_edit(), p.name, profileAlter(p))}><Pencil size={13} strokeWidth={1.8} /></button>
          <button class="icon-btn danger" type="button" aria-label={m.chq_drop()} data-tip={m.chq_drop()} onclick={() => ask(m.chq_profile_drop(), p.name, profileDrop(p), true)}><Trash2 size={13} strokeWidth={1.8} /></button>
        </td>
      </tr>
    {:else}
      <tr><td colspan="3" class="tool-none">{m.chq_no_profiles()}</td></tr>
    {/each}
  </tbody>
</table>

<div class="tool-subhead">
  <h4 class="caps tool-sub">{m.chq_policies()}</h4>
  <span class="spacer"></span>
  <button class="btn ghost small" type="button" onclick={() => ask(m.chq_policy_new(), principal.name, policyCreate(m.chq_policy_name(), principal.name, firstTable))}><Plus size={12} strokeWidth={2} aria-hidden="true" /> {m.chq_policy_new()}</button>
</div>
<table class="tool-table ch-policies">
  <thead>
    <tr>
      <th>{m.chq_policy()}</th>
      <th>{m.tool_kind_table()}</th>
      <th>{m.chq_condition()}</th>
      <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
    </tr>
  </thead>
  <tbody>
    {#each policies as r (`${r.database}.${r.table}.${r.name}`)}
      <tr>
        <td class="mono">{r.name}</td>
        <td class="mono">{r.database}.{r.table}</td>
        <td><code class="ch-filter">{r.filter ?? '—'}</code> <span class="tool-tag" class:warn={r.restrictive}>{r.restrictive ? m.chq_restrictive() : m.chq_permissive()}</span></td>
        <td class="acts">
          <button class="icon-btn" type="button" aria-label={m.chq_edit()} data-tip={m.chq_edit()} onclick={() => ask(m.chq_policy_edit(), r.name, policyAlter(r))}><Pencil size={13} strokeWidth={1.8} /></button>
          <button class="icon-btn danger" type="button" aria-label={m.chq_drop()} data-tip={m.chq_drop()} onclick={() => ask(m.chq_policy_drop(), r.name, policyDrop(r), true)}><Trash2 size={13} strokeWidth={1.8} /></button>
        </td>
      </tr>
    {:else}
      <tr><td colspan="4" class="tool-none">{m.chq_no_policies()}</td></tr>
    {/each}
  </tbody>
</table>
<p class="hint tool-foot">{m.chq_note()}</p>

{#if open}
  <AccessSqlDialog title={open.title} subject={open.subject} sql={open.sql} warn={open.warn} danger={open.danger} {connectionId} onclose={() => (open = null)} ondone={onchanged} />
{/if}
