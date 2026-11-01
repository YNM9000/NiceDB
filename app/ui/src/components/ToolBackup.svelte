<script lang="ts">
  import { backupBase } from '../lib/file-names';
  import { sqlOf } from '../lib/family';
  import { untrack } from 'svelte';
  import { CircleCheck, DatabaseBackup, FolderSearch, TriangleAlert, Upload } from '@lucide/svelte';
  import { api, type Backed, type ToolCheck } from '../lib/api';
  import type { BackupFormat, ToolFamily } from '../lib/bindings';
  import { bytes, duration, fileName, flavorOf } from '../lib/tools';
  import { confirmToolWrite, errorText, Job } from '../lib/tools.svelte';
  import type { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import JobBar from './JobBar.svelte';
  import Segmented from './Segmented.svelte';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  const family = $derived<ToolFamily>(sqlOf(session.info) === 'mysql' ? 'mysql' : 'postgres');
  const flavor = $derived(flavorOf(sqlOf(session.info), session.info.server.product));
  const dirKey = $derived(`nicedb.tooldir.${family}`);
  let check = $state.raw<ToolCheck | null>(null);
  let failure = $state<string | null>(null);
  let format = $state<BackupFormat>('archive');
  let schemaOnly = $state(false);
  const dump = new Job();
  const restore = new Job();
  let dumped = $state.raw<(Backed & { path: string }) | null>(null);
  let restored = $state.raw<(Backed & { path: string }) | null>(null);
  let serverPath = $state(untrack(() => `${session.info.server.database || 'backup'}-${new Date().toISOString().slice(0, 10)}.${session.info.dialect === 'clickhouse' ? 'zip' : 'bak'}`));
  let dumpError = $state<string | null>(null);
  let restoreError = $state<string | null>(null);

  async function load() {
    try {
      const dir = localStorage.getItem(dirKey);
      if (dir) await api.toolDirSet(family, dir);
      check = await api.backupTools(sid);
      if (!check.formats.includes(format)) format = check.formats[0] ?? 'sql';
      failure = null;
    } catch (e) {
      failure = errorText(e);
    }
  }

  $effect(() => {
    void load();
  });

  async function chooseDir() {
    const dir = await api.pickFolder();
    if (!dir) return;
    localStorage.setItem(dirKey, dir);
    await load();
  }

  const ext = $derived({ archive: 'dump', file: 'sqlite', sql: 'sql', bak: 'bak', zip: 'zip', folder: '' }[format]);
  const onServer = $derived(format === 'bak' || format === 'zip');
  const today = () => new Date().toISOString().slice(0, 10);
  const canDump = $derived(!!check && (check.builtin || !!check.dump));
  const canRestore = $derived(!!check && check.can_restore && (check.builtin || check.restore.length > 0));
  const FORMAT: Record<BackupFormat, () => string> = { archive: m.tool_backup_archive, sql: m.tool_backup_sql, file: m.tool_backup_file, folder: m.tool_backup_folder, bak: m.tool_backup_bak, zip: m.tool_backup_zip };
  const HINT: Record<BackupFormat, () => string> = { archive: m.tool_backup_archive_hint, sql: m.tool_backup_sql_hint, file: m.tool_backup_file_hint, folder: m.tool_backup_folder_hint, bak: m.tool_backup_bak_hint, zip: m.tool_backup_zip_hint };
  const restoreHint = $derived(
    { postgres: m.tool_restore_hint_pg, cockroach: m.tool_restore_hint_pg, mysql: m.tool_restore_hint_mysql, sqlite: m.tool_restore_hint_sqlite, duckdb: m.tool_restore_hint_duckdb, mssql: m.tool_restore_hint_mssql, clickhouse: m.tool_restore_hint_clickhouse, firebird: m.tool_backup_unsupported_firebird, hana: m.tool_backup_unsupported_firebird, db2: m.tool_backup_unsupported_firebird, ansi: m.tool_backup_unsupported_firebird, oracle: m.tool_backup_unsupported_oracle }[flavor](),
  );
  const unsupportedNote = $derived(flavor === 'cockroach' ? m.tool_backup_unsupported_cockroach() : flavor === 'oracle' ? m.tool_backup_unsupported_oracle() : flavor === 'firebird' || flavor === 'hana' || flavor === 'db2' || flavor === 'ansi' ? m.tool_backup_unsupported_firebird() : m.tool_backup_unsupported_libsql());

  async function chooseDumpPath(): Promise<string | null> {
    const name = backupBase(session.info.server.database, today());
    if (onServer) return serverPath.trim() || null;
    if (format === 'folder') {
      const parent = await api.pickFolder();
      return parent ? `${parent.replace(/[\\/]$/, '')}${parent.includes('\\') ? '\\' : '/'}${name}` : null;
    }
    return api.pickSave(`${name}.${ext}`, [ext], FORMAT[format]());
  }

  async function runDump() {
    dumpError = null;
    dumped = null;
    const path = await chooseDumpPath();
    if (!path) return;
    try {
      const out = await dump.run((job_id, onProgress) => api.backupRun({ job_id, session_id: sid, path, format, schema_only: schemaOnly && check?.schema_only === true }, onProgress));
      dumped = { ...out, path };
    } catch (e) {
      dumpError = errorText(e);
    }
  }

  async function runRestore() {
    restoreError = null;
    restored = null;
    const exts = check?.builtin ? ['sqlite', 'sqlite3', 'db'] : family === 'postgres' ? ['dump', 'backup', 'sql'] : ['sql'];
    const path = flavor === 'duckdb' ? await api.pickFolder() : await api.pickOpen(exts, m.tool_backup());
    if (!path) return;
    try {
      const typed = await confirmToolWrite(session.info, m.tool_restore_confirm({ file: fileName(path) }), [flavor === 'mysql' ? m.tool_restore_not_atomic() : flavor === 'duckdb' ? m.tool_restore_import_adds() : check?.builtin ? m.tool_restore_replaces() : m.tool_restore_atomic()], m.tool_restore_go(), '');
      if (typed === false) return;
      const out = await restore.run((job_id, onProgress) => api.backupRestore({ job_id, session_id: sid, path, confirm: typed }, onProgress));
      restored = { ...out, path };
    } catch (e) {
      restoreError = errorText(e);
    }
  }
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_backup()}</h2>
    <p class="hint">{m.tool_backup_lead()}</p>
  </div>
