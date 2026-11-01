<script lang="ts">
  import { m } from '../paraglide/messages';
  import { ICON } from '../lib/icons';
  import { closeWindow, frame, minimizeWindow, toggleMaximizeWindow } from '../lib/window-frame.svelte';

  const Restore = ICON.windowRestore;
  const Maximize = ICON.windowMaximize;
  const Minimize = ICON.windowMinimize;
  const Close = ICON.windowClose;
</script>

{#if frame.kind === 'linux' || frame.kind === 'windows'}
  <span class="titlebar-sep win-sep" aria-hidden="true"></span>
  <div class="win-ctl" role="group" aria-label={m.window_controls()}>
    <button class="win-btn win-min" type="button" aria-label={m.window_minimize()} data-tip={m.window_minimize()} onclick={() => void minimizeWindow()}>
      <Minimize size={14} strokeWidth={1.5} />
    </button>
    <button class="win-btn win-max" type="button" data-restore={frame.maximized ? '' : undefined} aria-label={frame.maximized ? m.window_restore() : m.window_maximize()} data-tip={frame.maximized ? m.window_restore() : m.window_maximize()} onclick={() => void toggleMaximizeWindow()}>
      {#if frame.maximized}<Restore size={13} strokeWidth={1.5} />{:else}<Maximize size={12} strokeWidth={1.6} />{/if}
    </button>
    <button class="win-btn win-close" type="button" aria-label={m.window_close()} data-tip={m.window_close()} onclick={() => void closeWindow()}>
      <Close size={15} strokeWidth={1.5} />
    </button>
  </div>
{/if}
