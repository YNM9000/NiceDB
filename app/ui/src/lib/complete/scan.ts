import type { Dialect } from '../bindings';

export type TokenKind = 'word' | 'qident' | 'string' | 'number' | 'punct' | 'op' | 'param' | 'comment';

export interface Token {
  k: TokenKind;
  from: number;
  to: number;
  text: string;
  open?: boolean;
}

function isWordStart(c: string): boolean {
  const n = c.charCodeAt(0);
  if (n < 128) return (n >= 97 && n <= 122) || (n >= 65 && n <= 90) || n === 95;
  return /\p{L}/u.test(c);
}

function isWordChar(c: string): boolean {
  const n = c.charCodeAt(0);
  if (n < 128) return (n >= 97 && n <= 122) || (n >= 65 && n <= 90) || (n >= 48 && n <= 57) || n === 95 || n === 36;
  return /[\p{L}\p{N}]/u.test(c);
}
const isDigit = (c: string) => c >= '0' && c <= '9';

function closeQuote(s: string, i: number, q: string, backslash: boolean): [number, boolean] {
  while (i < s.length) {
    const c = s[i];
    if (backslash && c === '\\') {
      i += 2;
      continue;
    }
    if (c === q) {
      if (s[i + 1] === q) {
        i += 2;
        continue;
      }
      return [i + 1, false];
    }
    i++;
  }
  return [s.length, true];
}

function unquote(raw: string, q: string): string {
  const close = q === '[' ? ']' : q;
  const inner = raw.slice(1, raw.endsWith(close) && raw.length > 1 ? -1 : undefined);
  return inner.split(close + close).join(close);
}

export function scan(s: string, dialect: Dialect): Token[] {
  const out: Token[] = [];
  const dollar = dialect === 'postgres' || dialect === 'duckdb';
  const nested = dialect !== 'mysql' && dialect !== 'sqlite';
  const mysql = dialect === 'mysql';
  let delim: string | null = null;
  const atDelim = (at: number) => delim !== null && s.startsWith(delim, at);
  let i = 0;
  while (i < s.length) {
    const c = s[i] ?? '';
    const n = s[i + 1] ?? '';
    const from = i;
    if (c === ' ' || c === '\t' || c === '\n' || c === '\r') {
      i++;
      continue;
    }
    if (mysql && (c === 'd' || c === 'D') && statementStart(out)) {
      const line = delimiterLine(s, i);
      if (line) {
        out.push({ k: 'punct', from, to: from, text: ';' }, { k: 'comment', from, to: line.end, text: '' });
        delim = line.delimiter === ';' ? null : line.delimiter;
        i = line.end;
        continue;
      }
    }
    if (atDelim(i)) {
      i += delim?.length ?? 1;
      out.push({ k: 'punct', from, to: i, text: ';' });
      continue;
    }
    if ((c === '-' && n === '-') || (c === '#' && dialect === 'mysql')) {
      const end = s.indexOf('\n', i);
      i = end < 0 ? s.length : end;
      out.push({ k: 'comment', from, to: i, text: '', open: true });
      continue;
    }
    if (c === '/' && n === '*') {
      let depth = 1;
      i += 2;
      while (i < s.length && depth > 0) {
        if (s[i] === '*' && s[i + 1] === '/') {
          depth--;
          i += 2;
        } else if (nested && s[i] === '/' && s[i + 1] === '*') {
          depth++;
          i += 2;
        } else i++;
      }
      out.push({ k: 'comment', from, to: i, text: '', open: depth > 0 });
      continue;
    }
    if (c === "'") {
      const [end, open] = closeQuote(s, i + 1, "'", dialect === 'mysql');
      i = end;
      out.push({ k: 'string', from, to: i, text: '', open });
      continue;
    }
    if (c === '"' || c === '`' || (c === '[' && (dialect === 'mssql' || dialect === 'sqlite'))) {
      const stringy = c === '"' && dialect === 'mysql';
      const backtick = c === '`' && dialect !== 'mysql' && dialect !== 'sqlite';
      if (!backtick) {
        const [end, open] = c === '[' ? closeQuote(s, i + 1, ']', false) : closeQuote(s, i + 1, c, stringy);
        i = end;
        const raw = s.slice(from, i);
        out.push(stringy ? { k: 'string', from, to: i, text: '', open } : { k: 'qident', from, to: i, text: unquote(raw, c), open });
        continue;
      }
    }
    if (c === '$' && dollar) {
      const m = /^\$([A-Za-z_][\w]*)?\$/.exec(s.slice(i, i + 64));
      if (m) {
        const tag = m[0];
        const end = s.indexOf(tag, i + tag.length);
        i = end < 0 ? s.length : end + tag.length;
        out.push({ k: 'string', from, to: i, text: '', open: end < 0 });
        continue;
      }
    }
    if (/[eEnNxXbB]/.test(c) && n === "'") {
      const [end, open] = closeQuote(s, i + 2, "'", c === 'e' || c === 'E' || dialect === 'mysql');
      i = end;
      out.push({ k: 'string', from, to: i, text: '', open });
      continue;
    }
    if (isWordStart(c) || (c === '#' && dialect === 'mssql')) {
      i++;
      while (i < s.length && !atDelim(i) && (isWordChar(s[i] ?? '') || (dialect === 'mssql' && (s[i] === '#' || s[i] === '@')))) i++;
      out.push({ k: 'word', from, to: i, text: s.slice(from, i) });
      continue;
    }
    if (isDigit(c) || (c === '.' && isDigit(n))) {
      i++;
      while (i < s.length && /[\d.eE_]/.test(s[i] ?? '')) i++;
      out.push({ k: 'number', from, to: i, text: s.slice(from, i) });
      continue;
    }
    if (c === '@' || (c === ':' && isWordStart(n) && s[i - 1] !== ':') || (c === '$' && (n === '{' || isDigit(n)))) {
      i++;
      if (s[i] === '@' || s[i] === '{') i++;
      while (i < s.length && (isWordChar(s[i] ?? '') || s[i] === '}')) {
        i++;
        if (s[i - 1] === '}') break;
      }
      out.push({ k: 'param', from, to: i, text: s.slice(from, i) });
      continue;
    }
    if (c === '?') {
      i++;
      out.push({ k: 'param', from, to: i, text: '?' });
      continue;
    }
    if ('.,();'.includes(c)) {
      i++;
      out.push({ k: 'punct', from, to: i, text: c });
      continue;
    }
    i++;
    while (i < s.length && !atDelim(i) && '+-*/<>=~!|&%^:'.includes(s[i] ?? '') && !(s[i] === '-' && s[i + 1] === '-')) i++;
    out.push({ k: 'op', from, to: i, text: s.slice(from, i) });
  }
  return out;
}

