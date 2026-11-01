import { ApiError, type RunEvent, type RunRequest } from './api';
import type { Dialect, ParamSlot, ParamUse, ParamValue, ParamsFound, RunScope } from './bindings';

type Run = (req: RunRequest, emit: (e: RunEvent) => void) => Promise<void>;

interface Mark {
  from: number;
  to: number;
  name: string;
  label: string;
}

function marks(sql: string, base: number, first: number, dialect: Dialect = 'postgres'): Mark[] {
  const out: Mark[] = [];
  let n = first;
  let i = 0;
  while (i < sql.length) {
    const c = sql[i] ?? '';
    const two = sql.slice(i, i + 2);
    if (two === '--') i = sql.indexOf('\n', i) < 0 ? sql.length : sql.indexOf('\n', i);
    else if (two === '/*') i = sql.indexOf('*/', i) < 0 ? sql.length : sql.indexOf('*/', i) + 2;
    else if (c === "'" || c === '"') {
      const end = sql.indexOf(c, i + 1);
      i = end < 0 ? sql.length : end + 1;
    } else if (two === '::' || two === ':=') i += 2;
    else if (c === ':' && !/[\w]/.test(sql[i - 1] ?? '') && /[A-Za-z_]/.test(sql[i + 1] ?? '')) {
      const name = /^[A-Za-z_]\w*/.exec(sql.slice(i + 1))?.[0] ?? '';
      out.push({ from: base + i, to: base + i + 1 + name.length, name, label: `:${name}` });
      i += 1 + name.length;
    } else if (two === '${') {
      const m = /^\$\{([A-Za-z_]\w*)\}/.exec(sql.slice(i));
      if (m?.[1]) {
        out.push({ from: base + i, to: base + i + m[0].length, name: m[1], label: m[0] });
        i += m[0].length;
      } else i++;
    } else if (/[@$]/.test(c) && !/[\w$@]/.test(sql[i - 1] ?? '') && engineForm(dialect, c, sql.slice(i + 1))) {
      const name = engineForm(dialect, c, sql.slice(i + 1));
      out.push({ from: base + i, to: base + i + 1 + name.length, name, label: `${c}${name}` });
      i += 1 + name.length;
    } else if (c === '?' && !/[|&]/.test(sql[i + 1] ?? '')) {
      n++;
      out.push({ from: base + i, to: base + i + 1, name: String(n), label: '?' });
      i++;
    } else i++;
  }
  return out;
}

function engineForm(dialect: Dialect, c: string, rest: string): string {
  if (c === '$' && (dialect === 'postgres' || dialect === 'duckdb')) return /^\d+/.exec(rest)?.[0] ?? '';
  if ((c === '@' && (dialect === 'mssql' || dialect === 'sqlite')) || (c === '$' && dialect === 'sqlite')) return /^[A-Za-z_]\w*/.exec(rest)?.[0] ?? '';
  return '';
}

function statements(text: string): { from: number; sql: string }[] {
  const out: { from: number; sql: string }[] = [];
  let start = 0;
  for (const part of text.split(';')) {
    const lead = part.length - part.trimStart().length;
    if (part.trim()) out.push({ from: start + lead, sql: part.trim() });
    start += part.length + 1;
  }
  return out;
}

function inScope(text: string, scope: RunScope) {
  const all = statements(text);
  if (scope.kind === 'script') return all;
  if (scope.kind === 'selection') {
    const [a, b] = [Math.min(scope.from, scope.to), Math.max(scope.from, scope.to)];
    return statements(text.slice(a, b)).map((s) => ({ ...s, from: s.from + a }));
  }
  const at = all.filter((s) => s.from <= scope.cursor).at(-1) ?? all[0];
  return at ? [at] : [];
}

function found(text: string, scope: RunScope, dialect: Dialect = 'postgres'): Mark[] {
  let positional = 0;
  return inScope(text, scope).flatMap((s) => {
    const list = marks(s.sql, s.from, positional, dialect);
    positional += list.filter((m) => m.label === '?').length;
    return list;
  });
}

