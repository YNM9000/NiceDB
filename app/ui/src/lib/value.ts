import { m } from '../paraglide/messages';

export type ValueKind = 'null' | 'json' | 'xml' | 'image' | 'binary' | 'text';

export interface Magic {
  mime: string;
  ext: string;
  label: string;
  image: boolean;
}

const BINARY = /^(bytea|(tiny|medium|long)?blob|(var)?binary|image|bytes)\b/i;
const HEX = /^(\\x|0[xX])[0-9a-fA-F]*$/;
const JSON_TYPE = /^jsonb?\b/i;
const XML_TYPE = /^xml\b/i;
const JSON_PARSE_LIMIT = 8 * 1024 * 1024;

export const isBinaryColumn = (type: string | null) => !!type && BINARY.test(type.trim());

export const isHexValue = (v: string) => HEX.test(v) && v.length % 2 === 0;

export const hexBody = (v: string) => v.slice(2);

export const byteLength = (v: string) => (v.length - 2) >> 1;

const MAGIC: [RegExp, Magic][] = [
  [/^89504e47/, { mime: 'image/png', ext: 'png', label: 'PNG', image: true }],
  [/^ffd8ff/, { mime: 'image/jpeg', ext: 'jpg', label: 'JPEG', image: true }],
  [/^47494638(37|39)61/, { mime: 'image/gif', ext: 'gif', label: 'GIF', image: true }],
  [/^52494646.{8}57454250/, { mime: 'image/webp', ext: 'webp', label: 'WebP', image: true }],
  [/^424d/, { mime: 'image/bmp', ext: 'bmp', label: 'BMP', image: true }],
  [/^00000100/, { mime: 'image/x-icon', ext: 'ico', label: 'ICO', image: true }],
  [/^25504446/, { mime: 'application/pdf', ext: 'pdf', label: 'PDF', image: false }],
  [/^504b0304/, { mime: 'application/zip', ext: 'zip', label: 'ZIP', image: false }],
  [/^1f8b/, { mime: 'application/gzip', ext: 'gz', label: 'GZIP', image: false }],
  [/^(49492a00|4d4d002a)/, { mime: 'image/tiff', ext: 'tif', label: 'TIFF', image: true }],
];

