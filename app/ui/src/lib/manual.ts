export const GROUPS = [
  { id: 'basics', pages: ['start', 'connections', 'workspaces', 'notes'] },
  { id: 'query', pages: ['editor', 'results', 'parameters', 'transactions'] },
  { id: 'data', pages: ['data', 'fk', 'value', 'import-export'] },
  { id: 'schema', pages: ['explorer', 'ddl', 'designer', 'er-plans'] },
  { id: 'server', pages: ['tools', 'production'] },
  { id: 'families', pages: ['compatible', 'oracle', 'db2', 'hana', 'firebird', 'odbc', 'mongodb', 'redis', 'clickhouse', 'cassandra', 'dynamodb', 'elasticsearch'] },
  { id: 'app', pages: ['palette', 'settings', 'troubleshooting'] },
] as const;

export type GroupId = (typeof GROUPS)[number]['id'];
export type Slug = (typeof GROUPS)[number]['pages'][number];
export type Lang = 'it' | 'en';

export const SLUGS: readonly Slug[] = GROUPS.flatMap((g) => g.pages);

export const isSlug = (s: string): s is Slug => (SLUGS as readonly string[]).includes(s);

export interface Heading {
  id: string;
  level: number;
  text: string;
}

export interface Page {
  slug: Slug;
  title: string;
  html: string;
  headings: Heading[];
  text: string;
}

export interface RenderHooks {
  keys: (combo: string) => string;
  binding: (action: string) => string | null;
}

const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');

export const anchorOf = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

const plainOf = (s: string) =>
  s
    .replace(/^```.*$/gm, '')
    .replace(/^\s*\|?[\s:|-]+\|?\s*$/gm, '')
    .replace(/^#{1,4}\s+(.*)$/gm, '$1 ·')
    .replace(/^\s*(>|[-*+]|\d+[.)])\s+/gm, '')
    .replace(/^\s*\|(.*)\|\s*$/gm, '$1')
    .replace(/\s*\|\s*/g, ' · ')
    .replace(/\[\[key:(\w+)\]\]/g, '$1')
    .replace(/\[\[([^\]]+)\]\]/g, '$1')
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/[*`]/g, '');

function keyCombo(raw: string, hooks: RenderHooks): string {
  if (/^Ctrl\+(Space|Spazio)$/.test(raw)) return raw;
  if (raw.length > 1 && raw.endsWith('+')) return raw;
  return hooks.keys(raw.split('+').map((k) => (k === 'Ctrl' ? 'Mod' : k)).join('+'));
}

