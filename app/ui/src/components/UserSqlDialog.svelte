<script lang="ts">
  import { Play, TextCursorInput } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import type { Dialect, Flavor as Edition, Principal_Serialize as Principal } from '../lib/bindings';
  import type { Schema } from '../lib/api';
  import { alterRoleSql, createRoleSql, dropRoleSql, flavorOf, grantSql, variantOf, PRIVILEGES, revokeSql, type GrantKind, type RoleDraft, type UserAction } from '../lib/tools';
  import { runScript } from '../lib/tools.svelte';
  import { mongoAlter, mongoCreate, mongoDrop, mongoGrant, mongoRevoke } from '../lib/users-mongo';
  import { newTab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';

  interface Props {
    action: UserAction;
    dialect: Dialect | null;
    product: string;
    edition?: Edition | null;
    connectionId: string;
    principals: Principal[];
    schema: Schema | null;
    onclose: () => void;
    ondone: () => void;
  }
  let { action, dialect, product, edition = null, connectionId, principals, schema, onclose, ondone }: Props = $props();

  const mysql = $derived(dialect === 'mysql');
  const mssql = $derived(dialect === 'mssql');
  const mongo = $derived(dialect === null);
  const either = $derived(mongo || dialect === 'clickhouse');
  const account = $derived(mysql || mssql);
  const who = untrack(() => ('principal' in action ? action.principal : null));
  let draft = $state<RoleDraft>({
    name: who?.name ?? '',
    host: who?.host ?? untrack(() => (dialect === null ? (schema?.current_database ?? 'admin') : '%')),
    login: who ? untrack(() => (dialect === 'mysql' ? !who.attributes.includes('locked') : who.attributes.includes('login'))) : true,
    createDb: who?.attributes.includes('create_db') ?? false,
    createRole: who?.attributes.includes('create_role') ?? false,
    memberOf: [...(who?.member_of ?? [])],
  });
  const firstNs = untrack(() => schema?.namespaces[0]);
  let kind = $state<GrantKind>('table');
  let ns = $state(firstNs?.name ?? '');
  let object = $state(firstNs?.tables[0]?.name ?? '');
  let privileges = $state<string[]>([untrack(() => (dialect === null ? 'read' : 'SELECT'))]);
  let sql = $state('');
  let edited = $state(false);
  let busy = $state(false);
  let failure = $state<string | null>(null);
  let dialog: HTMLDialogElement;

  const variant = $derived(dialect === null ? 'postgres' : variantOf(dialect, product, edition));
  const family = $derived(dialect === null ? PRIVILEGES.mongodb : (PRIVILEGES[flavorOf(dialect, product) as keyof typeof PRIVILEGES] ?? PRIVILEGES.postgres));
  const kinds = $derived((['table', 'schema', 'database'] as const).filter((k) => family[k].length > 0 && !(mongo && k === 'table' && who?.attributes.includes('login'))));
  const space = $derived(schema?.namespaces.find((n) => n.name === ns));
  const objects = $derived([...(space?.tables ?? []), ...(space?.views ?? [])].map((r) => r.name));
  const roles = $derived(
    mongo
      ? [...PRIVILEGES.mongodb.database.map((r) => `${r}@${schema?.current_database ?? 'admin'}`), ...principals.filter((p) => !p.attributes.includes('login') && p.name !== draft.name).map((p) => `${p.name}@${p.host ?? 'admin'}`)]
      : principals.filter((p) => p.name !== draft.name && (mysql ? !p.attributes.includes('login') : !p.host && (dialect !== 'clickhouse' || !p.attributes.includes('login')))).map((p) => p.name),
  );

  const generated = $derived.by(() => {
    if (dialect === null) return mongoText();
    switch (action.mode) {
      case 'create':
        return draft.name.trim() ? createRoleSql(dialect, { ...draft, name: draft.name.trim() }, variant) : '';
      case 'alter':
        return alterRoleSql(dialect, action.principal, draft, variant);
      case 'drop':
        return dropRoleSql(dialect, action.principal, variant);
      case 'grant': {
        const target = kind === 'database' ? (schema?.current_database ?? '') : object;
        return grantSql(dialect, { privileges: family[kind].filter((p) => privileges.includes(p)), kind, schema: ns, object: target }, action.principal);
      }
      case 'revoke':
        return action.grants
          .map((g) => revokeSql(dialect, g))
          .filter((s): s is string => !!s)
          .join('\n');
    }
  });

  function mongoText(): string {
    switch (action.mode) {
      case 'create':
        return draft.name.trim() ? mongoCreate({ ...draft, name: draft.name.trim() }) : '';
      case 'alter':
        return mongoAlter(action.principal, draft);
      case 'drop':
        return mongoDrop(action.principal);
      case 'grant': {
        const target = kind === 'database' ? (schema?.current_database ?? '') : object;
        return mongoGrant({ privileges: family[kind].filter((p) => privileges.includes(p)), kind, schema: ns, object: target }, action.principal);
      }
      case 'revoke': {
        const of = action.principal;
        return action.grants
          .map((g) => mongoRevoke(g, of))
          .filter((s): s is string => !!s)
          .join('\n\n');
      }
    }
  }

  $effect(() => {
    const next = generated;
    if (!untrack(() => edited)) sql = next;
  });

  $effect(() => {
    if (!objects.includes(object)) object = objects[0] ?? '';
  });

  $effect(() => {
    const first = kinds[0];
    if (first && !kinds.includes(kind)) {
      kind = first;
      privileges = [family[first][0] ?? ''];
    }
  });

  $effect(() => dialog.showModal());

  const TITLE = { create: () => (dialect === 'mysql' || dialect === 'mssql' ? m.tool_user_new_title() : dialect === null || dialect === 'clickhouse' ? m.tool_principal_new_title() : m.tool_role_new_title()), alter: m.tool_role_edit, drop: m.tool_role_drop, grant: m.tool_grant, revoke: m.tool_revoke };

  function close() {
    dialog.close();
    onclose();
  }

  function toggle(list: string[], v: string) {
    return list.includes(v) ? list.filter((x) => x !== v) : [...list, v];
  }

  async function execute() {
    if (!sql.trim()) return;
    busy = true;
    failure = null;
    const error = await runScript(connectionId, sql);
    busy = false;
    if (error) failure = error;
    else {
      ondone();
      close();
    }
  }
</script>

<dialog class="sheet-dialog tool-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="usql-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void execute();
    }}
  >
    <header class="dialog-head">
      <h2 id="usql-title" class="display">{TITLE[action.mode]()}</h2>
      {#if who}<p class="hint">{who.host ? `${who.name}@${who.host}` : who.name}</p>{/if}
    </header>
    <div class="dialog-body">
      {#if action.mode === 'create' || action.mode === 'alter'}
        <div class="form-row" class:two={mysql || mongo}>
          <label class="form-field">
            <span class="caps">{account ? m.tool_user_name() : either ? m.tool_principal_name() : m.tool_role_name()}</span>
            <span class="field"><input bind:value={draft.name} autocomplete="off" spellcheck="false" disabled={(mysql || mongo) && action.mode === 'alter'} /></span>
          </label>
          {#if mysql || mongo}
            <label class="form-field">
              <span class="caps">{mongo ? m.mongo_user_db() : m.tool_user_host()}</span>
              <span class="field"><input bind:value={draft.host} autocomplete="off" spellcheck="false" disabled={action.mode === 'alter'} /></span>
            </label>
          {/if}
        </div>
        <div class="tool-checks">
          {#if !mssql}<label class="check"><input type="checkbox" bind:checked={draft.login} disabled={(mysql && action.mode === 'create') || (either && action.mode !== 'create')} /> {mysql ? m.tool_attr_unlocked() : either ? m.tool_principal_login() : m.tool_attr_login()}</label>{/if}
          {#if !account && !either}
            <label class="check"><input type="checkbox" bind:checked={draft.createDb} /> {m.tool_attr_create_db()}</label>
            <label class="check"><input type="checkbox" bind:checked={draft.createRole} /> {m.tool_attr_create_role()}</label>
          {/if}
        </div>
        {#if roles.length}
          <div class="form-field">
            <span class="caps">{m.tool_member_of()}</span>
            <div class="tool-chips">
              {#each roles as r (r)}
                <button type="button" class="tool-chip" aria-pressed={draft.memberOf.includes(r)} onclick={() => (draft.memberOf = toggle(draft.memberOf, r))}>{r}</button>
              {/each}
            </div>
          </div>
        {/if}
        {#if mongo}<p class="hint">{draft.login && action.mode === 'create' ? m.tool_mongo_password_hint() : m.tool_mongo_roles_hint()}</p>{:else if dialect === 'clickhouse'}{#if draft.login && action.mode === 'create'}<p class="hint">{m.tool_clickhouse_user_hint()}</p>{/if}{:else if mysql && action.mode === 'create'}<p class="hint">{m.tool_user_locked_hint()}</p>{:else if mssql}<p class="hint">{m.tool_user_mssql_hint()}</p>{:else if !mysql}<p class="hint">{m.tool_password_hint()}</p>{/if}
      {:else if action.mode === 'grant'}
        <div class="form-field">
          <span class="caps">{m.tool_grant_on()}</span>
          <Segmented
            label={m.tool_grant_on()}
            value={kind}
            options={kinds.map((k) => ({ value: k, label: { table: mongo ? m.tool_kind_collection() : m.tool_kind_table(), schema: mysql ? m.tool_kind_database() : m.tool_kind_schema(), database: mongo ? m.tool_kind_roles() : m.tool_kind_database() }[k] }))}
            onchange={(v) => {
              kind = v;
              privileges = [family[v][0] ?? ''];
            }}
          />
        </div>
        {#if kind !== 'database'}
          <div class="form-row" class:two={kind === 'table'}>
            <label class="form-field">
              <span class="caps">{mysql || either ? m.tool_kind_database() : m.tool_kind_schema()}</span>
              <Select class="tool-select" label={mysql || either ? m.tool_kind_database() : m.tool_kind_schema()} bind:value={ns} options={(schema?.namespaces ?? []).map((n) => ({ value: n.name, label: n.name }))} />
            </label>
            {#if kind === 'table'}
              <label class="form-field">
                <span class="caps">{mongo ? m.tool_kind_collection() : m.tool_kind_table()}</span>
                <Select class="tool-select" label={mongo ? m.tool_kind_collection() : m.tool_kind_table()} bind:value={object} options={objects.map((o) => ({ value: o, label: o }))} />
              </label>
            {/if}
          </div>
        {/if}
        <div class="form-field">
          <span class="caps">{m.tool_privileges()}</span>
          <div class="tool-chips">
            {#each family[kind] as p (p)}
              <button type="button" class="tool-chip mono" aria-pressed={privileges.includes(p)} onclick={() => (privileges = toggle(privileges, p))}>{p}</button>
            {/each}
          </div>
        </div>
      {:else if action.mode === 'drop'}
        <p class="tool-warn">{account ? m.tool_drop_user_note() : m.tool_drop_role_note()}</p>
      {/if}

      <label class="form-field">
        <span class="caps">{mongo ? m.tool_mql_review() : m.tool_sql_review()}</span>
        <textarea class="tool-sql" rows="5" spellcheck="false" bind:value={sql} oninput={() => (edited = true)}></textarea>
        <span class="hint">{m.tool_sql_review_hint()}</span>
      </label>
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      <button class="btn ghost" type="button" disabled={!sql.trim()} onclick={() => (newTab(connectionId, sql), close())}>
        <TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.tool_open_editor()}
      </button>
      <span class="spacer"></span>
      <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
      <button class="btn primary" class:danger={action.mode === 'drop' || action.mode === 'revoke'} type="submit" disabled={busy || !sql.trim()}>
        <Play size={13} strokeWidth={2} aria-hidden="true" />
        {m.tool_sql_run()}
      </button>
    </footer>
  </form>
</dialog>
