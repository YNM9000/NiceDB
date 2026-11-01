import { mount } from 'svelte';
import App from './App.svelte';
import { inTauri } from './lib/api';
import { loadAppearance } from './lib/appearance.svelte';
import { installKeyboardMode, mirrorInvalid } from './lib/focus';
import { loadLibrary } from './lib/library.svelte';
import { loadPrefs } from './lib/prefs.svelte';
import { mountSplash } from './lib/play-splash';
import { installFrame } from './lib/window-frame.svelte';
import './styles/index.css';

function reveal() {
  document.documentElement.dataset.ready = '';
  if (inTauri) void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().show());
}

async function boot() {
  installKeyboardMode();
  installFrame();
  try {
    await Promise.all([loadAppearance(), loadPrefs(), loadLibrary()]);
    const target = document.getElementById('app');
    if (!target) throw new Error('missing #app');
    mount(App, { target });
    mirrorInvalid(target);
    const splash = mountSplash();
    await document.fonts.ready;
    requestAnimationFrame(() => {
      reveal();
      splash?.start();
    });
  } catch (e) {
    const pre = document.createElement('pre');
    pre.className = 'boot-error';
    pre.textContent = `NiceDB could not start.\n\n${e instanceof Error ? (e.stack ?? e.message) : String(e)}`;
    document.body.replaceChildren(pre);
    reveal();
  }
}

void boot();
