<script lang="ts">
  import { ArrowDown, ArrowUp, ChevronDown, Plus, ShieldAlert, X } from '@lucide/svelte';
  import { tick, untrack } from 'svelte';
  import { ApiError, api, inTauri, type ConnectionProfile, type Secrets } from '../lib/api';
  import type { Consistency, TlsMode, TunnelError } from '../lib/bindings';
  import { connections, saveConnection } from '../lib/connections.svelte';
  import { DRIVERS, PRESETS, defaultPort, defaultTls, dialectOf, hasTls, isFlavor, isNetwork, isNetworkDriver, needsTls, splitChoice, takesClientCert, type ChoiceId, type DriverId } from '../lib/drivers';
  import { parseConnectionUrl } from '../lib/connurl';
  import { num } from '../lib/format';
  import { prefs } from '../lib/prefs.svelte';
  import { blankHop, fill, formOf, jumpSecrets, MAX_JUMPS, portOk, sessionOf, targetOf, type HopForm } from '../lib/profile-form';
  import { followersOf, isModel, overridesOf } from '../lib/profile-template';
  import { IDLE_CHOICES, INIT_EXAMPLE, KEEPALIVE_CHOICES, sessionSummary, wholeNumber } from '../lib/session';
  import { currentScope, keepInSpace, org, removeConnection } from '../lib/org.svelte';
  import { moveChoices } from '../lib/tree';
  import { VarPreviewer } from '../lib/vars-preview.svelte';
  import { hasVars, VAR_EXAMPLE } from '../lib/vars';
  import { m } from '../paraglide/messages';
  import { ICON } from '../lib/icons';
  import DriverMark from './DriverMark.svelte';
  import EngineRail from './EngineRail.svelte';
  import HopFields from './HopFields.svelte';
  import OwnMark from './OwnMark.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';
  import SqlField from './SqlField.svelte';
  import VarHint from './VarHint.svelte';

  let {
    profile,
    copy = false,
    askDelete = false,
    follow = false,
    folder: inFolder = null,
    onclose,
  }: { profile: ConnectionProfile | null; copy?: boolean; askDelete?: boolean; follow?: boolean; folder?: string | null; onclose: () => void } = $props();

  const initial = untrack(() => profile);
  const fromModel = untrack(() => follow) && initial !== null && isModel(initial);
  const editing = initial !== null && !untrack(() => copy) && !fromModel;
  const prefilled = initial !== null;
  const newId = crypto.randomUUID();
  const t0 = initial?.target;
  const net0 = t0 && isNetwork(t0) ? t0 : null;
  const proxy0 = net0?.proxy ?? (t0?.driver === 'libsql' && t0.location === 'remote' ? t0.proxy : null);
  const savedJumps = editing ? (net0?.ssh?.jumps.length ?? 0) : 0;

  let f = $state(formOf(initial));
  if (initial && !editing) f.name = fromModel ? m.conn_from_model_name({ name: initial.name }) : m.conn_copy_name({ name: initial.name });
  if (!initial) f.folder = untrack(() => inFolder) ?? '';
  if (fromModel && initial) {
    f.role = 'follows';
    f.modelId = initial.id;
  }

  let dialog: HTMLDialogElement;
  let portTouched = prefilled;
  let password = $state('');
  let clearPassword = $state(false);
  let tlsTouched = prefilled;
  let proxySecret = $state('');
  let tlsKey = $state('');
  let sessionOpen = $state(false);
  const keepaliveChoices = [...new Set([...KEEPALIVE_CHOICES, f.keepalive])].sort((a, b) => a - b);
  const idleChoices = [...new Set([...IDLE_CHOICES, f.idle])].sort((a, b) => a - b);
  const oracleString = $derived(f.driver === 'oracle' && f.oraAddress === 'string');
  const shape = $derived(f.driver === 'odbc' ? 'odbc' : f.driver === 'dynamodb' ? 'aws' : isNetworkDriver(f.driver) ? 'network' : f.driver === 'libsql' && f.location === 'remote' ? 'remote' : 'file');
  const CONSISTENCY: Consistency[] = ['any', 'one', 'two', 'three', 'quorum', 'all', 'local_quorum', 'each_quorum', 'local_one'];
  const consistencyOptions = CONSISTENCY.map((c) => ({ value: c, label: c.toUpperCase() }));
  const optionalDatabase = $derived(f.driver === 'cassandra' || f.driver === 'elasticsearch' || f.driver === 'hana' || f.driver === 'mongodb' || f.driver === 'clickhouse');
  const optionalUser = $derived(f.driver === 'redis' || f.driver === 'cassandra' || f.driver === 'elasticsearch' || f.driver === 'mongodb' || f.driver === 'clickhouse');
  const databaseLabel = $derived(f.driver === 'redis' ? m.redis_form_database() : f.driver === 'cassandra' ? m.cql_keyspace() : f.driver === 'elasticsearch' ? m.es_patterns() : f.driver === 'oracle' ? (f.oraSid ? m.ora_sid() : m.ora_service()) : f.driver === 'hana' ? m.hana_tenant() : m.form_database());

  let busy = $state<'test' | 'save' | null>(null);
  let result = $state<{ ok: boolean; text: string } | null>(null);
  let hostKey = $state<Extract<TunnelError, { kind: 'host_key_unknown' }> | null>(null);
  let confirmDelete = $state(untrack(() => askDelete) && initial !== null);
  let showErrors = $state(false);

  const preview = new VarPreviewer(() => ({ id: editing && initial ? initial.id : null, folder: f.folder || null }));
  const models = $derived(connections.list.filter((c) => isModel(c) && !(editing && c.id === initial?.id)));
  const model = $derived(f.role === 'follows' ? models.find((c) => c.id === f.modelId) : undefined);
  const followers = $derived(editing && initial ? followersOf(initial.id, connections.list) : []);

  const choice = $derived<ChoiceId>(f.flavor ?? f.driver);
  const preset = $derived(isFlavor(choice) ? PRESETS[choice] : null);
  const tlsRequired = $derived(needsTls(choice));
  const tlsDefault = $derived(defaultTls(choice, f.host));
  $effect(() => {
    if (!tlsTouched) f.tls = tlsDefault;
    else if (tlsRequired && f.tls === 'disable') f.tls = 'verify';
  });
  $effect(() => {
    const d = f.driver;
    if (!portTouched && d === 'clickhouse') f.port = String(defaultPort(d, f.tls));
  });

  const missing = $derived.by(() => {
    const out = new Set<string>();
    if (!f.name.trim()) out.add('name');
    if (shape === 'remote') {
      if (!/^(libsql|https?|wss?):\/\/\S+$/.test(f.url.trim()) && !f.url.includes('${')) out.add('url');
      if (f.useProxy && (!f.proxyHost.trim() || !portOk(f.proxyPort))) out.add('proxy');
    } else if (shape === 'odbc') {
      if (!f.odbcDsn.trim() && !f.odbcDriver.trim()) out.add('odbc');
    } else if (shape === 'aws') {
      if (!f.region.trim()) out.add('region');
      if (f.endpoint.trim() && !/^https?:\/\/\S+$/.test(f.endpoint.trim()) && !hasVars(f.endpoint)) out.add('endpoint');
    } else if (shape === 'file') {
      if (f.driver !== 'duckdb' && !f.path.trim()) out.add('path');
    } else if (oracleString) {
      if (!f.connectString.trim()) out.add('connectString');
      if (!f.user.trim()) out.add('user');
    } else {
      if (!f.host.trim()) out.add('host');
      if (!portOk(f.port)) out.add('port');
      if (f.driver === 'redis' ? !/^\d{1,5}$/.test(f.database.trim()) && !hasVars(f.database) : !f.database.trim() && !optionalDatabase) out.add('database');
      if (!f.user.trim() && !optionalUser) out.add('user');
      if (f.useSsh && (!f.ssh.host.trim() || !f.ssh.user.trim() || !portOk(f.ssh.port))) out.add('ssh');
      if (f.useSsh && f.jumps.some((j) => !j.host.trim() || !j.user.trim() || !portOk(j.port))) out.add('jumps');
      if (f.useProxy && (!f.proxyHost.trim() || !portOk(f.proxyPort))) out.add('proxy');
      if (f.useCert && f.tls !== 'disable' && (!f.certFile.trim() || !f.keyFile.trim())) out.add('cert');
      if (f.driver === 'redis' && f.redisMode === 'sentinel' && !f.sentinelMaster.trim()) out.add('master');
    }
    if (wholeNumber(f.rowLimit, 1, 2_000_000) === undefined) out.add('rowLimit');
    if (f.role === 'follows' && !model) out.add('model');
    return out;
  });

  const networkShape = $derived(shape !== 'file');
  const session = $derived(sessionOf(f, networkShape));

  function build(): ConnectionProfile {
    const id = editing && initial ? initial.id : newId;
    const { target, raw } = targetOf(f);
    const base: ConnectionProfile = {
      id,
      name: f.name.trim(),
      folder: f.folder || null,
      environment: f.environment,
      color: initial?.color ?? null,
      target,
      session,
      template: f.role === 'model' ? { kind: 'model' } : { kind: 'none' },
      raw_fields: raw,
    };
    return model ? { ...base, template: { kind: 'follows', id: model.id, overrides: overridesOf(base, model) } } : base;
  }

  const own = $derived(model ? new Set(overridesOf(build(), model)) : new Set<string>());
  const mine = (key: string) => own.has(key);

  function reset(key: string) {
    if (model) fill(f, formOf(model), [key]);
  }

  function chooseRole(value: string) {
    if (value === 'none' || value === 'model') {
      f.role = value;
      f.modelId = '';
      return;
    }
    const picked = models.find((c) => c.id === value);
    if (!picked) return;
    f.role = 'follows';
    f.modelId = picked.id;
    fill(f, formOf(picked));
    tlsTouched = true;
    portTouched = true;
  }

  function secrets(): Secrets {
    const keeps = shape === 'network' || shape === 'remote' || shape === 'aws' || shape === 'odbc';
    const network = shape === 'network';
    const hadProxy = editing && !!proxy0;
    const proxied = network || shape === 'remote';
    const hadCert = editing && !!net0 && 'client_cert' in net0 && !!net0.client_cert;
    const useCert = network && f.useCert && f.tls !== 'disable' && takesClientCert(f.driver);
    return {
      password: keeps ? (clearPassword ? '' : password || null) : null,
      ssh: network && f.useSsh && f.ssh.method !== 'agent' ? f.ssh.secret || null : null,
      proxy: proxied && f.useProxy && f.proxyUser.trim() ? proxySecret || null : hadProxy ? '' : null,
      tls_key: useCert ? tlsKey || null : hadCert ? '' : null,
      jumps: network && f.useSsh ? jumpSecrets(f.jumps, savedJumps) : Array.from({ length: savedJumps }, () => ''),
    };
  }

  function explain(e: unknown) {
    if (e instanceof ApiError && e.error.code === 'tunnel') {
      const d = e.error.detail;
      if (d.kind === 'host_key_unknown') {
        hostKey = d;
        return null;
      }
      if (d.kind === 'host_key_changed') return m.hostkey_changed({ host: d.host });
    }
    return e instanceof Error ? e.message : String(e);
  }

  function reveal() {
    if (missing.has('rowLimit')) sessionOpen = true;
  }

  async function test() {
    showErrors = true;
    reveal();
    if (missing.size) return;
    busy = 'test';
    result = null;
    hostKey = null;
    try {
      const s = await api.connectionTest(build(), secrets());
      result = { ok: true, text: m.form_test_ok({ product: s.product, version: s.version, database: s.database }) };
    } catch (e) {
      const text = explain(e);
      if (text) result = { ok: false, text };
    } finally {
      busy = null;
    }
  }

  async function trust() {
    if (!hostKey) return;
    await api.hostKeyTrust(hostKey.host, hostKey.port, hostKey.key);
    hostKey = null;
    await test();
  }

  async function save() {
    showErrors = true;
    reveal();
    if (missing.size) return;
    busy = 'save';
    try {
      const saved = build();
      if (!(await saveConnection(saved, secrets()))) return;
      if (!editing) await keepInSpace(saved);
      close();
    } catch (e) {
      result = { ok: false, text: explain(e) ?? '' };
    } finally {
      busy = null;
    }
  }

  async function remove() {
    if (!initial) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    await removeConnection(initial);
    close();
  }

  const FILE_TYPES: Partial<Record<DriverId, { name: string; extensions: string[] }>> = {
    sqlite: { name: 'SQLite', extensions: ['sqlite', 'sqlite3', 'db', 'db3'] },
    duckdb: { name: 'DuckDB', extensions: ['duckdb', 'ddb', 'db'] },
    libsql: { name: 'libSQL', extensions: ['db', 'sqlite', 'sqlite3'] },
  };

  async function chooseFile() {
    if (!inTauri) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const ownType = FILE_TYPES[f.driver];
    const picked = await open({ multiple: false, filters: [...(ownType ? [ownType] : []), { name: '*', extensions: ['*'] }] });
    if (typeof picked === 'string') f.path = picked;
  }

  function pick(id: ChoiceId) {
    const { driver, flavor } = splitChoice(id);
    f.driver = driver;
    f.flavor = flavor;
    if (!portTouched && isNetworkDriver(driver)) f.port = String(flavor ? PRESETS[flavor].port : defaultPort(driver, f.tls));
    if (driver === 'redis' && !/^\d+$/.test(f.database.trim()) && !hasVars(f.database)) f.database = '0';
  }

  let pasted = $state<'ok' | 'bad' | null>(null);
  function fromUrl(text: string) {
    if (!text.trim()) return void (pasted = null);
    const u = parseConnectionUrl(text, choice);
    if (!u || (editing && splitChoice(u.choice).driver !== f.driver)) return void (pasted = 'bad');
    if (!editing) pick(u.choice);
    f.host = u.host;
    f.port = String(u.port);
    portTouched = true;
    if (u.database) f.database = u.database;
    if (u.user) f.user = u.user;
    if (u.password) password = u.password;
    f.tls = u.tls;
    tlsTouched = true;
    pasted = 'ok';
  }

  function moveJump(i: number, step: -1 | 1) {
    const next = [...f.jumps];
    const [hop] = next.splice(i, 1);
    if (!hop) return;
    next.splice(i + step, 0, hop);
    f.jumps = next;
  }

  function close() {
    dialog.close();
    onclose();
  }

  let deleteButton = $state<HTMLButtonElement>();
  let hostKeyBox = $state<HTMLElement>();
  $effect(() => hostKeyBox?.scrollIntoView({ block: 'nearest' }));

  $effect(() => {
    dialog.showModal();
    if (untrack(() => askDelete)) deleteButton?.focus();
    else if (editing) nameInput?.focus();
    else search?.focus();
  });

  const envOptions = (['local', 'development', 'staging', 'production'] as const).map((value) => ({
    value,
    label: { local: m.env_local, development: m.env_development, staging: m.env_staging, production: m.env_production }[value](),
  }));
  const tlsOptions = $derived(
    (['verify', 'disable', 'insecure'] as const)
      .filter((value) => !(tlsRequired && value === 'disable'))
      .map((value) => ({ value, label: { verify: m.tls_verify, disable: m.tls_disable, insecure: m.tls_insecure }[value]() })),
  );
  const FLAVOR_HINT = { timescale: m.flavor_hint_timescale, yugabyte: m.flavor_hint_yugabyte, neon: m.flavor_hint_neon, supabase: m.flavor_hint_supabase, tidb: m.flavor_hint_tidb, planetscale: m.flavor_hint_planetscale } as const;
  const redisModes = (['single', 'cluster', 'sentinel'] as const).map((value) => ({
    value,
    label: { single: m.redis_mode_single, cluster: m.redis_mode_cluster, sentinel: m.redis_mode_sentinel }[value](),
  }));
  const proxyOptions = (['socks5', 'http'] as const).map((value) => ({ value, label: { socks5: m.proxy_socks5, http: m.proxy_http }[value]() }));
  const locationOptions = (['file', 'remote'] as const).map((value) => ({
    value,
    label: { file: m.location_file, remote: m.location_remote }[value](),
  }));
  const current = $derived(DRIVERS.find((d) => d.id === choice) ?? DRIVERS[0]!);
  const baseName = $derived(current.base ? DRIVERS.find((d) => d.id === current.base)?.name : undefined);
  const KIND = { relational: m.kind_relational, document: m.kind_document, keyvalue: m.kind_keyvalue, analytical: m.kind_analytical, widecolumn: m.kind_widecolumn, search: m.kind_search } as const;
  const usable = (id: ChoiceId) => !((editing || !!model) && splitChoice(id).driver !== f.driver);
  let railOpen = $state(!editing);
  let search = $state<HTMLInputElement>();
  let nameInput = $state<HTMLInputElement>();
  async function toggleRail() {
    railOpen = !railOpen;
    await tick();
    if (railOpen) search?.focus();
  }
  const takesUrl = $derived(f.driver === 'postgres' || f.driver === 'mysql' || f.driver === 'cockroach' || f.driver === 'oracle');
  const oraAddresses = (['host', 'string'] as const).map((value) => ({ value, label: { host: m.ora_address_host, string: m.ora_address_string }[value]() }));
  const oraRoles = (['normal', 'sysdba', 'sysoper'] as const).map((value) => ({ value, label: { normal: m.ora_role_normal(), sysdba: 'SYSDBA', sysoper: 'SYSOPER' }[value] }));
  const bad = (k: string) => showErrors && missing.has(k);
  const scopedChoices = moveChoices(org.folders, { kind: 'connection', id: '' }, currentScope());
  const ownFolder = scopedChoices.some((c) => c.folder.id === initial?.folder) ? undefined : org.folders.find((x) => x.id === initial?.folder);
  const folderChoices = ownFolder ? [...scopedChoices, { folder: ownFolder, depth: 0, allowed: true }] : scopedChoices;
  const roleValue = $derived(f.role === 'follows' ? f.modelId : f.role);
