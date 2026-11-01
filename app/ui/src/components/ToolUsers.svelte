<script lang="ts">
  import { isSql, sqlOf } from '../lib/family';
  import { quotaCap, quotaWindow } from '../lib/quotas';
  import { KeyRound, Pencil, RefreshCw, ShieldCheck, Trash2, UserPlus, Users } from '@lucide/svelte';
  import { api, ApiError, type Principals } from '../lib/api';
  import type { Grant_Serialize as Grant, Principal_Serialize as Principal, RoleAttribute } from '../lib/bindings';
  import type { UserAction } from '../lib/tools';
  import { errorText } from '../lib/tools.svelte';
  import type { sessionFor, Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import UserSqlDialog from './UserSqlDialog.svelte';
  import ClickhouseAccess from './ClickhouseAccess.svelte';
  import type { ClickhouseAccess_Serialize as Access } from '../lib/bindings';
  import { registerCommands } from '../lib/palette.svelte';
  import { usersFocus } from '../lib/explorer-state.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  let data = $state.raw<Principals | null>(null);
  let failure = $state<string | null>(null);
  let unsupported = $state(false);
  let picked = $state<string | null>(null);
  let filter = $state('');
  let action = $state<UserAction | null>(null);
  let access = $state.raw<Access | null>(null);

  const key = (p: Principal) => (p.host ? `${p.name}@${p.host}` : p.name);
  const dialect = $derived(isSql(session.info.caps) ? sqlOf(session.info) : null);
  const either = $derived(dialect === null || dialect === 'clickhouse');
  const quotasOf = (p: Principal) => (data?.quotas ?? []).filter((q) => (q.all ? !q.except.includes(p.name) : q.applies_to.includes(p.name)));

  async function load() {
    try {
      data = await api.principalsList(session.info.session_id);
      failure = null;
      if (dialect === 'clickhouse') access = await api.clickhouseAccess(session.info.session_id).catch(() => null);
      if (!picked || !data.principals.some((p) => key(p) === picked)) picked = data.principals.map(key).find((k) => k !== 'postgres') ?? null;
    } catch (e) {
      unsupported = e instanceof ApiError && e.error.code === 'tool' && e.error.detail.kind === 'unsupported';
      failure = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  $effect(() => {
    const want = usersFocus.key;
    if (!want || usersFocus.connectionId !== tab.connectionId || !data?.principals.some((p) => key(p) === want)) return;
    picked = want;
    filter = '';
    usersFocus.key = '';
  });

  $effect(() =>
    registerCommands([
      { id: 'users.new', label: () => (dialect === 'mysql' || dialect === 'mssql' ? m.tool_user_new() : either ? m.tool_principal_new() : m.tool_role_new()), group: m.pal_group_tools, when: () => !!data && !action, run: () => (action = { mode: 'create' }) },
      { id: 'users.grant', label: m.tool_grant, group: m.pal_group_tools, when: () => !!current && !action, run: () => current && (action = { mode: 'grant', principal: current }) },
    ]),
  );

  const list = $derived((data?.principals ?? []).filter((p) => !filter.trim() || key(p).toLowerCase().includes(filter.trim().toLowerCase())));
  const current = $derived(data?.principals.find((p) => key(p) === picked) ?? null);
  const grants = $derived(current ? (data?.grants ?? []).filter((g) => g.grantee === key(current)) : []);
  const publicGrants = $derived((data?.grants ?? []).filter((g) => g.grantee === 'PUBLIC'));

  const ATTR: Record<RoleAttribute, () => string> = {
    login: m.tool_attr_login,
    superuser: m.tool_attr_superuser,
    create_db: m.tool_attr_create_db,
    create_role: m.tool_attr_create_role,
    locked: m.tool_attr_locked,
  };
  const KIND: Record<string, () => string> = {
    global: m.tool_kind_global,
    database: m.tool_kind_database,
    schema: m.tool_kind_schema,
    table: m.tool_kind_table,
    view: m.tool_kind_view,
    sequence: m.tool_kind_sequence,
  };

  function byObject(list: Grant[]) {
    const out = new Map<string, { kind: string; label: string; grants: Grant[] }>();
    for (const g of list) {
      const label = g.kind === 'global' ? '*.*' : [g.schema, g.object].filter(Boolean).join('.');
      const k = `${g.kind}:${label}`;
      const row = out.get(k) ?? { kind: g.kind, label, grants: [] };
      row.grants.push(g);
      out.set(k, row);
    }
    return [...out.values()];
  }

  const isRole = (p: Principal) => !p.attributes.includes('login');
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_users()}</h2>
    {#if data}<p class="hint">{m.tool_users_summary({ count: data.principals.length })}</p>{/if}
  </div>
  <span class="spacer"></span>
  {#if data}
    <button class="btn" type="button" onclick={() => (action = { mode: 'create' })}><UserPlus size={13} strokeWidth={1.8} aria-hidden="true" /> {dialect === 'mysql' || dialect === 'mssql' ? m.tool_user_new() : either ? m.tool_principal_new() : m.tool_role_new()}</button>
    <button class="icon-btn" type="button" aria-label={m.schema_refresh()} onclick={() => void load()}><RefreshCw size={13} strokeWidth={1.8} /></button>
  {/if}
</header>

{#if unsupported}
  <p class="tool-empty">{m.tool_users_sqlite()}</p>
{:else if failure && !data}
  <p class="tool-empty err">{failure}</p>
{:else if data}
  {#if data.partial}<p class="tool-note">{m.tool_users_partial()}</p>{/if}
  <div class="users">
    <div class="users-list">
      <label class="field users-filter"><input type="search" placeholder={m.tool_users_filter()} aria-label={m.tool_users_filter()} bind:value={filter} /></label>
      <ul role="listbox" aria-label={m.tool_users()}>
        {#each list as p (key(p))}
          <li>
            <button type="button" role="option" aria-selected={key(p) === picked} class="users-item" onclick={() => (picked = key(p))}>
              {#if isRole(p)}<Users size={13} strokeWidth={1.7} aria-hidden="true" />{:else}<KeyRound size={13} strokeWidth={1.7} aria-hidden="true" />{/if}
              <span class="users-name">{p.name}{#if p.host}<span class="sub">@{p.host}</span>{/if}</span>
              {#if p.attributes.includes('superuser')}<span class="tool-tag bad">{m.tool_attr_superuser_short()}</span>{/if}
              {#if p.attributes.includes('locked')}<span class="tool-tag warn">{m.tool_attr_locked()}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
    <div class="users-detail">
      {#if current}
        <div class="users-head">
          <h3 class="users-title">{current.name}{#if current.host}<span class="sub">@{current.host}</span>{/if}</h3>
          <span class="spacer"></span>
          <button class="btn ghost" type="button" onclick={() => (action = { mode: 'alter', principal: current })}><Pencil size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_role_edit_short()}</button>
          <button class="btn ghost" type="button" onclick={() => (action = { mode: 'grant', principal: current })}><ShieldCheck size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_grant_short()}</button>
          <button class="btn ghost danger-text" type="button" onclick={() => (action = { mode: 'drop', principal: current })}><Trash2 size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_role_drop_short()}</button>
        </div>
        <div class="tool-chips static">
          {#each current.attributes as a (a)}<span class="tool-chip on">{ATTR[a]()}</span>{:else}<span class="tool-chip">{m.tool_attr_none()}</span>{/each}
        </div>
        {#if current.member_of.length}<p class="users-member"><span class="caps">{m.tool_member_of()}</span> {current.member_of.join(', ')}</p>{/if}
        <h4 class="caps tool-sub">{m.tool_grants()}</h4>
        <table class="tool-table grants">
          <thead>
            <tr>
              <th>{m.tool_col_object()}</th>
              <th>{m.tool_privileges()}</th>
              <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
            </tr>
          </thead>
          <tbody>
            {#each byObject(grants) as row (row.kind + row.label)}
              {@const revocable = row.grants.filter((g) => g.privilege !== 'OWNER')}
              <tr>
                <td><span class="sub">{KIND[row.kind]?.() ?? row.kind}</span> <span class="mono">{row.label}</span></td>
                <td>
                  <span class="tool-privs">
                    {#each row.grants as g (g.privilege)}<span class="tool-priv" class:owner={g.privilege === 'OWNER'}>{g.privilege === 'OWNER' ? m.tool_owner() : g.privilege}{g.grantable ? '*' : ''}</span>{/each}
                  </span>
                </td>
                <td class="acts">
                  {#if revocable.length}<button class="btn ghost small" type="button" onclick={() => (action = { mode: 'revoke', principal: current, grants: revocable })}>{m.tool_revoke_short()}</button>{/if}
                </td>
              </tr>
            {:else}
              <tr><td colspan="3" class="tool-none">{m.tool_no_grants()}</td></tr>
            {/each}
          </tbody>
        </table>
        {#if dialect === 'clickhouse'}
          <ClickhouseAccess principal={current} quotas={data.quotas} {access} schema={session.schema} connectionId={tab.connectionId} onchanged={() => void load()} />
        {:else if quotasOf(current).length}
          <h4 class="caps tool-sub">{m.tool_quotas()}</h4>
          <table class="tool-table quotas">
            <thead>
              <tr>
                <th>{m.tool_quota()}</th>
                <th>{m.tool_quota_window()}</th>
                <th>{m.tool_quota_caps()}</th>
              </tr>
            </thead>
            <tbody>
              {#each quotasOf(current) as q (q.name)}
                {#each q.limits.length ? q.limits : [null] as l, i (i)}
                  <tr>
                    <td class="mono">{i === 0 ? q.name : ''}</td>
                    <td>{l && l.seconds !== null ? quotaWindow(l.seconds) : '—'}</td>
                    <td>
                      <span class="tool-privs">
                        {#each l?.caps ?? [] as c (c.what)}<span class="tool-priv">{quotaCap(c)}</span>{:else}<span class="sub">{m.tool_quota_none()}</span>{/each}
                      </span>
                    </td>
                  </tr>
                {/each}
              {/each}
            </tbody>
          </table>
        {/if}
        {#if publicGrants.length}<p class="hint tool-foot">{m.tool_public_grants({ list: publicGrants.map((g) => `${g.privilege} ${g.object ?? g.schema ?? ''}`.trim()).join(', ') })}</p>{/if}
      {:else}
        <p class="tool-empty">{m.tool_users_pick()}</p>
      {/if}
    </div>
  </div>
{:else}
  <p class="tool-empty">{m.schema_loading()}</p>
{/if}

{#if action && data}
  <UserSqlDialog {action} {dialect} product={session.info.server.product} edition={session.info.edition?.flavor ?? null} connectionId={tab.connectionId} principals={data.principals} schema={session.schema} onclose={() => (action = null)} ondone={() => void load()} />
{/if}
