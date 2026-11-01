<script lang="ts">
  import { Search, X } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { GROUPS, searchPages, SLUGS, type GroupId, type Slug } from '../lib/manual';
  import { animUrl, closeHelp, help, manualPages, openHelp } from '../lib/manual.svelte';
  import { m } from '../paraglide/messages';

  const GROUP_NAME: Record<GroupId, () => string> = {
    basics: m.help_group_basics,
    query: m.help_group_query,
    data: m.help_group_data,
    schema: m.help_group_schema,
    server: m.help_group_server,
    families: m.help_group_families,
    app: m.help_group_app,
  };

  let { side = false }: { side?: boolean } = $props();

  let root = $state<HTMLElement>();
  let article = $state<HTMLElement>();
  let field = $state<HTMLInputElement>();
  let still = $state(typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches);

  const pages = $derived(manualPages());
  const page = $derived(pages.find((p) => p.slug === help.slug) ?? pages[0]);
  const hits = $derived(help.query.trim() ? searchPages(pages, help.query) : null);
  const at = $derived(SLUGS.indexOf(help.slug));
  const prev = $derived(at > 0 ? pages[at - 1] : undefined);
  const next = $derived(at < SLUGS.length - 1 ? pages[at + 1] : undefined);

  function go(slug: Slug, anchor: string | null = null) {
    help.query = '';
    openHelp(slug, anchor);
  }

  $effect(() => {
    void help.seq;
    void page;
    if (hits) return;
    void tick().then(() => {
      const target = help.anchor ? document.getElementById(help.anchor) : null;
      if (target) target.scrollIntoView({ block: 'start' });
      else article?.closest('.sheet')?.scrollTo({ top: 0 });
    });
  });

  $effect(() => {
    if (help.query.trim()) article?.closest('.sheet')?.scrollTo({ top: 0 });
  });

  $effect(() => {
    void page;
    if (!article || hits) return;
    for (const fig of article.querySelectorAll<HTMLElement>('figure.man-anim')) {
      const name = fig.dataset.anim ?? '';
      const src = animUrl(name, still);
      let img = fig.querySelector('img');
      let btn = fig.querySelector<HTMLButtonElement>('button.man-anim-play');
      if (!src) {
        fig.dataset.missing = m.help_missing();
        img?.remove();
        btn?.remove();
        continue;
      }
      delete fig.dataset.missing;
      if (!img) {
        img = document.createElement('img');
        img.alt = fig.querySelector('figcaption')?.textContent ?? '';
        img.decoding = 'async';
        fig.prepend(img);
      }
      if (img.getAttribute('src') !== src) img.src = src;
      if (!btn) {
        btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'man-anim-play';
        btn.addEventListener('click', () => (still = !still));
        img.after(btn);
      }
      btn.dataset.state = still ? 'paused' : 'playing';
      btn.setAttribute('aria-label', still ? m.help_play() : m.help_pause());
      btn.dataset.tip = btn.getAttribute('aria-label') ?? '';
    }
  });

  function onclick(e: MouseEvent) {
    const a = (e.target as Element | null)?.closest('a');
    if (!a) return;
    e.preventDefault();
    const slug = a.dataset.page;
    if (slug && SLUGS.includes(slug as Slug)) go(slug as Slug);
  }

  function navKey(e: KeyboardEvent) {
    const items = [...((e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button.man-page'))];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: items.length - 1 }[e.key];
    if (to === undefined || i < 0) return;
    e.preventDefault();
    items[(to + items.length) % items.length]?.focus();
  }

  function fieldKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && help.query) {
      e.preventDefault();
      e.stopPropagation();
      help.query = '';
    } else if (e.key === 'Enter' && hits?.[0]) {
      e.preventDefault();
      go(hits[0].slug, hits[0].heading?.id ?? null);
    } else if (e.key === 'ArrowDown' && hits?.length) {
      e.preventDefault();
      article?.querySelector<HTMLButtonElement>('.man-hit')?.focus();
    }
  }

  function hitKey(e: KeyboardEvent) {
    const items = [...(article?.querySelectorAll<HTMLButtonElement>('.man-hit') ?? [])];
    const i = items.indexOf(e.currentTarget as HTMLButtonElement);
    if (e.key === 'ArrowDown') items[i + 1]?.focus();
    else if (e.key === 'ArrowUp') (items[i - 1] ?? field)?.focus();
    else return;
    e.preventDefault();
  }

  function onwindowkey(e: KeyboardEvent) {
    if (side && !(e.target instanceof Node && root?.contains(e.target))) return;
    const typing = (e.target as Element | null)?.closest('input, textarea, [contenteditable]');
    if (e.key === '/' && !typing && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      field?.focus();
    } else if (e.key === 'Escape' && !e.defaultPrevented && !document.querySelector('dialog[open]')) {
      e.preventDefault();
      closeHelp();
    } else if (e.altKey && !typing && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      const to = e.key === 'ArrowLeft' ? prev : next;
      if (!to) return;
      e.preventDefault();
      go(to.slug);
    }
  }
</script>

<svelte:window onkeydown={onwindowkey} />

{#snippet groups()}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="man-groups" onkeydown={navKey}>
    {#each GROUPS as g (g.id)}
      <div class="man-group">
        <h3 class="caps">{GROUP_NAME[g.id]()}</h3>
        <ul>
          {#each g.pages as slug (slug)}
            <li>
              <button type="button" class="man-page" aria-current={!hits && help.slug === slug ? 'page' : undefined} onclick={() => go(slug)}>
                {pages.find((p) => p.slug === slug)?.title}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </div>
{/snippet}

<section class="screen manual" class:side bind:this={root} aria-label={m.help_title()}>
  <nav class="man-nav" aria-label={m.help_index()}>
    <label class="field man-search">
      <Search size={13} strokeWidth={1.8} aria-hidden="true" />
      <input bind:this={field} type="search" placeholder={m.help_search()} data-tip="{m.help_search()} (/)" aria-label={m.help_search()} spellcheck="false" autocomplete="off" bind:value={help.query} onkeydown={fieldKey} />
    </label>
    {#if side}
      <details class="man-index">
        <summary class="caps">{m.help_index()}</summary>
        {@render groups()}
      </details>
    {:else}
      {@render groups()}
    {/if}
  </nav>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <article class="man-article" bind:this={article} {onclick}>
    {#if hits}
      <header class="man-head">
        <h2 class="display">{m.help_search()}</h2>
        <button class="icon-btn man-close" type="button" aria-label={m.help_close()} data-tip={m.help_close()} onclick={closeHelp}><X size={15} strokeWidth={1.7} /></button>
        <p class="hint" role="status">{hits.length ? m.help_results({ count: hits.length }) : m.help_search_none({ query: help.query.trim() })}</p>
      </header>
      <ul class="man-hits">
        {#each hits as h (h.slug)}
          <li>
            <button type="button" class="man-hit" onkeydown={hitKey} onclick={() => go(h.slug, h.heading?.id ?? null)}>
              <span class="man-hit-title">{h.title}{#if h.heading}<span class="man-hit-sep" aria-hidden="true">›</span>{h.heading.text}{/if}</span>
              <span class="man-hit-text">{h.snippet}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else if page}
      <header class="man-head">
        <h2 class="display">{page.title}</h2>
        <button class="icon-btn man-close" type="button" aria-label={m.help_close()} data-tip={m.help_close()} onclick={closeHelp}><X size={15} strokeWidth={1.7} /></button>
      </header>
      <div class="md man-body">{@html page.html}</div>
      <footer class="man-foot">
        {#if prev}<button type="button" class="btn ghost compact" onclick={() => go(prev.slug)}><span class="hint">{m.help_prev()}</span>{prev.title}</button>{:else}<span></span>{/if}
        {#if next}<button type="button" class="btn ghost compact" onclick={() => go(next.slug)}><span class="hint">{m.help_next()}</span>{next.title}</button>{/if}
      </footer>
    {/if}
  </article>

  {#if page && !hits && !side && page.headings.length > 2}
    <aside class="man-toc" aria-label={m.help_contents()}>
      <h3 class="caps">{m.help_contents()}</h3>
      <ul>
        {#each page.headings.filter((h) => h.level === 2) as h (h.id)}
          <li><a href="#{h.id}" onclick={(e) => (e.preventDefault(), document.getElementById(h.id)?.scrollIntoView({ block: 'start' }))}>{h.text}</a></li>
        {/each}
      </ul>
    </aside>
  {/if}
</section>