const keyOf = (text: string, scope: RunScope) =>
  inScope(text, scope)
    .map((s) => s.sql.replace(/\s+/g, ' '))
    .join(';');

const NAME = /^[A-Za-z_][\w$]*(\.[A-Za-z_][\w$]*)*$/;

function literal(v: ParamValue): string {
  const raw = v.value.trim();
  switch (v.kind) {
    case 'null':
      return 'NULL';
    case 'boolean':
      return raw === 'true' ? 'TRUE' : 'FALSE';
    case 'number':
      if (!/^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(raw)) throw new ApiError({ code: 'param', detail: { kind: 'invalid', name: v.name, as_kind: v.kind } });
      return raw.startsWith('-') ? `(${raw})` : raw;
    case 'raw':
      if (!NAME.test(raw)) throw new ApiError({ code: 'param', detail: { kind: 'invalid', name: v.name, as_kind: v.kind } });
      return raw;
    default:
      return `'${v.value.replaceAll("'", "''")}'`;
  }
}

const DEMO: [string, ParamUse[]][] = [
  [
    'SELECT numero, cliente_id, totale, scadenza FROM fatture WHERE cliente_id = :cliente AND scadenza >= :dal AND pagata = :pagata ORDER BY scadenza',
    [
      { name: 'cliente', kind: 'number', value: '1042', used_at: 3, here: true },
      { name: 'cliente', kind: 'number', value: '877', used_at: 2, here: true },
      { name: 'dal', kind: 'date', value: '2026-01-01', used_at: 3, here: true },
      { name: 'pagata', kind: 'boolean', value: 'false', used_at: 3, here: true },
    ],
  ],
];

export function paramsMock(seed: boolean) {
  const history = new Map<string, ParamUse[]>(seed ? DEMO : []);
  let clock = 10;

  function recall(key: string, names: string[]): ParamUse[] {
    const here = (history.get(key) ?? []).filter((h) => names.includes(h.name));
    const elsewhere = [...history.entries()].filter(([k]) => k !== key).flatMap(([, list]) => list.filter((h) => names.includes(h.name)).map((h) => ({ ...h, here: false })));
    return [...here, ...elsewhere];
  }

  return {
    paramsFound: async (text: string, dialect: Dialect, scope: RunScope): Promise<ParamsFound> => {
      const slots: ParamSlot[] = [];
      for (const m of found(text, scope, dialect)) {
        const slot = slots.find((s) => s.name === m.name);
        if (slot) slot.spots.push({ from: m.from, to: m.to });
        else slots.push({ name: m.name, label: m.label, spots: [{ from: m.from, to: m.to }] });
      }
      return { slots, history: slots.length ? recall(keyOf(text, scope), slots.map((s) => s.name)) : [] };
    },
    wrap(run: Run): Run {
      return async (req, emit) => {
        if (!req.params) return run(req, emit);
        const used: ParamValue[] = [];
        const parts = inScope(req.text, req.scope).map((st) => {
          let sql = st.sql;
          const list = found(req.text, req.scope).filter((m) => m.from >= st.from && m.to <= st.from + st.sql.length);
          for (const m of list.sort((a, b) => b.from - a.from)) {
            const v = req.params?.find((p) => p.name === m.name);
            if (!v) throw new ApiError({ code: 'param', detail: { kind: 'missing', name: m.name } });
            sql = sql.slice(0, m.from - st.from) + literal(v) + sql.slice(m.to - st.from);
            if (!used.includes(v)) used.push(v);
          }
          return sql;
        });
        const text = parts.join(';\n');
        const key = keyOf(req.text, req.scope);
        const kept = (history.get(key) ?? []).filter((h) => !used.some((u) => u.name === h.name && u.value === h.value));
        history.set(key, [...used.map((u) => ({ ...u, used_at: ++clock, here: true })), ...kept].slice(0, 40));
        return run({ ...req, text, scope: { kind: 'script' } }, emit);
      };
    },
  };
}