</header>

{#if failure}
  <p class="tool-empty err">{failure}</p>
{:else if check && !check.supported}
  <p class="tool-empty">{unsupportedNote}</p>
{:else if check}
  <div class="tool-scroll">
    <div class="bk-programs">
      {#if check.server_side}
        <p class="bk-tool ok"><CircleCheck size={14} strokeWidth={1.8} aria-hidden="true" /> {flavor === 'clickhouse' ? m.tool_backup_server_note_ch() : m.tool_backup_server_note()}</p>
      {:else if check.builtin}
        <p class="bk-tool ok"><CircleCheck size={14} strokeWidth={1.8} aria-hidden="true" /> {m.tool_backup_builtin()}</p>
      {:else}
        {#each [check.dump, ...check.restore] as t, i (i)}
          {#if t}<p class="bk-tool ok"><CircleCheck size={14} strokeWidth={1.8} aria-hidden="true" /> <b>{t.name}</b> <span class="sub">{t.version ?? ''}</span> <span class="mono sub">{t.path ?? ''}</span></p>{/if}
        {/each}
        {#if !check.dump || !check.restore.length}
          <div class="bk-missing">
            <p><TriangleAlert size={14} strokeWidth={1.8} aria-hidden="true" /> {family === 'postgres' ? m.tool_backup_missing_pg() : m.tool_backup_missing_mysql()}</p>
            <button class="btn" type="button" onclick={() => void chooseDir()}><FolderSearch size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_backup_choose_dir()}</button>
          </div>
        {/if}
      {/if}
    </div>

    <div class="bk-cards">
      <section class="bk-card" aria-labelledby="bk-dump">
        <h3 id="bk-dump" class="bk-title"><DatabaseBackup size={15} strokeWidth={1.7} aria-hidden="true" /> {m.tool_backup_make()}</h3>
        {#if check.formats.length > 1}
          <div class="form-field">
            <span class="caps">{m.tool_backup_format()}</span>
            <Segmented label={m.tool_backup_format()} value={format} options={check.formats.map((f) => ({ value: f, label: FORMAT[f]() }))} onchange={(v) => (format = v)} />
          </div>
        {/if}
        <p class="hint">{HINT[format]()}</p>
        {#if onServer}
          <label class="form-field">
            <span class="caps">{m.tool_backup_server_path()}</span>
            <span class="field"><input bind:value={serverPath} autocomplete="off" spellcheck="false" /></span>
          </label>
        {/if}
        {#if check.schema_only}<label class="check"><input type="checkbox" bind:checked={schemaOnly} /> {m.tool_backup_schema_only()}</label>{/if}
        {#if dump.running}
          <JobBar job={dump} label={m.tool_backup_running()} />
        {:else}
          <div class="bk-actions">
            <button class="btn primary" type="button" disabled={!canDump || (onServer && !serverPath.trim())} onclick={() => void runDump()}>{format === 'folder' ? m.tool_backup_pick_folder() : onServer ? m.tool_backup_run() : m.tool_backup_save()}</button>
          </div>
        {/if}
        {#if dumped}
          <p class="tool-done" role="status">{dumped.cancelled ? m.tool_job_cancelled() : check.server_side && !dumped.bytes ? m.tool_backup_done_server({ time: duration(dumped.elapsed_ms), file: fileName(dumped.path) }) : m.tool_backup_done({ size: bytes(dumped.bytes), time: duration(dumped.elapsed_ms), file: fileName(dumped.path) })}</p>
          {#if dumped.notes}<pre class="tool-output">{dumped.notes}</pre>{/if}
        {/if}
        {#if dumpError}<p class="test-result" role="alert">{dumpError}</p>{/if}
      </section>

      <section class="bk-card" aria-labelledby="bk-restore">
        <h3 id="bk-restore" class="bk-title"><Upload size={15} strokeWidth={1.7} aria-hidden="true" /> {m.tool_restore()}</h3>
        <p class="hint">{restoreHint}</p>
        {#if session.info.environment === 'production'}<p class="prod-note">{m.tool_restore_prod()}</p>{/if}
        {#if restore.running}
          <JobBar job={restore} label={m.tool_restore_running()} />
        {:else}
          <div class="bk-actions">
            <button class="btn danger" type="button" disabled={!canRestore} onclick={() => void runRestore()}>{flavor === 'duckdb' ? m.tool_restore_pick_folder() : m.tool_restore_pick()}</button>
          </div>
        {/if}
        {#if restored}<p class="tool-done" role="status">{restored.cancelled ? m.tool_job_cancelled() : m.tool_restore_done({ file: fileName(restored.path), time: duration(restored.elapsed_ms) })}</p>{/if}
        {#if restoreError}<pre class="tool-output err" role="alert">{restoreError}</pre>{/if}
      </section>
    </div>
  </div>
{:else}
  <p class="tool-empty">{m.schema_loading()}</p>
{/if}
