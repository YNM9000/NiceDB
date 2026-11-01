import { m } from '../paraglide/messages';
import type { VarError, VarPreview } from './bindings';

export const VAR_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
const SECRET_WORDS = ['PASSWORD', 'PASSWD', 'PWD', 'SECRET', 'TOKEN', 'PASSPHRASE', 'API_KEY', 'PRIVATE_KEY'];

export const secretLike = (name: string) => SECRET_WORDS.some((w) => name.toUpperCase().includes(w));

export const hasVars = (text: string) => /(^|[^$])\$\{/.test(text);

export function expand(text: string, field: string, lookup: (name: string) => string | undefined): VarPreview {
  let out = '';
  let i = 0;
  while (i < text.length) {
    const at = text.indexOf('$', i);
    if (at < 0) {
      out += text.slice(i);
      break;
    }
    out += text.slice(i, at);
    if (text.startsWith('$${', at)) {
      out += '${';
      i = at + 3;
    } else if (text.startsWith('${', at)) {
      const end = text.indexOf('}', at + 2);
      if (end < 0) return { kind: 'problem', error: { kind: 'syntax', field } };
      const inner = text.slice(at + 2, end);
      const cut = inner.indexOf(':-');
      const name = (cut < 0 ? inner : inner.slice(0, cut)).trim();
      const fallback = cut < 0 ? undefined : inner.slice(cut + 2);
      if (!VAR_NAME.test(name)) return { kind: 'problem', error: { kind: 'syntax', field } };
      if (secretLike(name)) return { kind: 'problem', error: { kind: 'secret', name, field } };
      const v = lookup(name) ?? fallback;
      if (v === undefined) return { kind: 'problem', error: { kind: 'undefined', name, field } };
      out += v;
      i = end + 1;
    } else {
      out += '$';
      i = at + 1;
    }
  }
  return { kind: 'value', text: out };
}

export const VAR_EXAMPLE = '${DB_HOST}';

export function varMessage(e: VarError): string {
  switch (e.kind) {
    case 'undefined':
      return m.var_undefined({ name: `\${${e.name}}` });
    case 'secret':
      return m.var_secret({ name: `\${${e.name}}` });
    case 'syntax':
      return m.var_syntax({ example: VAR_EXAMPLE });
    case 'bad_number':
      return m.var_bad_number({ value: e.value });
  }
}

export const varFault = (e: VarError) => (e.field && e.field !== 'workspace' ? m.var_in_field({ message: varMessage(e), field: e.field }) : varMessage(e));