const SVG: Magic = { mime: 'image/svg+xml', ext: 'svg', label: 'SVG', image: true };
const SVG_HEAD = /^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*(<!DOCTYPE\s+svg[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*<svg[\s>]/i;

export const isSvgText = (v: string) => SVG_HEAD.test(v.slice(0, 4096));

function svgBytes(v: string): boolean {
  const head = v.slice(2, 2 + 1024).toLowerCase();
  if (!/^(3c|efbbbf3c|20|09|0a|0d)/.test(head)) return false;
  return isSvgText(new TextDecoder().decode(bytesOf(v, 512)).replace(/^\uFEFF/, ''));
}

const UNKNOWN: Magic = { mime: 'application/octet-stream', ext: 'bin', label: '', image: false };

export function magicOf(v: string): Magic {
  const head = v.slice(2, 34).toLowerCase();
  return MAGIC.find(([re]) => re.test(head))?.[1] ?? (svgBytes(v) ? SVG : UNKNOWN);
}

function looksJson(v: string): boolean {
  const t = v.trimStart();
  return t.startsWith('{') || t.startsWith('[');
}

export function parseJson(v: string): { ok: true; value: unknown } | { ok: false; error: string } {
  try {
    return { ok: true, value: JSON.parse(v) as unknown };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

function looksXml(v: string): boolean {
  const t = v.trim();
  return t.startsWith('<') && t.endsWith('>') && /<\/|\/>/.test(t) && !t.startsWith('<!DOCTYPE html') && !/^<html\b/i.test(t);
}

export function kindOf(value: string | null, type: string | null): ValueKind {
  if (value === null) return 'null';
  const binary = isBinaryColumn(type);
  if (isHexValue(value) && value.length > 2 && (binary || (!type && value.startsWith('\\x')))) return magicOf(value).image ? 'image' : 'binary';
  if (binary) return 'text';
  if (type && JSON_TYPE.test(type)) return 'json';
  if (type && XML_TYPE.test(type)) return 'xml';
  if (looksJson(value) && value.length <= JSON_PARSE_LIMIT && parseJson(value).ok) return 'json';
  if (looksXml(value)) return 'xml';
  return 'text';
}

export function bytesOf(v: string, limit = Number.POSITIVE_INFINITY): Uint8Array {
  const n = Math.min(byteLength(v), limit);
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = Number.parseInt(v.slice(2 + i * 2, 4 + i * 2), 16);
  return out;
}

export function hexLines(v: string, width = 16): string {
  const body = hexBody(v).toLowerCase();
  const out: string[] = [];
  for (let i = 0; i < body.length; i += width * 2) out.push(body.slice(i, i + width * 2).replace(/(..)(?!$)/g, '$1 '));
  return out.join('\n');
}

export function parseHexInput(text: string): string | null {
  const body = text.replace(/^\s*(\\x|0x)/i, '').replace(/\s+/g, '');
  return /^[0-9a-fA-F]*$/.test(body) && body.length % 2 === 0 ? `\\x${body.toLowerCase()}` : null;
}

export function hexOfBytes(bytes: Uint8Array): string {
  let s = '\\x';
  for (const b of bytes) s += b.toString(16).padStart(2, '0');
  return s;
}

export interface DumpLine {
  offset: string;
  hex: string;
  ascii: string;
}

export function hexDump(v: string, maxBytes = 256, width = 16): DumpLine[] {
  const bytes = bytesOf(v, maxBytes);
  const out: DumpLine[] = [];
  for (let i = 0; i < bytes.length; i += width) {
    const row = bytes.subarray(i, i + width);
    out.push({
      offset: i.toString(16).padStart(8, '0'),
      hex: Array.from(row, (b) => b.toString(16).padStart(2, '0')).join(' '),
      ascii: Array.from(row, (b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '·')).join(''),
    });
  }
  return out;
}

export function sizeText(bytes: number, locale: string): string {
  const f = (n: number, d: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: d }).format(n);
  if (bytes < 1024) return `${f(bytes, 0)} B`;
  if (bytes < 1024 * 1024) return `${f(bytes / 1024, 1)} KB`;
  return `${f(bytes / 1024 / 1024, 1)} MB`;
}

const XML_TOKEN = /<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<\?[\s\S]*?\?>|<![^>]*>|<\/?[^>]+>|[^<]+/g;

type Tok = { kind: 'open' | 'close' | 'single' | 'text'; text: string };

function tokens(src: string): Tok[] {
  const out: Tok[] = [];
  for (const [raw] of src.matchAll(XML_TOKEN)) {
    if (!raw.startsWith('<')) {
      const text = raw.trim();
      if (text) out.push({ kind: 'text', text });
    } else if (raw.startsWith('</')) out.push({ kind: 'close', text: raw });
    else if (raw.startsWith('<!') || raw.startsWith('<?') || raw.endsWith('/>')) out.push({ kind: 'single', text: raw });
    else out.push({ kind: 'open', text: raw });
  }
  return out;
}

export function formatXml(src: string, indent = '  '): string {
  const toks = tokens(src);
  const lines: string[] = [];
  let depth = 0;
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i]!;
    const pad = indent.repeat(Math.max(0, depth));
    if (t.kind === 'open') {
      const a = toks[i + 1];
      const b = toks[i + 2];
      if (a?.kind === 'close') {
        lines.push(pad + t.text + a.text);
        i += 1;
      } else if (a?.kind === 'text' && b?.kind === 'close') {
        lines.push(pad + t.text + a.text + b.text);
        i += 2;
      } else {
        lines.push(pad + t.text);
        depth++;
      }
    } else if (t.kind === 'close') {
      depth--;
      if (depth < 0) return src;
      lines.push(indent.repeat(depth) + t.text);
    } else lines.push(pad + t.text);
  }
  return depth === 0 ? lines.join('\n') : src;
}

export type XmlPart = { cls: 'tag' | 'attr' | 'str' | 'text' | 'note'; text: string };

export function xmlParts(line: string): XmlPart[] {
  const out: XmlPart[] = [];
  for (const [raw] of line.matchAll(XML_TOKEN)) {
    if (!raw.startsWith('<')) out.push({ cls: 'text', text: raw });
    else if (raw.startsWith('<!') || raw.startsWith('<?')) out.push({ cls: 'note', text: raw });
    else
      for (const [piece, str, attr] of raw.matchAll(/("[^"]*"|'[^']*')|(\s[\w:.-]+(?==))|[^"'\s]+|\s+/g))
        out.push({ cls: str ? 'str' : attr ? 'attr' : piece.trim() === '=' ? 'text' : piece.trim() ? 'tag' : 'text', text: piece });
  }
  return out;
}

export function fileName(column: string, row: string, kind: ValueKind, magic: Magic): string {
  const base = `${column || m.file_value()}-${row}`.replace(/[^\w.-]+/g, '_');
  const ext = kind === 'json' ? 'json' : kind === 'xml' ? 'xml' : kind === 'text' ? 'txt' : magic.ext;
  return `${base}.${ext}`;
}
