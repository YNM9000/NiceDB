import { appearance, resolvedTheme } from './appearance.svelte';
import { keys } from './contextmenu.svelte';
import { designOf } from './design.svelte';
import { contextSlug, isSlug, renderPage, SLUGS, type Lang, type Page, type Slug } from './manual';
import { palette } from './palette.svelte';
import { paramsOf } from './params.svelte';
import { currentBindings } from './prefs.svelte';
import { matches, shown } from './shortcuts';
import { valuePane } from './valuepanel.svelte';
import { activeTab, sessionOf } from './workspace.svelte';

const SOURCES = import.meta.glob<string>('../manual/*/*.md', { query: '?raw', import: 'default', eager: true });
const ANIMS = import.meta.glob<string>('../manual/anim/*.webp', { query: '?url', import: 'default', eager: true });

const asked = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('help');

export const help = $state({ open: asked !== null, slug: (asked && isSlug(asked) ? asked : 'start') as Slug, anchor: null as string | null, query: '', seq: 0 });

const langOf = (): Lang => (appearance.value.locale === 'it' ? 'it' : 'en');

const cache = new Map<string, Page[]>();

export function manualPages(): Page[] {
  const lang = langOf();
  const b = currentBindings();
  const key = `${lang}:${JSON.stringify(b)}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const binding = (id: string) => {
    const combo = id in b ? b[id as keyof typeof b] : '';
    return combo ? shown(combo) : null;
  };
  const hooks = { keys, binding };
  const pages = SLUGS.map((slug) => renderPage(slug, SOURCES[`../manual/${lang}/${slug}.md`] ?? `# ${slug}`, hooks));
  cache.clear();
  cache.set(key, pages);
  return pages;
}

export function animUrl(name: string, still: boolean): string | null {
  const a = appearance.value;
  const variant = a.style === 'registro' || a.style === 'listino' ? a.style : resolvedTheme(a, appearance.systemDark) === 'dark' ? 'listino' : 'registro';
  return ANIMS[`../manual/anim/${name}-${variant}-${langOf()}${still ? '-still' : ''}.webp`] ?? null;
}

export function openHelp(slug: Slug = help.slug, anchor: string | null = null) {
  help.slug = slug;
  help.anchor = anchor;
  help.open = true;
  help.seq++;
}

export const closeHelp = () => (help.open = false);

export function currentContext(settings: boolean): Slug {
  const tab = activeTab();
  const caps = tab ? sessionOf(tab)?.info.caps : null;
  const language = caps?.language ?? null;
  return contextSlug({
    settings,
    palette: palette.open,
    tab: tab && {
      view: tab.view,
      language: language?.kind ?? null,
      dialect: language?.kind === 'sql' ? language.dialect : null,
      design: !!designOf(tab),
      dry: tab.dry,
      params: paramsOf(tab.id).slots.length > 0,
      valuePanel: valuePane.data,
    },
  });
}

export function helpKeys(here: () => void, allowed: () => boolean, target: Window = window) {
  const onkey = (e: KeyboardEvent) => {
    if (!matches(e, 'F1') || !allowed()) return;
    e.preventDefault();
    if (help.open) closeHelp();
    else here();
  };
  target.addEventListener('keydown', onkey);
  return () => target.removeEventListener('keydown', onkey);
}
