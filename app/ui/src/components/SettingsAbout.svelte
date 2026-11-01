<script lang="ts">
  import { ExternalLink } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { AppInfo } from '../lib/about-api';
  import { entryKey, groupLicences, loadLicences, type LicenceEntry, type LicenceSource, type Licences } from '../lib/licenses';
  import { showToast } from '../lib/toast.svelte';
  import { updateErrorText } from '../lib/update.svelte';
  import { m } from '../paraglide/messages';
  import Wordmark from './Wordmark.svelte';

  let info = $state<AppInfo | null>(null);
  let data = $state<Licences | null>(null);
  let failed = $state(false);
  let query = $state('');
  let open = $state<Record<string, boolean>>({});
  let groupOpen = $state<Record<string, boolean>>({});

  onMount(() => {
    void api.appInfo().then((i) => (info = i));
    loadLicences().then((d) => (data = d), () => (failed = true));
  });

  const groups = $derived(data ? groupLicences(data.entries, query) : []);
  const searching = $derived(query.trim().length > 0);
  const SOURCE: Record<LicenceSource, () => string> = { crate: m.about_src_crate, npm: m.about_src_npm, font: m.about_src_font, sound: m.about_src_sound, icon: m.about_src_icon };
  const LINKS = [
    { url: 'https://github.com/maxib0n/NiceDB', label: m.about_link_code, shown: 'github.com/maxib0n/NiceDB' },
    { url: 'https://github.com/maxib0n', label: m.about_link_author, shown: 'github.com/maxib0n' },
    { url: 'https://www.gnu.org/licenses/gpl-3.0.html', label: m.about_link_licence, shown: 'gnu.org/licenses/gpl-3.0' },
  ];

  function go(url: string) {
    api.openLink(url).catch((e: unknown) => showToast(updateErrorText(e), null, 'error'));
  }

  const textOf = (e: LicenceEntry) => (data ? e.texts.map((t) => data?.texts[t] ?? '').join('\n\n――――\n\n') : '');
</script>

<section class="pref-section about-page" aria-labelledby="pref-about">
  <header class="pref-head about-head">
    <h2 class="display" id="pref-about">{m.settings_about()}</h2>
  </header>

  <div class="about-id">
    <Wordmark size={34} text />
    <p class="about-desc">{m.about_desc()}</p>
  </div>

  <div class="setting row">
    <h3 class="caps">{m.about_version()}</h3>
    <span class="upd-value mono" data-testid="about-version">{info?.version ?? '…'}</span>
  </div>
  <div class="setting row">
    <h3 class="caps">{m.about_build()}</h3>
    <span class="upd-value mono">{info ? [info.built, info.commit].filter(Boolean).join(' · ') || '—' : '…'}</span>
  </div>
  <div class="setting row">
    <h3 class="caps">{m.about_platform()}</h3>
    <span class="upd-value mono">{info?.platform ?? '…'}</span>
  </div>
  <div class="setting row about-links-row">
    <h3 class="caps">{m.about_links()}</h3>
    <ul class="about-links">
      {#each LINKS as l (l.url)}
        <li>
          <span class="about-link-label">{l.label()}</span>
          <button type="button" class="about-link" onclick={() => go(l.url)}>{l.shown}<ExternalLink size={11} strokeWidth={1.8} aria-hidden="true" /></button>
        </li>
      {/each}
    </ul>
  </div>

  <div class="setting row">
    <h3 class="caps">{m.about_licence()}</h3>
    <details class="about-own">
      <summary><span class="lic-tag mono">GPL-3.0-or-later</span>{m.about_show_text()}</summary>
      {#if data}<pre class="lic-text">{data.app.text}</pre>{/if}
    </details>
  </div>

  <div class="about-third">
    <h3 class="caps about-third-title">{m.about_third()}</h3>
    {#if failed}
      <p class="hint">{m.about_load_failed()}</p>
    {:else if !data}
      <p class="hint">{m.about_loading()}</p>
    {:else}
      <p class="hint">{m.about_third_hint({ count: data.entries.length })}</p>
      <label class="field about-find">
        <input type="search" placeholder={m.about_search()} aria-label={m.about_search()} bind:value={query} spellcheck="false" autocomplete="off" />
      </label>
      {#if groups.length === 0}
        <p class="hint about-none">{m.about_none()}</p>
      {/if}
      <div class="lic-groups" data-testid="licence-groups">
        {#each groups as g (g.license)}
          <details class="lic-group" open={searching || groupOpen[g.license]} ontoggle={(e) => (groupOpen[g.license] = (e.currentTarget as HTMLDetailsElement).open)}>
            <summary>
              <span class="lic-name mono">{g.license}</span>
              <span class="lic-count">{g.entries.length === 1 ? m.about_count_one() : m.about_count({ count: g.entries.length })}</span>
            </summary>
            {#if searching || groupOpen[g.license]}
              <ul class="lic-entries">
                {#each g.entries as e (entryKey(e))}
                  {@const k = entryKey(e)}
                  <li>
                    <details class="lic-entry" ontoggle={(ev) => (open[k] = (ev.currentTarget as HTMLDetailsElement).open)}>
                      <summary>
                        <span class="lic-pkg">{e.name}</span>
                        {#if e.version}<span class="lic-ver mono">{e.version}</span>{/if}
                        <span class="lic-src">{SOURCE[e.source]()}</span>
                      </summary>
                      {#if open[k]}
                        {#if e.url}<p class="lic-url mono">{e.url}</p>{/if}
                        {#if e.standard}<p class="hint">{m.about_standard()}</p>{/if}
                        <pre class="lic-text">{textOf(e)}</pre>
                      {/if}
                    </details>
                  </li>
                {/each}
              </ul>
            {/if}
          </details>
        {/each}
      </div>
    {/if}
  </div>
</section>
