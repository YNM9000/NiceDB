<script lang="ts">
  import type { Snippet } from 'svelte';
  import { CircleHelp, Settings } from '@lucide/svelte';
  import { m } from '../paraglide/messages';
  import Wordmark from './Wordmark.svelte';
  import WindowControls from './WindowControls.svelte';
  import { selectionStats, statParts, widen } from '../lib/aggregate.svelte';
  import { num } from '../lib/format';
  import { copyText } from '../lib/contextmenu.svelte';

  interface Props {
    title: string;
    settingsOpen: boolean;
    helpOpen?: boolean;
    ontogglehelp?: () => void;
    status: string;
    statusTone?: string | null;
    statusLive?: boolean;
    statusRight: string;
    production: boolean;
    ontogglesettings: () => void;
    sidebar: Snippet;
    switcher?: Snippet;
    actions?: Snippet;
    children: Snippet;
    inert?: boolean;
  }
  function skipTo(selector: string) {
    const inner = document.querySelector<HTMLElement>(`.sheet ${selector}`);
    const target = inner ?? document.querySelector<HTMLElement>('.sheet');
    if (target && !inner) target.tabIndex = -1;
    target?.focus();
  }

  let { title, settingsOpen, helpOpen = false, ontogglehelp, status, statusTone = null, statusLive = false, statusRight, production, ontogglesettings, sidebar, switcher, actions, children, inert = false }: Props = $props();
</script>

<div class="shell" class:production {inert}>
  <button class="skip-link" type="button" onclick={() => skipTo('.cm-content')}>{m.skip_to_editor()}</button>
  <button class="skip-link" type="button" onclick={() => skipTo('[role=grid]')}>{m.skip_to_grid()}</button>
  <header class="titlebar" data-tauri-drag-region="deep">
    <Wordmark />
    <span class="titlebar-sep" aria-hidden="true"></span>
    {@render switcher?.()}
    <h1 class="titlebar-title"><span class="titlebar-name" data-tip={title}>{title}</span></h1>
    {#if production}<span class="env env-production">{m.env_production()}</span>{/if}
    <span class="spacer"></span>
    {@render actions?.()}
    {#if ontogglehelp}<button class="icon-btn titlebar-help" type="button" aria-label={m.help_button()} data-tip="{m.help_button()} (F1)" aria-pressed={helpOpen} onclick={ontogglehelp}>
      <CircleHelp size={16} strokeWidth={1.6} />
    </button>{/if}
    <button class="icon-btn titlebar-settings" type="button" aria-label={m.nav_settings()} aria-pressed={settingsOpen} onclick={ontogglesettings}>
      <Settings size={16} strokeWidth={1.6} />
    </button>
    <WindowControls />
  </header>

  {@render sidebar()}

  <main class="sheet">
    {@render children()}
  </main>

  <footer class="statusbar">
    <span class="status-main" data-tone={statusTone} data-live={statusLive ? '' : undefined}>{#if statusTone}<span class="conn-state {statusTone === 'reconnecting' ? 'busy' : 'down'}" aria-hidden="true"></span>{/if}{status}</span>
    <span class="spacer"></span>
    {#if selectionStats.stats || selectionStats.busy}
      <span class="status-agg" class:busy={selectionStats.busy} role="status" aria-label={m.agg_label()}>
        {#if selectionStats.stats}
          {#each statParts(selectionStats.stats) as p (p.key)}<button type="button" class="agg" data-agg={p.key} data-tip={m.agg_copy({ label: p.label })} onclick={() => void copyText(p.raw)}><b>{p.label}</b>{p.value}</button>{/each}
        {:else}{m.agg_busy()}{/if}
        {#if selectionStats.all}<span class="agg-scope" data-scope="all">{m.agg_all()}</span>
        {:else if selectionStats.wider}<span class="agg-scope" data-scope="loaded" data-tip={selectionStats.error ? m.agg_widen_failed({ message: selectionStats.error }) : undefined}>{m.agg_partial({ count: num(selectionStats.wider.loaded) })}</span><button type="button" class="agg-widen" disabled={selectionStats.busy} data-tip={m.agg_widen_tip()} onclick={() => void widen()}>{m.agg_widen()}</button>{/if}
      </span>
    {/if}
    <span>{statusRight}</span>
    <span>NiceDB 0.1</span>
  </footer>
  <span class="sr-only" role="status">{statusTone ? status : ''}</span>
</div>