function statementStart(out: Token[]): boolean {
  for (let k = out.length - 1; k >= 0; k--) {
    const t = out[k];
    if (t && t.k !== 'comment') return t.k === 'punct' && t.text === ';';
  }
  return true;
}

export function delimiterLine(s: string, i: number): { delimiter: string; end: number } | null {
  if (s.slice(s.lastIndexOf('\n', i - 1) + 1, i).trim() !== '') return null;
  const m = /^delimiter[ \t]+(\S+)[^\n]*/i.exec(s.slice(i, i + 200));
  if (!m?.[1]) return null;
  return { delimiter: m[1], end: i + m[0].length };
}

export const lower = (t: Token | undefined) => (t && t.k === 'word' ? t.text.toLowerCase() : '');
export const isName = (t: Token | undefined): t is Token & { k: 'word' | 'qident' } => !!t && (t.k === 'word' || t.k === 'qident');

export interface StatementSpan {
  from: number;
  to: number;
  tokens: Token[];
}

function isGo(s: string, t: Token): boolean {
  if (t.k !== 'word' || t.text.toLowerCase() !== 'go') return false;
  const start = s.lastIndexOf('\n', t.from - 1) + 1;
  const endNl = s.indexOf('\n', t.to);
  const end = endNl < 0 ? s.length : endNl;
  return s.slice(start, t.from).trim() === '' && /^\s*\d*\s*(--.*)?$/.test(s.slice(t.to, end));
}

export function statements(s: string, dialect: Dialect, tokens: Token[] = scan(s, dialect)): StatementSpan[] {
  const out: StatementSpan[] = [];
  let cur: Token[] = [];
  let start = 0;
  const flush = (end: number) => {
    out.push({ from: start, to: end, tokens: cur });
    cur = [];
  };
  for (const t of tokens) {
    if (t.k === 'punct' && t.text === ';') {
      flush(t.from);
      start = t.to;
      continue;
    }
    if (dialect === 'mssql' && isGo(s, t)) {
      flush(t.from);
      const nl = s.indexOf('\n', t.to);
      start = nl < 0 ? s.length : nl;
      continue;
    }
    if (t.k !== 'comment') cur.push(t);
  }
  flush(s.length);
  return out;
}

export function statementAt(list: StatementSpan[], pos: number): StatementSpan {
  for (const st of list) if (pos >= st.from && pos <= st.to) return st;
  return list.at(-1) ?? { from: 0, to: 0, tokens: [] };
}

export function insideLiteral(tokens: Token[], pos: number): Token | null {
  for (const t of tokens) {
    if (t.from >= pos) break;
    if ((t.k === 'comment' || t.k === 'string') && (pos < t.to || (t.open && pos === t.to))) return t;
  }
  return null;
}