</script>

{#snippet proxyBox()}
<div class="check-line">
  <label class="check"><input type="checkbox" bind:checked={f.useProxy} /> <span>{m.form_proxy()}</span></label>
  <OwnMark own={mine('proxy')} onreset={() => reset('proxy')} />
</div>
{#if f.useProxy}
  <div class="ssh net-box">
    <div class="form-field">
      <span class="caps">{m.form_proxy_kind()}</span>
      <Segmented label={m.form_proxy_kind()} value={f.proxyKind} options={proxyOptions} onchange={(v) => (f.proxyKind = v)} />
    </div>
    <div class="form-row host">
      <label class="form-field" class:invalid={bad('proxy') && !f.proxyHost.trim()}>
        <span class="caps">{m.form_proxy_host()}</span>
        <span class="field"><input bind:value={f.proxyHost} placeholder={m.form_ph_proxy_host()} spellcheck="false" autocomplete="off" /></span>
        <VarHint text={f.proxyHost} {preview} />
      </label>
      <label class="form-field" class:invalid={bad('proxy') && !portOk(f.proxyPort)}>
        <span class="caps">{m.form_port()}</span>
        <span class="field"><input bind:value={f.proxyPort} inputmode="numeric" autocomplete="off" /></span>
        <VarHint text={f.proxyPort} {preview} />
      </label>
    </div>
    <div class="form-row two">
      <label class="form-field">
        <span class="caps">{m.form_proxy_user()}</span>
        <span class="field"><input bind:value={f.proxyUser} placeholder={m.form_proxy_user_hint()} spellcheck="false" autocomplete="off" /></span>
        <VarHint text={f.proxyUser} {preview} />
      </label>
      <label class="form-field">
        <span class="caps">{m.form_proxy_password()}</span>
        <span class="field"
          ><input type="password" bind:value={proxySecret} placeholder={editing && proxy0?.user ? m.form_password_saved() : ''} autocomplete="new-password" disabled={!f.proxyUser.trim()} /></span
        >
      </label>
    </div>
    <p class="hint">{m.proxy_hint()}</p>
  </div>
{/if}
{/snippet}

<dialog class="sheet-dialog conn-dialog" class:rail-shut={!railOpen} bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="conn-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void save();
    }}
  >
    {#if railOpen}
      <aside class="conn-rail" id="conn-rail">
        <EngineRail value={choice} {usable} onpick={pick} onenter={() => nameInput?.focus()} bind:search />
      </aside>
    {/if}

    <div class="conn-pane">
      <header class="dialog-head conn-head">
        <button class="icon-btn conn-rail-toggle" type="button" aria-controls={railOpen ? 'conn-rail' : undefined} aria-expanded={railOpen} aria-label={railOpen ? m.eng_hide() : m.eng_show()} data-tip={railOpen ? m.eng_hide() : m.eng_show()} onclick={toggleRail}>
          <ICON.engines size={15} strokeWidth={1.8} />
        </button>
        <span class="conn-plate" aria-hidden="true"><DriverMark driver={current} size={24} /></span>
        <div class="conn-ident">
          <h2 id="conn-title" class="caps conn-title">{editing ? m.form_title_edit() : m.form_title_new()}</h2>
          <p class="display conn-engine">{current.name}</p>
          <p class="conn-engine-meta">{KIND[current.kind]()} · {current.where === 'file' ? m.where_file() : current.where === 'cloud' ? m.where_cloud() : m.where_server()}{baseName ? ` · ${m.eng_on({ base: baseName })}` : ''}</p>
        </div>
      </header>

      <div class="dialog-body">
        <div class="form-row two">
          <label class="form-field" class:invalid={bad('name')}>
            <span class="caps">{m.form_name()}</span>
            <span class="field"><input bind:this={nameInput} bind:value={f.name} placeholder={m.form_name_hint()} /></span>
          </label>
          <label class="form-field">
            <span class="caps">{m.org_folder()}</span>
            <Select class="field" label={m.org_folder()} bind:value={f.folder} options={[{ value: '', label: m.org_folder_none() }, ...folderChoices.map((c) => ({ value: c.folder.id, label: c.folder.name, indent: c.depth }))]} />
          </label>
        </div>

        <div class="form-row two conn-role">
          <div class="form-field model-field" class:invalid={bad('model')}>
            <label class="caps" for="conn-model">{m.form_model()}</label>
            <div class="model-row">
              <Select
                id="conn-model"
                class="field"
                value={roleValue}
                onchange={(v) => chooseRole(v)}
                options={[
                  { value: 'none', label: m.model_role_none() },
                  { value: 'model', label: m.model_role_model() },
                  ...models.map((c) => ({ value: c.id, label: c.name, group: m.model_follow_group() })),
                ]}
              />
              {#if model && own.size}<span class="model-own-count">{m.model_own_count({ count: own.size })}</span>{/if}
            </div>
            {#if f.role === 'model'}
              <p class="hint">{m.model_is_hint()} {followers.length ? m.model_followers({ names: followers.map((c) => c.name).join(', ') }) : m.model_no_followers()}</p>
            {:else if model}
              <p class="hint">{m.model_follows_hint()}</p>
            {/if}
          </div>

          <div class="form-field">
            <span class="caps">{m.form_environment()}</span>
            <Segmented label={m.form_environment()} value={f.environment} options={envOptions} onchange={(v) => (f.environment = v)} />
            {#if f.environment === 'production'}<p class="hint prod-note"><ShieldAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.form_prod_note()}</p>{/if}
          </div>
        </div>

        {#if isFlavor(choice)}<p class="hint flavor-note">{FLAVOR_HINT[choice]()}</p>{/if}

        {#if shape === 'network' && takesUrl}
          <label class="form-field" class:invalid={pasted === 'bad'}>
            <span class="caps">{m.form_url_paste()}</span>
            <span class="field"
              ><input
                placeholder={f.driver === 'mysql' ? m.form_url_hint_mysql() : f.driver === 'oracle' ? m.form_url_hint_oracle() : m.form_url_hint_postgres()}
                autocomplete="off"
                spellcheck="false"
                onchange={(e) => fromUrl(e.currentTarget.value)}
                onpaste={(e) => {
                  const text = e.clipboardData?.getData('text') ?? '';
                  if (!text) return;
                  e.preventDefault();
                  e.currentTarget.value = '';
                  fromUrl(text);
                }}
              /></span
            >
            <span class="hint">{pasted === 'ok' ? m.form_url_filled() : pasted === 'bad' ? m.form_url_bad() : m.form_url_paste_hint()}</span>
          </label>
        {/if}

        {#if f.driver === 'libsql'}
          <div class="form-field">
            <span class="caps-line"><span class="caps">{m.form_location()}</span><OwnMark own={mine('location')} onreset={() => reset('location')} /></span>
            <Segmented label={m.form_location()} value={f.location} options={locationOptions} onchange={(v) => (f.location = v)} />
          </div>
        {/if}

        {#if shape === 'file'}
          <div class="form-field" class:invalid={bad('path')}>
            <span class="caps-line"><span class="caps">{m.form_file()}</span><OwnMark own={mine('path')} onreset={() => reset('path')} /></span>
            <div class="with-button">
              <span class="field"
                ><input
                  bind:value={f.path}
                  placeholder={f.driver === 'duckdb' ? m.form_file_memory() : f.driver === 'libsql' ? m.form_file_hint({ ext: 'db' }) : m.form_file_hint({ ext: 'sqlite' })}
                  aria-label={m.form_file()}
                  spellcheck="false"
                /></span
              >
              {#if inTauri}<button class="btn" type="button" onclick={() => void chooseFile()}>{m.form_choose()}</button>{/if}
            </div>
            <VarHint text={f.path} {preview} />
            {#if f.driver === 'libsql'}<p class="hint">{m.libsql_file_note()}</p>{/if}
          </div>
          <label class="check"><input type="checkbox" bind:checked={f.readOnly} /> <span>{m.form_read_only()}</span></label>
        {:else if shape === 'remote'}
          <label class="form-field" class:invalid={bad('url')}>
            <span class="caps-line"><span class="caps">{m.form_url()}</span><OwnMark own={mine('url')} onreset={() => reset('url')} /></span>
            <span class="field"><input bind:value={f.url} placeholder={m.form_ph_libsql_url()} autocomplete="off" spellcheck="false" /></span>
            <VarHint text={f.url} {preview} />
          </label>
          <label class="form-field">
            <span class="caps">{m.form_token()}</span>
            <span class="field"><input type="password" bind:value={password} placeholder={editing ? m.form_password_saved() : ''} autocomplete="new-password" disabled={clearPassword} /></span>
          </label>
          {#if editing}<label class="check"><input type="checkbox" bind:checked={clearPassword} /> <span>{m.form_token_clear()}</span></label>{/if}
          {@render proxyBox()}
        {:else if shape === 'odbc'}
          <div class="form-row two">
            <label class="form-field" class:invalid={bad('odbc')}>
              <span class="caps-line"><span class="caps">{m.odbc_dsn()}</span><OwnMark own={mine('dsn')} onreset={() => reset('dsn')} /></span>
              <span class="field"><input bind:value={f.odbcDsn} placeholder={m.form_ph_odbc_dsn()} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.odbcDsn} {preview} />
            </label>
            <label class="form-field" class:invalid={bad('odbc')}>
              <span class="caps-line"><span class="caps">{m.odbc_driver()}</span><OwnMark own={mine('driver_path')} onreset={() => reset('driver_path')} /></span>
              <span class="field"><input bind:value={f.odbcDriver} placeholder={m.form_ph_odbc_driver()} disabled={!!f.odbcDsn.trim()} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.odbcDriver} {preview} />
            </label>
          </div>
          <p class="hint">{m.odbc_driver_hint()}</p>
          <label class="form-field">
            <span class="caps-line"><span class="caps">{m.odbc_connect_string()}</span><OwnMark own={mine('connect_string')} onreset={() => reset('connect_string')} /></span>
            <span class="field"><input bind:value={f.connectString} placeholder={m.form_ph_odbc_connect()} autocomplete="off" spellcheck="false" /></span>
            <VarHint text={f.connectString} {preview} />
          </label>
          <div class="form-row two">
            <label class="form-field">
              <span class="caps-line"><span class="caps">{m.form_user()}</span><OwnMark own={mine('user')} onreset={() => reset('user')} /></span>
              <span class="field"><input bind:value={f.user} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.user} {preview} />
            </label>
            <label class="form-field">
              <span class="caps">{m.form_password()}</span>
              <span class="field"><input type="password" bind:value={password} placeholder={editing ? m.form_password_saved() : ''} disabled={clearPassword} autocomplete="new-password" /></span>
            </label>
          </div>
          <p class="hint">{m.odbc_hint()}</p>
          {#if editing}<label class="check"><input type="checkbox" bind:checked={clearPassword} /> <span>{m.form_password_clear()}</span></label>{/if}
        {:else if shape === 'aws'}
          <div class="form-row two">
            <label class="form-field" class:invalid={bad('region')}>
              <span class="caps-line"><span class="caps">{m.ddb_region()}</span><OwnMark own={mine('region')} onreset={() => reset('region')} /></span>
              <span class="field"><input bind:value={f.region} placeholder={m.form_ph_region()} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.region} {preview} />
            </label>
            <label class="form-field" class:invalid={bad('endpoint')}>
              <span class="caps-line"><span class="caps">{m.ddb_endpoint()}</span><OwnMark own={mine('endpoint')} onreset={() => reset('endpoint')} /></span>
              <span class="field"><input bind:value={f.endpoint} placeholder={m.form_ph_endpoint()} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.endpoint} {preview} />
            </label>
          </div>
          <p class="hint">{m.ddb_endpoint_hint()}</p>
          <div class="form-row three">
            <label class="form-field">
              <span class="caps-line"><span class="caps">{m.ddb_profile()}</span><OwnMark own={mine('profile')} onreset={() => reset('profile')} /></span>
              <span class="field"><input bind:value={f.awsProfile} placeholder={m.form_ph_aws_profile()} autocomplete="off" spellcheck="false" /></span>
            </label>
            <label class="form-field">
              <span class="caps-line"><span class="caps">{m.ddb_access_key()}</span><OwnMark own={mine('user')} onreset={() => reset('user')} /></span>
              <span class="field"><input bind:value={f.user} placeholder={m.form_ph_access_key()} disabled={!!f.awsProfile.trim()} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.user} {preview} />
            </label>
            <label class="form-field">
              <span class="caps">{m.ddb_secret_key()}</span>
              <span class="field"><input type="password" bind:value={password} placeholder={editing ? m.form_password_saved() : ''} disabled={clearPassword || !!f.awsProfile.trim()} autocomplete="new-password" /></span>
            </label>
          </div>
          <p class="hint">{m.ddb_credentials_hint()}</p>
          {#if editing}<label class="check"><input type="checkbox" bind:checked={clearPassword} /> <span>{m.form_password_clear()}</span></label>{/if}
        {:else}
          {#if f.driver === 'redis'}
            <div class="form-field">
              <span class="caps-line"><span class="caps">{m.redis_mode()}</span><OwnMark own={mine('mode')} onreset={() => reset('mode')} /></span>
              <Segmented label={m.redis_mode()} value={f.redisMode} options={redisModes} onchange={(v) => (f.redisMode = v)} />
              {#if f.redisMode !== 'single'}<p class="hint">{f.redisMode === 'cluster' ? m.redis_mode_cluster_hint() : m.redis_mode_sentinel_hint()}</p>{/if}
            </div>
          {/if}
          {#if f.driver === 'oracle'}
            <div class="form-field">
              <span class="caps-line"><span class="caps">{m.ora_address()}</span><OwnMark own={mine('connect_string')} onreset={() => reset('connect_string')} /></span>
              <Segmented label={m.ora_address()} value={f.oraAddress} options={oraAddresses} onchange={(v) => (f.oraAddress = v)} />
            </div>
            {#if oracleString}
              <label class="form-field" class:invalid={bad('connectString')}>
                <span class="caps">{m.ora_connect_string()}</span>
                <span class="field"><input bind:value={f.connectString} placeholder={m.form_ph_oracle_connect()} autocomplete="off" spellcheck="false" /></span>
                <VarHint text={f.connectString} {preview} />
                <p class="hint">{m.ora_connect_string_hint()}</p>
              </label>
            {/if}
          {/if}
          {#if !oracleString}
          <div class="form-row host">
            <label class="form-field" class:invalid={bad('host')}>
              <span class="caps-line"><span class="caps">{f.driver === 'redis' && f.redisMode === 'sentinel' ? m.redis_sentinels() : f.driver === 'cassandra' ? m.cql_hosts() : m.form_host()}</span><OwnMark own={mine('host')} onreset={() => reset('host')} /></span>
              <span class="field"><input bind:value={f.host} placeholder={preset?.host ?? ''} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.host} {preview} />
            </label>
            {#if !(f.driver === 'mongodb' && f.srv)}
            <label class="form-field" class:invalid={bad('port')}>
              <span class="caps-line"><span class="caps">{m.form_port()}</span><OwnMark own={mine('port')} onreset={() => reset('port')} /></span>
              <span class="field"><input bind:value={f.port} inputmode="numeric" oninput={() => (portTouched = true)} autocomplete="off" /></span>
              <VarHint text={f.port} {preview} />
            </label>
            {/if}
          </div>
          {/if}
          {#if f.driver === 'mongodb'}
            <div class="check-line">
              <label class="check"><input type="checkbox" bind:checked={f.srv} /> <span>{m.mongo_srv()}</span></label>
              <OwnMark own={mine('srv')} onreset={() => reset('srv')} />
            </div>
            {#if f.srv}<p class="hint">{m.mongo_srv_hint()}</p>{/if}
            <div class="form-row two">
              <label class="form-field">
                <span class="caps-line"><span class="caps">{m.mongo_auth_source()}</span><OwnMark own={mine('auth_source')} onreset={() => reset('auth_source')} /></span>
                <span class="field"><input bind:value={f.authSource} placeholder={m.form_ph_auth_source()} autocomplete="off" spellcheck="false" /></span>
              </label>
              <label class="form-field">
                <span class="caps-line"><span class="caps">{m.mongo_options()}</span><OwnMark own={mine('options')} onreset={() => reset('options')} /></span>
                <span class="field"><input bind:value={f.options} placeholder={m.form_ph_mongo_options()} autocomplete="off" spellcheck="false" /></span>
                <VarHint text={f.options} {preview} />
              </label>
            </div>
          {/if}
          {#if f.driver === 'redis' && f.redisMode === 'sentinel'}
            <label class="form-field" class:invalid={bad('master')}>
              <span class="caps">{m.redis_sentinel_master()}</span>
              <span class="field"><input bind:value={f.sentinelMaster} placeholder={m.form_ph_sentinel_master()} autocomplete="off" spellcheck="false" /></span>
            </label>
          {/if}
          <div class="form-row" class:three={!oracleString} class:two={oracleString}>
            {#if !oracleString}
            <label class="form-field" class:invalid={bad('database')}>
              <span class="caps-line"><span class="caps">{databaseLabel}</span><OwnMark own={mine('database')} onreset={() => reset('database')} /></span>
              <span class="field"><input bind:value={f.database} inputmode={f.driver === 'redis' ? 'numeric' : undefined} placeholder={f.driver === 'redis' ? m.form_ph_redis_db() : f.driver === 'elasticsearch' ? m.form_ph_es_indexes() : f.driver === 'oracle' ? m.form_ph_oracle_service() : (preset?.database ?? '')} disabled={f.driver === 'redis' && f.redisMode === 'cluster'} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.database} {preview} />
            </label>
            {/if}
            <label class="form-field" class:invalid={bad('user')}>
              <span class="caps-line"><span class="caps">{m.form_user()}</span><OwnMark own={mine('user')} onreset={() => reset('user')} /></span>
              <span class="field"><input bind:value={f.user} placeholder={f.driver === 'redis' ? m.form_ph_redis_user() : (preset?.user ?? '')} autocomplete="off" spellcheck="false" /></span>
              <VarHint text={f.user} {preview} />
            </label>
            <label class="form-field">
              <span class="caps">{m.form_password()}</span>
              <span class="field"><input type="password" bind:value={password} placeholder={editing ? m.form_password_saved() : ''} autocomplete="new-password" disabled={clearPassword} /></span>
            </label>
          </div>
          {#if editing}<label class="check"><input type="checkbox" bind:checked={clearPassword} /> <span>{m.form_password_clear()}</span></label>{/if}
          {#if f.driver === 'oracle'}
            {#if !oracleString}
              <div class="check-line">
                <label class="check"><input type="checkbox" bind:checked={f.oraSid} /> <span>{m.ora_sid_check()}</span></label>
                <OwnMark own={mine('sid')} onreset={() => reset('sid')} />
              </div>
            {/if}
            <div class="form-field">
              <span class="caps-line"><span class="caps">{m.ora_role()}</span><OwnMark own={mine('role')} onreset={() => reset('role')} /></span>
              <Segmented label={m.ora_role()} value={f.oraRole} options={oraRoles} onchange={(v) => (f.oraRole = v)} />
            </div>
          {/if}
          {#if f.driver === 'cassandra'}
            <div class="form-row two">
              <div class="form-field">
                <span class="caps-line"><span class="caps">{m.cql_consistency()}</span><OwnMark own={mine('consistency')} onreset={() => reset('consistency')} /></span>
                <Select class="field" label={m.cql_consistency()} bind:value={f.consistency} options={consistencyOptions} />
              </div>
              <label class="form-field">
                <span class="caps-line"><span class="caps">{m.cql_datacenter()}</span><OwnMark own={mine('datacenter')} onreset={() => reset('datacenter')} /></span>
                <span class="field"><input bind:value={f.datacenter} placeholder={m.form_ph_datacenter()} autocomplete="off" spellcheck="false" /></span>
                <VarHint text={f.datacenter} {preview} />
              </label>
            </div>
            <p class="hint">{m.cql_form_hint()}</p>
          {/if}
          {#if f.driver === 'elasticsearch'}<p class="hint">{m.es_form_hint()}</p>{/if}

          {#if !hasTls(f.driver)}
            <p class="hint">{m.fb_no_tls_hint()}</p>
          {:else}
          <div class="form-field">
            <span class="caps-line"><span class="caps">{m.form_tls()}</span><OwnMark own={mine('tls')} onreset={() => reset('tls')} /></span>
            <Segmented
              label={m.form_tls()}
              value={f.tls}
              options={tlsOptions}
              onchange={(v: TlsMode) => {
                f.tls = v;
                tlsTouched = true;
              }}
            />
            {#if f.tls === 'insecure'}<p class="hint warn-note">{m.tls_insecure_note()}</p>{:else if tlsRequired}<p class="hint">{m.tls_required_note()}</p>{/if}
          </div>
          {#if f.tls === 'verify'}
            <label class="form-field">
              <span class="caps-line"><span class="caps">{m.form_ca()}</span><OwnMark own={mine('ca_file')} onreset={() => reset('ca_file')} /></span>
              <span class="field"><input bind:value={f.caFile} placeholder={m.form_ph_ca_file()} spellcheck="false" /></span>
              <VarHint text={f.caFile} {preview} />
            </label>
          {/if}
          {/if}
          {#if f.driver === 'clickhouse'}<p class="hint">{m.clickhouse_http_hint()}</p>{/if}
          {#if f.driver === 'oracle' && f.tls !== 'disable'}<p class="hint">{m.ora_tls_hint()}</p>{/if}
          {#if f.driver === 'db2'}<p class="hint">{m.db2_form_hint()}</p>{/if}
          {#if f.driver === 'hana'}<p class="hint">{m.hana_form_hint()}</p>{/if}
          {#if f.tls !== 'disable' && takesClientCert(f.driver)}
            <div class="check-line">
              <label class="check"><input type="checkbox" bind:checked={f.useCert} /> <span>{m.form_client_cert()}</span></label>
              <OwnMark own={mine('client_cert')} onreset={() => reset('client_cert')} />
            </div>
            {#if f.useCert}
              <div class="ssh net-box" class:invalid-box={bad('cert')}>
                <div class="form-row two">
                  <label class="form-field" class:invalid={bad('cert') && !f.certFile.trim()}>
                    <span class="caps">{m.form_cert_file()}</span>
                    <span class="field"><input bind:value={f.certFile} placeholder={m.form_cert_hint({ ext: 'crt' })} spellcheck="false" autocomplete="off" /></span>
                    <VarHint text={f.certFile} {preview} />
                  </label>
                  <label class="form-field" class:invalid={bad('cert') && !f.keyFile.trim()}>
                    <span class="caps">{m.form_key_file()}</span>
                    <span class="field"><input bind:value={f.keyFile} placeholder={m.form_cert_hint({ ext: 'key' })} spellcheck="false" autocomplete="off" /></span>
                    <VarHint text={f.keyFile} {preview} />
                  </label>
                </div>
                <label class="form-field">
                  <span class="caps">{m.form_key_passphrase()}</span>
                  <span class="field"><input type="password" bind:value={tlsKey} placeholder={editing && net0 && 'client_cert' in net0 && net0.client_cert ? m.form_password_saved() : ''} autocomplete="new-password" /></span>
                </label>
                <p class="hint">{m.cert_hint()}</p>
              </div>
            {/if}
          {/if}

          {#if !(f.driver === 'mongodb' && f.srv) && !(f.driver === 'redis' && f.redisMode !== 'single') && !oracleString}
          <div class="check-line">
            <label class="check"><input type="checkbox" bind:checked={f.useSsh} /> <span>{m.form_ssh()}</span></label>
            <OwnMark own={mine('ssh')} onreset={() => reset('ssh')} />
          </div>
          {#if f.useSsh}
            <div class="ssh">
              <HopFields bind:hop={f.ssh} {preview} showErrors={bad('ssh')} hostLabel={m.form_ssh_host()} savedSecret={editing && !!net0?.ssh} />
              <div class="jumps">
                <span class="caps">{m.ssh_jumps()}</span>
                {#each f.jumps as jump, i (i)}
                  <div class="jump" role="group" aria-label={m.ssh_jump_n({ n: i + 1 })}>
                    <div class="jump-head">
                      <span class="jump-n">{m.ssh_jump_n({ n: i + 1 })}</span>
                      <button class="icon-btn" type="button" aria-label={m.ssh_jump_up({ n: i + 1 })} disabled={i === 0} onclick={() => moveJump(i, -1)}><ArrowUp size={13} strokeWidth={1.8} /></button>
                      <button class="icon-btn" type="button" aria-label={m.ssh_jump_down({ n: i + 1 })} disabled={i === f.jumps.length - 1} onclick={() => moveJump(i, 1)}><ArrowDown size={13} strokeWidth={1.8} /></button>
                      <button class="icon-btn" type="button" aria-label={m.ssh_jump_remove({ n: i + 1 })} onclick={() => (f.jumps = f.jumps.filter((_, k) => k !== i))}><X size={13} strokeWidth={1.8} /></button>
                    </div>
                    <HopFields bind:hop={f.jumps[i] as HopForm} {preview} showErrors={bad('jumps')} hostLabel={m.form_host()} savedSecret={jump.saved === i} />
                  </div>
                {/each}
                <div class="jump-add">
                  <button class="btn ghost" type="button" disabled={f.jumps.length >= MAX_JUMPS} onclick={() => (f.jumps = [...f.jumps, blankHop()])}><Plus size={14} strokeWidth={1.8} /> {m.ssh_jump_add()}</button>
                  <p class="hint">{f.jumps.length >= MAX_JUMPS ? m.ssh_jumps_max({ count: MAX_JUMPS }) : m.ssh_jumps_hint()}</p>
                </div>
              </div>
            </div>
          {/if}

          {@render proxyBox()}
          {/if}
        {/if}

        <details class="form-section session-section" bind:open={sessionOpen}>
          <summary>
            <span class="caps">{m.session_title()}</span>
            <span class="form-section-sum">{sessionSummary(session, networkShape)}</span>
            <ChevronDown size={14} strokeWidth={1.8} aria-hidden="true" />
          </summary>
          <div class="form-section-body">
            <div class="form-field">
              <span class="caps-line"><span class="caps">{m.session_init()}</span><OwnMark own={mine('session.init')} onreset={() => reset('session.init')} /></span>
              {#key `${f.driver}:${f.modelId}`}
                <SqlField value={f.init} dialect={dialectOf(f.driver)} label={m.session_init_label()} hint={INIT_EXAMPLE[f.driver]} onchange={(v) => (f.init = v)} />
              {/key}
              <VarHint text={f.init} {preview} />
              <p class="hint">{m.session_init_hint()}</p>
            </div>
            <div class="form-row" class:three={networkShape} class:two={!networkShape}>
              {#if networkShape}
                <label class="form-field">
                  <span class="caps-line"><span class="caps">{m.session_keepalive()}</span><OwnMark own={mine('session.keepalive_secs')} onreset={() => reset('session.keepalive_secs')} /></span>
                  <Select class="field" label={m.session_keepalive()} bind:value={f.keepalive} options={keepaliveChoices.map((s) => ({ value: s, label: s ? m.session_keepalive_every({ seconds: s }) : m.session_keepalive_off() }))} />
                </label>
              {/if}
              <label class="form-field">
                <span class="caps-line"><span class="caps">{m.session_idle()}</span><OwnMark own={mine('session.idle_minutes')} onreset={() => reset('session.idle_minutes')} /></span>
                <Select class="field" label={m.session_idle()} bind:value={f.idle} options={idleChoices.map((n) => ({ value: n, label: n ? m.session_idle_minutes({ minutes: n }) : m.session_idle_off() }))} />
              </label>
              <label class="form-field" class:invalid={bad('rowLimit')}>
                <span class="caps-line"><span class="caps">{m.session_row_limit()}</span><OwnMark own={mine('session.row_limit')} onreset={() => reset('session.row_limit')} /></span>
                <span class="field"><input bind:value={f.rowLimit} inputmode="numeric" placeholder={num(prefs.value.results.row_cap)} autocomplete="off" /></span>
              </label>
            </div>
            <p class="hint" class:warn-note={bad('rowLimit')}>
              {bad('rowLimit') ? m.session_row_limit_bad() : m.session_row_limit_hint({ count: num(prefs.value.results.row_cap) })}
              {#if networkShape && f.keepalive}{m.session_keepalive_hint()}{/if}
            </p>
            {#if networkShape}
              <div class="check-line">
                <label class="check"><input type="checkbox" bind:checked={f.reconnect} /> <span>{m.session_reconnect()}</span></label>
                <OwnMark own={mine('session.reconnect')} onreset={() => reset('session.reconnect')} />
              </div>
              <p class="hint">{m.session_reconnect_hint()}</p>
            {/if}
          </div>
        </details>

        <p class="hint var-note">{m.var_field_hint({ example: VAR_EXAMPLE })}</p>

        {#if hostKey}
          <div class="hostkey" role="alert" bind:this={hostKeyBox}>
            <strong>{m.hostkey_title()}</strong>
            <p>{m.hostkey_body({ host: hostKey.host })}</p>
            <code>{hostKey.fingerprint}</code>
            <button class="btn" type="button" onclick={() => void trust()}>{m.hostkey_trust()}</button>
          </div>
        {/if}
      </div>

      <footer class="dialog-foot">
        {#if editing}
          <button class="btn ghost danger-text" type="button" bind:this={deleteButton} onclick={() => void remove()}>
            {confirmDelete ? m.conn_delete_confirm({ name: initial?.name ?? '' }) : m.conn_delete()}
          </button>
        {/if}
        <p class="test-result" class:ok={result?.ok} role="status">{result?.text ?? ''}</p>
        <button class="btn" type="button" disabled={busy !== null} onclick={() => void test()}>{busy === 'test' ? m.form_testing() : m.form_test()}</button>
        <button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>
        <button class="btn primary" type="submit" disabled={busy !== null}>{m.form_save()}</button>
      </footer>
    </div>
  </form>
</dialog>
