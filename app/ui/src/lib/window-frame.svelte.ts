import { inTauri } from './api';
import { detectFrame } from './frame';

export const frame = $state({
  kind: typeof navigator === 'undefined' ? null : detectFrame(navigator.userAgent, inTauri, location.search),
  maximized: false,
  fullscreen: false,
  focused: true,
});

const win = () => import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow());

function mirror() {
  const html = document.documentElement;
  if (frame.kind) html.dataset.frame = frame.kind;
  else delete html.dataset.frame;
  html.toggleAttribute('data-maximized', frame.maximized);
  html.toggleAttribute('data-fullscreen', frame.fullscreen);
  html.toggleAttribute('data-window-blur', !frame.focused);
}

export function installFrame() {
  if (!frame.kind) return;
  const focus = (e: FocusEvent) => {
    frame.focused = e.type === 'focus';
    mirror();
  };
  addEventListener('focus', focus);
  addEventListener('blur', focus);
  mirror();
  if (!inTauri) return;
  void win().then(async (w) => {
    const sync = async () => {
      frame.maximized = await w.isMaximized();
      frame.fullscreen = frame.kind === 'macos' && (await w.isFullscreen());
      mirror();
    };
    await sync();
    await w.onResized(() => void sync());
  });
}

export async function minimizeWindow() {
  if (inTauri) await (await win()).minimize();
}

export async function toggleMaximizeWindow() {
  if (!inTauri) {
    frame.maximized = !frame.maximized;
    return mirror();
  }
  await (await win()).toggleMaximize();
}

export async function closeWindow() {
  if (inTauri) await (await win()).close();
}
