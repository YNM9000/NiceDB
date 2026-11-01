export type Rgba = [number, number, number, number];

const HEX = /^#([0-9a-f]{3,8})$/i;
const RGB = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/i;
const MIX = /^color-mix\(\s*in srgb\s*,(.*)\)$/i;

function splitTop(s: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of s) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) {
      out.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

function alpha(a: string | undefined): number {
  if (a === undefined) return 1;
  return a.endsWith('%') ? parseFloat(a) / 100 : parseFloat(a);
}

export function parseColor(css: string): Rgba | null {
  const s = css.trim();
  if (s === 'transparent') return [0, 0, 0, 0];
  const hex = HEX.exec(s)?.[1];
  if (hex) {
    const full = hex.length <= 4 ? [...hex].map((c) => c + c).join('') : hex;
    const n = (i: number) => parseInt(full.slice(i, i + 2), 16);
    if (full.length !== 6 && full.length !== 8) return null;
    return [n(0), n(2), n(4), full.length === 8 ? n(6) / 255 : 1];
  }
  const rgb = RGB.exec(s);
  if (rgb) return [+rgb[1]!, +rgb[2]!, +rgb[3]!, alpha(rgb[4])];
  const mix = MIX.exec(s)?.[1];
  if (mix) {
    const parts = splitTop(mix).map((p) => {
      const m = /^(.*?)(?:\s+([\d.]+)%)?$/.exec(p.trim());
      return { c: parseColor(m?.[1] ?? ''), p: m?.[2] === undefined ? null : parseFloat(m[2]) / 100 };
    });
    const [a, b] = parts;
    if (!a?.c || !b?.c) return null;
    const pa = a.p ?? (b.p === null ? 0.5 : 1 - b.p);
    const pb = b.p ?? 1 - pa;
    const wa = a.c[3] * pa;
    const wb = b.c[3] * pb;
    const w = wa + wb;
    if (w === 0) return [0, 0, 0, 0];
    const ch = (i: number) => (a.c![i]! * wa + b.c![i]! * wb) / w;
    return [ch(0), ch(1), ch(2), w / (pa + pb)];
  }
  return null;
}

export function over(top: Rgba, below: Rgba): Rgba {
  const a = top[3];
  return [top[0] * a + below[0] * (1 - a), top[1] * a + below[1] * (1 - a), top[2] * a + below[2] * (1 - a), 1];
}

function luminance(c: Rgba): number {
  const ch = (v: number) => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2]);
}

export function contrast(fg: Rgba, bg: Rgba): number {
  const solid = over(bg, [255, 255, 255, 1]);
  const [x, y] = [luminance(over(fg, solid)), luminance(solid)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

export function toHex(c: Rgba): string {
  return '#' + c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
}

type Rule = { attrs: Record<string, string>[]; decls: Map<string, string>; order: number };

const ATTR = /\[(data-[\w-]+)=['"]([^'"]+)['"]\]/g;

function rules(css: string, start: number): Rule[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const out: Rule[] = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  for (const m of text.matchAll(re)) {
    const attrs: Record<string, string>[] = [];
    for (const sel of m[1]!.split(',').map((x) => x.trim())) {
      if (!/^(\[data-[\w-]+=['"][^'"]+['"]\])+$/.test(sel)) continue;
      attrs.push(Object.fromEntries([...sel.matchAll(ATTR)].map((a) => [a[1]!, a[2]!])));
    }
    if (!attrs.length) continue;
    const decls = new Map<string, string>();
    for (const d of m[2]!.split(';')) {
      const i = d.indexOf(':');
      const name = d.slice(0, i).trim();
      if (i > 0 && name.startsWith('--')) decls.set(name, d.slice(i + 1).trim());
    }
    out.push({ attrs, decls, order: start + out.length });
  }
  return out;
}

export function tokenResolver(sheets: string[], root: Record<string, string>): (name: string) => string | null {
  let order = 0;
  const all = sheets.flatMap((css) => {
    const r = rules(css, order);
    order += r.length;
    return r;
  });
  const winners = new Map<string, { spec: number; order: number; value: string }>();
  for (const rule of all) {
    const specs = rule.attrs.filter((a) => Object.entries(a).every(([k, v]) => root[k] === v)).map((a) => Object.keys(a).length);
    if (!specs.length) continue;
    const spec = Math.max(...specs);
    for (const [name, value] of rule.decls) {
      const prev = winners.get(name);
      if (!prev || spec > prev.spec || (spec === prev.spec && rule.order > prev.order)) winners.set(name, { spec, order: rule.order, value });
    }
  }
  const resolve = (name: string, depth: number): string | null => {
    const raw = winners.get(name)?.value;
    if (raw === undefined || depth > 20) return null;
    let failed = false;
    const value = raw.replace(/var\((--[\w-]+)\s*(?:,\s*([^()]*))?\)/g, (_, ref: string, fallback: string | undefined) => {
      const v = resolve(ref, depth + 1) ?? fallback?.trim() ?? null;
      if (v === null) failed = true;
      return v ?? '';
    });
    return failed ? null : value;
  };
  return (name) => resolve(name, 0);
}
