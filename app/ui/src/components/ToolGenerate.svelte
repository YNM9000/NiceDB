<script lang="ts">
  import { Dices, Save, Shuffle, ShieldAlert, Trash2, TriangleAlert } from '@lucide/svelte';
  import { onDestroy, untrack } from 'svelte';
  import { api, type GenDone, type GenPreview, type GenProfile, type GenSaved } from '../lib/api';
  import type { ColumnGen_Serialize as ColumnGen, DataLocale, FakeColumn_Serialize as FakeColumn, Gen_Serialize as Gen } from '../lib/bindings';
  import { num } from '../lib/format';
  import { applyProfile, columnNote, DISTS, distOf, faultText, KIND_LABEL, kindsFor, localized, LOCALES, locked, newSeed, PARENT_ROWS, parseWeighted, percent, profilesFor, provinceLabel, ranged, ratio, recipe, ROW_CHOICES, ROWS_MAX, stepUnit, switchKind, takeGenerateSeed, weightedText, type DistKind, type GenKind } from '../lib/generate';
  import { duration } from '../lib/tools';
  import { confirmToolWrite, errorText, Job } from '../lib/tools.svelte';
  import type { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import JobBar from './JobBar.svelte';
  import DateField from './DateField.svelte';
  import Select from './Select.svelte';
  import { suggest } from '../lib/suggest';
  import { appearance } from '../lib/appearance.svelte';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const start = untrack(() => takeGenerateSeed());
  const production = $derived(session.info.environment === 'production');
  const tables = $derived(
    (session.schema?.namespaces ?? []).flatMap((n) => n.tables.map((t) => ({ namespace: n.name, name: t.name, key: `${n.name}\u0000${t.name}` }))),
  );
  const named = $derived(new Set(tables.map((t) => t.namespace)).size > 1);

  let chosenKey = $state<string | null>(null);
  let rows = $state(100);
  let seed = $state(newSeed());
  let parentsOn = $state(false);
  let parentRows = $state(PARENT_ROWS);
  let profile = $state.raw<GenProfile | null>(null);
  const RULE_LABEL = { column: m.gen_rule_column, row: m.gen_rule_row, server: m.gen_rule_server } as const;
  const RULE_TIP = { column: m.gen_rule_column_tip, row: m.gen_rule_row_tip, server: m.gen_rule_server_tip } as const;
  let chosen = $state<Record<string, ColumnGen>>({});
  let preview = $state.raw<GenPreview | null>(null);
  let failure = $state<string | null>(null);
  let done = $state.raw<GenDone | null>(null);
  let current = $state('');
  let locale = $state<DataLocale>(appearance.value.locale === 'it' ? 'it' : 'uk');
  let saved = $state.raw<GenSaved[]>([]);
  let profileId = $state('');
  let naming = $state<string | null>(null);
  const job = new Job();

  void api
    .genProfilesGet()
    .then((p) => (saved = p.profiles))
    .catch(() => {});

  $effect(() => {
    if (chosenKey || !tables.length) return;
    const want = start ? tables.find((t) => t.name === start.name && (!start.namespace || t.namespace === start.namespace)) : undefined;
    untrack(() => (chosenKey = (want ?? tables[0])?.key ?? null));
  });

  const table = $derived(tables.find((t) => t.key === chosenKey) ?? null);

  $effect(() => {
    const t = table;
    const sid = session.info.session_id;
    if (!t || production) return;
    let live = true;
    profile = null;
    preview = null;
    failure = null;
    done = null;
    api
      .genProfile(sid, t.namespace, t.name)
      .then((p) => {
        if (!live) return;
        profile = p;
        chosen = Object.fromEntries(p.columns.map((c) => [c.name, structuredClone(c.suggested)]));
        profileId = '';
        naming = null;
        parentsOn = p.foreign_keys.some((f) => !f.own && (f.parent_rows ?? 0) === 0);
      })
      .catch((e) => live && (failure = errorText(e)));
    return () => {
      live = false;
    };
  });

  const validRows = $derived(Number.isInteger(rows) && rows >= 1 && rows <= ROWS_MAX);
  const parents = $derived(parentsOn && profile?.ancestors.length ? Math.max(1, Math.min(ROWS_MAX, Math.round(parentRows) || 1)) : null);

  function request(jobId: string) {
    if (!profile) return null;
    return {
      job_id: jobId,
      session_id: session.info.session_id,
      namespace: profile.namespace,
      name: profile.name,
      columns: recipe(profile.columns, $state.snapshot(chosen) as Record<string, ColumnGen>),
      rows: validRows ? rows : 20,
      seed: Math.max(0, Math.round(seed) || 0),
      parents,
      locale,
    };
  }

  $effect(() => {
    const req = request('preview');
    if (!req || production) return;
    let live = true;
    const timer = setTimeout(() => {
      api
        .genPreview(req)
        .then((p) => live && (preview = p))
        .catch((e) => live && (failure = errorText(e)));
    }, 200);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  });

  function setKind(c: FakeColumn, kind: GenKind) {
    const now = chosen[c.name] ?? c.suggested;
    const generator = switchKind(c, now.generator, kind);
    chosen[c.name] = { ...now, generator, null_ratio: kind === 'auto' || kind === 'null' ? 0 : now.null_ratio, dist: ranged(generator) ? now.dist : { kind: 'uniform' } };
  }

  function setDist(c: FakeColumn, kind: DistKind, step?: number) {
    const now = chosen[c.name] ?? c.suggested;
    const was = now.dist.kind === 'sequential' ? (now.dist.step ?? 1) : 1;
    chosen[c.name] = { ...now, dist: distOf(kind, step ?? was) };
  }

  function setLocale(c: FakeColumn, value: string) {
    const now = chosen[c.name] ?? c.suggested;
    chosen[c.name] = { ...now, locale: value ? (value as DataLocale) : null };
  }

  const tableRef = $derived(profile ? { connectionId: session.info.connection_id, namespace: profile.namespace, name: profile.name } : null);
  const lists = $derived(tableRef ? profilesFor(saved, tableRef) : { own: [], other: [] });

  function useProfile(id: string) {
    profileId = id;
    const p = saved.find((x) => x.id === id);
    if (!p || !profile) return;
    chosen = applyProfile(profile.columns, p);
    locale = p.locale;
    seed = p.seed;
    rows = p.rows;
    parentsOn = p.parents !== null && profile.ancestors.length > 0;
    if (p.parents !== null) parentRows = p.parents;
    if (!lists.own.some((x) => x.id === id)) profileId = '';
  }

  async function saveProfile(name: string) {
    if (!profile || !name.trim()) return;
    const mine = lists.own.find((x) => x.id === profileId);
    const entry: GenSaved = {
      id: mine && mine.name === name.trim() ? mine.id : crypto.randomUUID(),
      name: name.trim(),
      connection_id: session.info.connection_id,
      database: session.database || null,
      namespace: profile.namespace,
      table: profile.name,
      locale,
      seed: Math.max(0, Math.round(seed) || 0),
      rows: validRows ? rows : 100,
      parents,
      columns: recipe(profile.columns, $state.snapshot(chosen) as Record<string, ColumnGen>),
      saved_ms: Date.now(),
    };
    try {
      saved = (await api.genProfileSave(entry)).profiles;
      profileId = entry.id;
      naming = null;
    } catch (e) {
      failure = errorText(e);
    }
  }

  async function dropProfile() {
    if (!profileId) return;
    try {
      saved = (await api.genProfileDelete(profileId)).profiles;
      profileId = '';
    } catch (e) {
      failure = errorText(e);
    }
  }

  const currentName = $derived(lists.own.find((x) => x.id === profileId)?.name ?? '');

  function setGen(c: FakeColumn, patch: Partial<Gen>) {
    const now = chosen[c.name] ?? c.suggested;
    chosen[c.name] = { ...now, generator: { ...now.generator, ...patch } as Gen };
  }

  function setNulls(c: FakeColumn, pct: number) {
    const now = chosen[c.name] ?? c.suggested;
    chosen[c.name] = { ...now, null_ratio: ratio(pct) };
  }

  const numberOf = (e: Event) => Number((e.currentTarget as HTMLInputElement).value);
  const textOf = (e: Event) => (e.currentTarget as HTMLInputElement).value;

  const ancestorNames = $derived(profile?.ancestors.map((a) => a.name).join(' → ') ?? '');

  async function run(e: SubmitEvent) {
    e.preventDefault();
    if (!profile || !validRows || job.running) return;
    const lines = [m.gen_confirm_line({ n: rows, count: num(rows), table: profile.name })];
    if (parents) lines.unshift(m.gen_confirm_parents({ rows: num(parents), tables: ancestorNames }));
    const ok = await confirmToolWrite(session.info, m.gen_confirm_title(), lines, m.gen_confirm_go(), m.gen_confirm_note());
    if (ok === false) return;
    done = null;
    failure = null;
    current = profile.name;
    try {
      done = await job.run((id, onProgress) => {
        const req = request(id);
        if (!req) throw new Error(m.tool_fault_empty());
        return api.genRun(req, (p) => {
          if (p.stage) current = p.stage;
          onProgress(p);
        });
      });
      if (done?.committed && profile) {
        const p = profile;
        api
          .genProfile(session.info.session_id, p.namespace, p.name)
          .then((fresh) => {
            for (const c of fresh.columns) {
              const g = chosen[c.name]?.generator;
              if (g?.kind === 'sequence' && c.suggested.generator.kind === 'sequence') chosen[c.name] = { ...chosen[c.name]!, generator: c.suggested.generator };
            }
            profile = fresh;
          })
          .catch(() => {});
      }
    } catch (err) {
      failure = errorText(err);
    }
  }

  onDestroy(() => void job.cancel());

  const genOf = (c: FakeColumn) => (chosen[c.name] ?? c.suggested).generator;
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_generate()}</h2>
    <p class="hint">{m.gen_lead()}</p>
    {#if session.info.dialect === 'clickhouse'}<p class="hint gen-batches">{m.gen_batches()}</p>{/if}
  </div>
</header>

{#if production}
  <div class="gen-blocked" role="note">
    <ShieldAlert size={18} strokeWidth={1.7} aria-hidden="true" />
    <div>
      <h3>{m.gen_prod_title()}</h3>
      <p>{m.gen_prod_body()}</p>
      <p class="hint">{m.gen_prod_how()}</p>
    </div>
  </div>
{:else if !tables.length}
  <p class="tool-empty">{m.gen_no_tables()}</p>
{:else}
  <form class="gen-form" onsubmit={(e) => void run(e)}>
    <div class="gen-line">
      <label class="gen-opt">
        <span class="caps gen-label">{m.gen_table()}</span>
        <Select class="tool-select mono gen-table" label={m.gen_table()} bind:value={chosenKey} options={tables.map((t) => ({ value: t.key, label: named ? `${t.namespace}.${t.name}` : t.name }))} />
      </label>
      <label class="gen-opt">
        <span class="caps gen-label">{m.gen_rows()}</span>
        <input class="gen-input gen-rows" type="number" min="1" max={ROWS_MAX} step="1" use:suggest={{ options: ROW_CHOICES, label: m.gen_rows() }} bind:value={rows} aria-invalid={!validRows} data-tip={m.gen_rows_max()} />
      </label>
      <label class="gen-opt">
        <span class="caps gen-label">{m.gen_seed()}</span>
        <input class="gen-input gen-seed mono" type="number" min="0" step="1" bind:value={seed} />
      </label>
      <button type="button" class="icon-btn gen-dice" aria-label={m.gen_seed_new()} data-tip={m.gen_seed_new()} onclick={() => (seed = newSeed())}><Shuffle size={15} strokeWidth={1.7} /></button>
      <span class="spacer"></span>
      <button class="btn primary" type="submit" disabled={!profile || !validRows || job.running}>
        <Dices size={14} strokeWidth={1.8} aria-hidden="true" />
        {m.gen_go({ n: validRows ? rows : 0, count: num(validRows ? rows : 0) })}
      </button>
    </div>
    <div class="gen-line gen-profiles">
      <label class="gen-opt">
        <span class="caps gen-label">{m.gen_locale()}</span>
        <Select class="tool-select gen-locale" label={m.gen_locale()} bind:value={locale} options={LOCALES.map((l) => ({ value: l.id, label: l.label() }))} />
      </label>
      <label class="gen-opt" class:naming={naming !== null}>
        <span class="caps gen-label">{m.gen_profile()}</span>
        <Select
          class="tool-select gen-profile"
          label={m.gen_profile()}
          value={profileId}
          disabled={!profile}
          onchange={(v) => useProfile(v)}
          options={[
            { value: '', label: m.gen_profile_none() },
            ...lists.own.map((p) => ({ value: p.id, label: p.name, group: m.gen_profile_own() })),
            ...lists.other.map((p) => ({ value: p.id, label: `${p.name} · ${p.table}`, group: m.gen_profile_other() })),
          ]}
        />
      </label>
      {#if naming !== null}
        <input
          class="gen-input gen-name-input"
          bind:value={naming}
          placeholder={m.gen_profile_name()}
          aria-label={m.gen_profile_name()}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void saveProfile(naming ?? '');
            } else if (e.key === 'Escape') {
              e.preventDefault();
              naming = null;
            }
          }}
        />
        <button type="button" class="btn" disabled={!naming?.trim()} onclick={() => void saveProfile(naming ?? '')}>{m.gen_profile_save()}</button>
        <button type="button" class="btn ghost" onclick={() => (naming = null)}>{m.form_cancel()}</button>
      {:else}
        <button type="button" class="btn ghost" disabled={!profile} onclick={() => (naming = currentName || m.gen_profile_default({ table: profile?.name ?? '' }))}>
          <Save size={13} strokeWidth={1.8} aria-hidden="true" />
          {profileId ? m.gen_profile_update() : m.gen_profile_save_dots()}
        </button>
        {#if profileId}
          <button type="button" class="icon-btn" aria-label={m.gen_profile_delete()} data-tip={m.gen_profile_delete()} onclick={() => void dropProfile()}><Trash2 size={14} strokeWidth={1.7} /></button>
        {/if}
      {/if}
    </div>
    {#if profile?.ancestors.length}
      <div class="gen-line">
        <label class="check gen-parents">
          <input type="checkbox" bind:checked={parentsOn} />
          {m.gen_parents()}
        </label>
        {#if parentsOn}
          <label class="gen-opt">
            <input class="gen-input gen-small" type="number" min="1" max={ROWS_MAX} step="1" bind:value={parentRows} aria-label={m.gen_parents_rows()} />
            <span class="hint">{m.gen_parents_rows()}</span>
          </label>
          <span class="hint mono gen-order">{m.gen_parents_order({ tables: ancestorNames })}</span>
        {/if}
      </div>
    {/if}
  </form>

  {#if job.running}
    <div class="gen-job"><JobBar {job} label={current ? `${m.gen_job()} ${m.gen_job_in({ table: current })}` : m.gen_job()} /></div>
  {/if}
  {#if failure}<p class="tool-note err" role="alert">{failure}</p>{/if}
  {#if done}
    {#if done.committed}
      <p class="tool-done gen-result" role="status">
        {m.gen_done({ rows: num(done.inserted ?? 0), time: duration(done.elapsed_ms) })}
        {#if done.tables.length > 1}<span class="hint">· {done.tables.map((t) => m.gen_done_table({ rows: num(t.rows ?? 0), table: t.name })).join(', ')}</span>{/if}
      </p>
    {:else}
      <p class="tool-warn gen-result" role="alert">
        <TriangleAlert size={14} strokeWidth={1.8} aria-hidden="true" />
        {#if done.kept}
          {@const rows = num(done.inserted ?? 0)}
          <span class="gen-msg">
            {#if done.fault}{faultText(done.fault)}<span class="gen-kept">{done.inserted ? m.gen_kept({ rows }) : m.gen_kept_none()}</span>{:else}{m.gen_stopped_kept({ rows })}{/if}
            {#if done.tables.length > 1}<span class="hint">· {done.tables.map((t) => m.gen_done_table({ rows: num(t.rows ?? 0), table: t.name })).join(', ')}</span>{/if}
          </span>
        {:else if done.fault}{faultText(done.fault)} {m.gen_rolled_back()}{:else}{m.gen_cancelled()}{/if}
      </p>
    {/if}
  {/if}

  <div class="tool-scroll gen-body">
    {#if !profile}
      {#if !failure}<p class="tool-empty">{m.gen_loading()}</p>{/if}
    {:else}
      <table class="tool-table gen-cols">
        <thead>
          <tr>
            <th>{m.gen_col_column()}</th>
            <th>{m.gen_col_values()}</th>
            <th>{m.gen_col_params()}</th>
            <th class="num">{m.gen_col_nulls()}</th>
          </tr>
        </thead>
        <tbody>
          {#each profile.columns as c (c.name)}
            {@const g = genOf(c)}
            {@const note = columnNote(c)}
            <tr class:server={g.kind === 'auto'}>
              <td class="gen-name">
                <span class="mono strong">{c.name}</span>
                {#if c.primary_key}<span class="gen-badge">{m.gen_badge_pk()}</span>{:else if c.unique}<span class="gen-badge">{m.gen_badge_unique()}</span>{/if}
                <span class="sub"><span class="mono">{c.type_name}</span>{#if !c.nullable && !c.primary_key}&nbsp;· {m.gen_badge_required()}{/if}{#if note}&nbsp;· {note}{/if}</span>
              </td>
              <td class="gen-kind">
                {#if locked(c)}
                  <span class="hint">{m.gen_server()}</span>
                {:else}
                  <Select
                    class="tool-select"
                    label={m.gen_kind_label({ column: c.name })}
                    value={g.kind}
                    onchange={(v) => setKind(c, v)}
                    options={kindsFor(c).flatMap((group) => group.kinds.map((k) => ({ value: k, label: k === 'province' ? provinceLabel((chosen[c.name] ?? c.suggested).locale ?? locale) : KIND_LABEL[k](), group: group.label() })))}
                  />
                  {#if ranged(g)}
                    {@const d = (chosen[c.name] ?? c.suggested).dist}
                    <span class="gen-dist">
                      <Select class="tool-select gen-small-select" label={m.gen_dist_label({ column: c.name })} value={d.kind} onchange={(v) => setDist(c, v)} options={DISTS.map((x) => ({ value: x.id, label: x.label() }))} />
                    </span>
                  {/if}
                {/if}
              </td>
              <td class="gen-params"><div class="gen-pline">
                {#if g.kind === 'integer'}
                  <label>{m.gen_p_min()} <input class="gen-input gen-small" type="number" step="1" value={g.min} onchange={(e) => setGen(c, { min: numberOf(e) })} /></label>
                  <label>{m.gen_p_max()} <input class="gen-input gen-small" type="number" step="1" value={g.max} onchange={(e) => setGen(c, { max: numberOf(e) })} /></label>
                {:else if g.kind === 'sequence'}
                  <label>{m.gen_p_start()} <input class="gen-input gen-small" type="number" step="1" value={g.start} onchange={(e) => setGen(c, { start: numberOf(e) })} /></label>
                {:else if g.kind === 'decimal'}
                  <label>{m.gen_p_min()} <input class="gen-input gen-small" type="number" step="any" value={g.min} onchange={(e) => setGen(c, { min: numberOf(e) })} /></label>
                  <label>{m.gen_p_max()} <input class="gen-input gen-small" type="number" step="any" value={g.max} onchange={(e) => setGen(c, { max: numberOf(e) })} /></label>
                  <label>{m.gen_p_scale()} <input class="gen-input gen-tiny" type="number" min="0" max="12" step="1" value={g.scale} onchange={(e) => setGen(c, { scale: Math.max(0, Math.min(12, Math.round(numberOf(e)))) })} /></label>
                {:else if g.kind === 'date' || g.kind === 'date_time'}
                  <span class="gen-pdate">{m.gen_p_from()} <DateField class="gen-date" strict value={g.from} max={g.to} label={m.gen_date_from({ column: c.name })} onchange={(v) => setGen(c, { from: v })} /></span>
                  <span class="gen-pdate">{m.gen_p_to()} <DateField class="gen-date" strict value={g.to} min={g.from} label={m.gen_date_to({ column: c.name })} onchange={(v) => setGen(c, { to: v })} /></span>
                {:else if g.kind === 'lorem'}
                  <label>{m.gen_p_words()} <input class="gen-input gen-tiny" type="number" min="1" max="200" step="1" value={g.min_words} onchange={(e) => setGen(c, { min_words: Math.max(1, Math.round(numberOf(e))) })} /></label>
                  <label>– <input class="gen-input gen-tiny" type="number" min="1" max="200" step="1" value={g.max_words} onchange={(e) => setGen(c, { max_words: Math.max(1, Math.round(numberOf(e))) })} /></label>
                {:else if g.kind === 'code'}
                  <label>{m.gen_p_len()} <input class="gen-input gen-tiny" type="number" min="1" max="64" step="1" value={g.len} onchange={(e) => setGen(c, { len: Math.max(1, Math.min(64, Math.round(numberOf(e)))) })} /></label>
                {:else if g.kind === 'pick'}
                  <input class="gen-input gen-wide mono" value={weightedText(g.values, g.weights)} placeholder={m.gen_p_values()} aria-label={m.gen_p_values()} data-tip={m.gen_p_weights()} onchange={(e) => setGen(c, parseWeighted(textOf(e)))} />
                {:else if g.kind === 'pattern'}
                  <input class="gen-input gen-wide mono" value={g.pattern} placeholder={m.gen_ph_pattern({ year: '{year}', counter: '{0000}' })} aria-label={m.gen_p_pattern()} data-tip={m.gen_p_pattern_help({ counter: '{0000}', year: '{year}', column: `{${m.gen_p_column_example()}|lower}` })} onchange={(e) => setGen(c, { pattern: textOf(e) })} />
                {:else if g.kind === 'expr'}
                  <input class="gen-input gen-wide mono" value={g.expr} placeholder={m.gen_p_expr()} aria-label={m.gen_p_expr()} data-tip={m.gen_p_expr_help()} onchange={(e) => setGen(c, { expr: textOf(e) })} />
                  <label>{m.gen_p_scale()} <input class="gen-input gen-tiny" type="number" min="0" max="12" step="1" value={g.scale} onchange={(e) => setGen(c, { scale: Math.max(0, Math.min(12, Math.round(numberOf(e)))) })} /></label>
                {:else if g.kind === 'fixed'}
                  <input class="gen-input gen-wide mono" value={g.value} placeholder={m.gen_p_value()} aria-label={m.gen_p_value()} onchange={(e) => setGen(c, { value: textOf(e) })} />
                {:else if !localized(g)}
                  <span class="hint">{m.gen_p_none()}</span>
                {/if}
                {#if ranged(g)}
                  {@const d = (chosen[c.name] ?? c.suggested).dist}
                  {#if d.kind === 'sequential'}
                    <label>{m.gen_p_step()} <input class="gen-input gen-tiny" type="number" min="0" step="any" value={d.step} onchange={(e) => setDist(c, 'sequential', numberOf(e))} /> {stepUnit(g)}</label>
                  {/if}
                {/if}
                {#if localized(g)}
                  {@const own = (chosen[c.name] ?? c.suggested).locale}
                  <label class="gen-loc">
                    <Select class="tool-select gen-small-select" label={m.gen_locale_of({ column: c.name })} value={own ?? ''} onchange={(v) => setLocale(c, v)} options={[{ value: '', label: m.gen_locale_run({ locale: LOCALES.find((l) => l.id === locale)?.short ?? '' }) }, ...LOCALES.map((l) => ({ value: l.id, label: l.label() }))]} />
                  </label>
                {/if}
              </div></td>
              <td class="num gen-nulls">
                {#if c.nullable && !locked(c) && g.kind !== 'auto' && g.kind !== 'null'}
                  <input class="gen-input gen-tiny" type="number" min="0" max="100" step="5" value={percent(chosen[c.name]?.null_ratio)} aria-label={m.gen_nulls_label({ column: c.name })} onchange={(e) => setNulls(c, numberOf(e))} />
                {:else}
                  <span class="hint">{m.gen_p_none()}</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>

      {#if profile?.rules.length}
        <section class="gen-rules" aria-label={m.gen_rules()}>
          <h3 class="caps tool-sub">{m.gen_rules()}</h3>
          <ul>
            {#each profile.rules as r, i (i)}
              <li data-kind={r.kind}><code class="mono">{r.expr}</code><span class="gen-rule-kind" data-tip={RULE_TIP[r.kind]()}>{RULE_LABEL[r.kind]()}</span></li>
            {/each}
          </ul>
        </section>
      {/if}

      <section class="gen-preview" aria-label={m.gen_preview()}>
        <div class="tool-subhead">
          <h3 class="caps tool-sub">{m.gen_preview()}</h3>
          <span class="hint">{m.gen_preview_hint()}</span>
        </div>
        {#if preview?.waiting.length}<p class="hint gen-waiting">{m.gen_waiting({ tables: preview.waiting.join(', ') })}</p>{/if}
        {#if preview?.fault}<p class="tool-warn gen-fault"><TriangleAlert size={14} strokeWidth={1.8} aria-hidden="true" /> {faultText(preview.fault)}</p>{/if}
        {#if preview && !preview.columns.length}
          <p class="hint">{m.gen_preview_none()}</p>
        {:else if preview?.rows.length}
          <div class="gen-rows-wrap">
            <table class="tool-table gen-sample">
              <thead><tr>{#each preview.columns as col (col)}<th>{col}</th>{/each}</tr></thead>
              <tbody>
                {#each preview.rows as row, i (i)}
                  <tr>{#each row as cell, j (j)}<td class="mono" class:null={cell === null}>{cell ?? 'NULL'}</td>{/each}</tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </section>
    {/if}
  </div>
{/if}
