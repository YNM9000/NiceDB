<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { AppInfo } from '../lib/about-api';
  import { appearance } from '../lib/appearance.svelte';
  import { update } from '../lib/update.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';
  import UpdateRelease from './UpdateRelease.svelte';

  let info = $state<AppInfo | null>(null);
  onMount(() => {
    void update.loadPrefs();
    void api.appInfo().then((i) => (info = i));
  });

  const when = (ms: number | null | undefined) =>
    ms ? new Intl.DateTimeFormat(appearance.value.locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms)) : m.upd_never();
</script>

<section class="pref-section upd-page" aria-labelledby="pref-updates">
  <header class="pref-head">
    <h2 class="display" id="pref-updates">{m.settings_updates()}</h2>
  </header>

  <div class="setting row">
    <h3 class="caps">{m.upd_version()}</h3>
    <span class="upd-value mono" data-testid="upd-version">{info?.version ?? '…'}</span>
  </div>
  <div class="setting row">
    <h3 class="caps">{m.upd_check_on_start()}</h3>
    <Segmented
      label={m.upd_check_on_start()}
      value={update.prefs?.check_on_start === false ? 'off' : 'on'}
      options={[
        { value: 'on', label: m.upd_yes() },
        { value: 'off', label: m.upd_no() },
      ] as const}
      onchange={(v) => void update.setCheckOnStart(v === 'on')}
    />
  </div>
  <p class="hint">{m.upd_check_hint()}</p>
  <div class="setting row">
    <h3 class="caps">{m.upd_last_check()}</h3>
    <span class="upd-check">
      <span class="upd-value" data-testid="upd-last">{when(update.prefs?.last_check)}</span>
      <button class="btn compact" type="button" disabled={update.busy} onclick={() => void update.check()}>
        <span class="upd-spin" class:on={update.phase === 'checking'}><RefreshCw size={12} strokeWidth={1.8} aria-hidden="true" /></span>
        {update.phase === 'checking' ? m.upd_checking() : m.upd_check_now()}
      </button>
    </span>
  </div>

  <div class="upd-state" aria-live="polite">
    {#if update.release}
      <UpdateRelease />
    {:else if update.phase === 'up_to_date'}
      <p class="upd-ok">{m.upd_up_to_date()}</p>
    {:else if update.phase === 'error'}
      <p class="upd-error" role="alert">{update.error}</p>
    {/if}
  </div>

  <p class="hint upd-trust">{m.upd_trust()}</p>
  {#if info}
    <div class="setting row">
      <h3 class="caps">{m.upd_source()}</h3>
      <span class="upd-value upd-url mono" data-tip={info.update_url}>{info.update_url}</span>
    </div>
  {/if}
</section>