function inline(raw: string, hooks: RenderHooks): string {
  const held: string[] = [];
  const hold = (html: string) => `\u0000${held.push(html) - 1}\u0000`;
  let s = raw.replace(/`([^`\n]+)`/g, (_, c: string) => hold(`<code>${escape(c)}</code>`));
  s = s.replace(/\[\[key:(\w+)\]\]/g, (_, id: string) => {
    const combo = hooks.binding(id);
    return hold(combo ? `<kbd>${escape(hooks.keys(combo))}</kbd>` : `<kbd class="unbound">—</kbd>`);
  });
  s = s.replace(/\[\[([^\]\n]+)\]\]/g, (_, k: string) => hold(`<kbd>${escape(keyCombo(k, hooks))}</kbd>`));
  s = s.replace(/\[([^\]\n]+)\]\(([a-z-]+)\)/g, (whole, text: string, slug: string) => (isSlug(slug) ? hold(`<a href="#${slug}" data-page="${slug}">${escape(text)}</a>`) : whole));
  s = escape(s)
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, '$1<em>$2</em>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i: string) => held[Number(i)] ?? '');
}

const cells = (line: string) =>
  line
    .trim()
    .replace(/^\||\|$/g, '')
    .split('|')
    .map((c) => c.trim());

export function renderPage(slug: Slug, src: string, hooks: RenderHooks): Page {
  const lines = src.replace(/\r\n?/g, '\n').split('\n');
  const out: string[] = [];
  const headings: Heading[] = [];
  const used = new Set<string>();
  let title: string = slug;
  let list: { kind: 'ul' | 'ol'; items: string[] } | null = null;
  let para: string[] = [];
  let quote: string[] = [];
  const flush = () => {
    if (para.length) out.push(`<p>${para.map((l) => inline(l, hooks)).join(' ')}</p>`);
    if (quote.length) out.push(`<blockquote>${quote.map((l) => inline(l, hooks)).join(' ')}</blockquote>`);
    if (list) out.push(`<${list.kind}>${list.items.map((i) => `<li>${inline(i, hooks)}</li>`).join('')}</${list.kind}>`);
    para = [];
    quote = [];
    list = null;
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? '';
    const fence = /^```\s*(\w*)\s*$/.exec(line);
    if (fence) {
      flush();
      const code: string[] = [];
      for (i++; i < lines.length && !/^```\s*$/.test(lines[i] ?? ''); i++) code.push(lines[i] ?? '');
      out.push(`<pre><code>${escape(code.join('\n'))}</code></pre>`);
      continue;
    }
    if (/^\s*\|/.test(line) && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1] ?? '')) {
      flush();
      const head = cells(line);
      const rows: string[][] = [];
      for (i += 2; i < lines.length && /^\s*\|/.test(lines[i] ?? ''); i++) rows.push(cells(lines[i] ?? ''));
      i--;
      out.push(
        `<div class="man-table"><table><thead><tr>${head.map((c) => `<th>${inline(c, hooks)}</th>`).join('')}</tr></thead><tbody>${rows
          .map((r) => `<tr>${head.map((_, j) => `<td>${inline(r[j] ?? '', hooks)}</td>`).join('')}</tr>`)
          .join('')}</tbody></table></div>`,
      );
      continue;
    }
    const anim = /^!\[([^\]]*)\]\(anim:([a-z-]+)\)\s*$/.exec(line);
    const head = /^(#{1,4})\s+(.*)$/.exec(line);
    const bullet = /^\s*[-*+]\s+(.*)$/.exec(line);
    const number = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    const q = /^>\s?(.*)$/.exec(line);
    if (anim) {
      flush();
      out.push(`<figure class="man-anim" data-anim="${escape(anim[2] ?? '')}"><figcaption>${escape(anim[1] ?? '')}</figcaption></figure>`);
    } else if (head) {
      flush();
      const level = (head[1] ?? '#').length;
      const text = head[2] ?? '';
      if (level === 1) {
        title = plainOf(text);
        continue;
      }
      let id = anchorOf(plainOf(text)) || 'h';
      while (used.has(id)) id += '-';
      used.add(id);
      headings.push({ id, level, text: plainOf(text) });
      out.push(`<h${level} id="${id}">${inline(text, hooks)}</h${level}>`);
    } else if (bullet || number) {
      const kind = bullet ? 'ul' : 'ol';
      if (para.length || quote.length || (list && list.kind !== kind)) flush();
      list ??= { kind, items: [] };
      list.items.push((bullet ?? number)?.[1] ?? '');
    } else if (q) {
      if (para.length || list) flush();
      quote.push(q[1] ?? '');
    } else if (line.trim() === '') flush();
    else if (list && /^\s{2,}\S/.test(line)) {
      const items: string[] = list.items;
      items[items.length - 1] += ` ${line.trim()}`;
    } else {
      if (list || quote.length) flush();
      para.push(line);
    }
  }
  flush();
  return { slug, title, html: out.join('\n'), headings, text: plainOf(src.replace(/^#\s.*\n/, '')).replace(/\s+/g, ' ').trim() };
}

export interface Hit {
  slug: Slug;
  title: string;
  heading: Heading | null;
  snippet: string;
  score: number;
}

const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

export function stem(word: string): string {
  let w = word;
  if (w.length > 4 && w.endsWith('ies')) w = `${w.slice(0, -3)}y`;
  else if (w.length > 4 && /(ch|sh|x|ss)es$/.test(w)) w = w.slice(0, -2);
  else if (w.length > 3 && w.endsWith('s') && !w.endsWith('ss')) w = w.slice(0, -1);
  if (w.length > 5 && w.endsWith('ing')) w = w.slice(0, -3);
  else if (w.length > 4 && w.endsWith('ed')) w = w.slice(0, -2);
  if (w.length > 3 && /[aeiouy]$/.test(w)) w = w.slice(0, -1);
  if (w.length > 3 && /[cg]h$/.test(w)) w = w.slice(0, -1);
  return w;
}

const stemmed = new WeakMap<Page, string>();
function stemsOf(p: Page): string {
  let s = stemmed.get(p);
  if (s === undefined) {
    s = ` ${fold(`${p.title} ${p.text}`)
      .split(/[^\p{L}\p{N}_]+/u)
      .filter(Boolean)
      .map(stem)
      .join(' ')}`;
    stemmed.set(p, s);
  }
  return s;
}

const holds = (text: string, stems: string, w: string) => text.includes(w) || stems.includes(` ${stem(w)}`);

export function searchPages(pages: readonly Page[], query: string, limit = 30): Hit[] {
  const words = fold(query).split(/\s+/).filter((w) => w.length > 1);
  if (!words.length) return [];
  const hits: Hit[] = [];
  for (const p of pages) {
    const body = fold(`${p.title} ${p.text}`);
    const stems = stemsOf(p);
    if (!words.every((w) => holds(body, stems, w))) continue;
    const title = fold(p.title);
    const titleStems = ` ${title.split(/[^\p{L}\p{N}_]+/u).map(stem).join(' ')}`;
    let score = words.filter((w) => holds(title, titleStems, w)).length * 10;
    const matched = (h: Heading) => {
      const text = fold(h.text);
      const hs = ` ${text.split(/[^\p{L}\p{N}_]+/u).map(stem).join(' ')}`;
      return words.filter((w) => holds(text, hs, w)).length;
    };
    const heading = p.headings.reduce<Heading | null>((best, h) => (matched(h) > (best ? matched(best) : 0) ? h : best), null);
    if (heading) score += 4;
    const phrase = fold(query).trim().replace(/\s+/g, ' ');
    const text = fold(p.text);
    if (words.length > 1 && body.includes(phrase)) score += 5;
    const first = words[0] ?? '';
    const at = text.includes(phrase) ? text.indexOf(phrase) : text.includes(first) ? text.indexOf(first) : text.indexOf(stem(first));
    const start = Math.max(0, at - 40);
    const raw = p.text.slice(start, start + 140).trim();
    score += at < 0 ? 0 : 1 / (1 + at / 1000);
    hits.push({ slug: p.slug, title: p.title, heading, snippet: (start > 0 ? '…' : '') + raw + '…', score });
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, limit);
}

export interface Context {
  settings: boolean;
  palette: boolean;
  tab: null | {
    view: 'query' | 'data' | 'schema' | 'tools';
    language: string | null;
    dialect: string | null;
    design: boolean;
    dry: boolean;
    params: boolean;
    valuePanel: boolean;
  };
}

export function contextSlug(c: Context): Slug {
  if (c.palette) return 'palette';
  if (c.settings) return 'settings';
  const t = c.tab;
  if (!t) return 'start';
  if (t.design) return 'designer';
  if (t.language === 'mql') return 'mongodb';
  if (t.language === 'redis') return 'redis';
  if (t.language === 'cql') return 'cassandra';
  if (t.language === 'partiql') return 'dynamodb';
  if (t.language === 'search') return 'elasticsearch';
  if (t.view === 'tools') return 'tools';
  if (t.view === 'schema') return 'er-plans';
  if (t.view === 'data') return t.valuePanel ? 'value' : 'data';
  if (t.dialect === 'clickhouse') return 'clickhouse';
  if (t.dialect === 'oracle') return 'oracle';
  if (t.dialect === 'db2') return 'db2';
  if (t.dialect === 'hana') return 'hana';
  if (t.dialect === 'firebird') return 'firebird';
  if (t.dialect === 'ansi' || t.dialect === 'ansi_backtick') return 'odbc';
  if (t.dry) return 'transactions';
  if (t.params) return 'parameters';
  return 'editor';
}
