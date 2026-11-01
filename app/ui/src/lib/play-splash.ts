import { mount, unmount } from 'svelte';
import Splash from '../components/Splash.svelte';
import { splashMode } from './splash';

let current: { holder: HTMLElement; splash: ReturnType<typeof mount> } | null = null;

export function mountSplash(): { start(): void } | null {
  const mode = splashMode(location.search, navigator.webdriver);
  if (mode === 'off') return null;
  if (current) {
    void unmount(current.splash);
    current.holder.remove();
  }
  const holder = document.body.appendChild(document.createElement('div'));
  const end = () => {
    holder.remove();
    if (current?.holder === holder) current = null;
  };
  const splash = mount(Splash, { target: holder, props: { mode, onend: end } });
  current = { holder, splash };
  return splash as unknown as { start(): void };
}

export function playSplash() {
  const splash = mountSplash();
  if (splash) requestAnimationFrame(() => splash.start());
}
