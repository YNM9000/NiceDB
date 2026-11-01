import type { Browse, Page, Schema } from './api';
import type { FkAction, ForeignKey, KeyMatch_Deserialize as KeyMatch } from './bindings';

type Cell = string | null;
type Gen = { columns: [string, string][]; total: number; row: (i: number) => Cell[] };

const STATES = ['bozza', 'confermato', 'spedito', 'chiuso'];
const GOODS = ['Bulloni M8', 'Dadi autobloccanti', 'Rondelle piane', 'Viti autofilettanti', 'Tasselli in nylon', 'Staffe zincate', 'Rivetti a strappo', 'Barre filettate'];
const day = (i: number) => new Date(Date.UTC(2025, 0, 1 + (i % 600))).toISOString().slice(0, 10);

export function fkTables(params: URLSearchParams): Record<string, Gen> {
  if (!params.has('fk')) return {};
  return {
    ordini: {
      columns: [['id', 'int4'], ['numero', 'text'], ['data', 'date'], ['cliente_id', 'int4'], ['stato', 'text'], ['totale', 'numeric']],
      total: 120_000,
      row: (i) => [String(i), `2026/${String(i).padStart(6, '0')}`, day(i), String(1 + ((i * 31) % 2400)), STATES[i % 4] ?? null, (120 + ((i * 53) % 9000) * 1.1).toFixed(2)],
    },
    articoli: {
      columns: [['id', 'int4'], ['codice', 'text'], ['descrizione', 'text'], ['prezzo_listino', 'numeric']],
      total: 4000,
      row: (i) => [String(i), `ART-${String(i).padStart(5, '0')}`, `${GOODS[i % 8]} ${10 + (i % 40)} mm`, (0.15 + (i % 400) * 0.37).toFixed(2)],
    },
    listini: {
      columns: [['articolo_id', 'int4'], ['anno', 'int4'], ['prezzo', 'numeric']],
      total: 8000,
      row: (i) => [String(1 + ((i - 1) >> 1)), String(2025 + ((i - 1) % 2)), (0.2 + (i % 400) * 0.4).toFixed(2)],
    },
    promozioni: {
      columns: [['id', 'int4'], ['articolo_id', 'int4'], ['anno', 'int4'], ['sconto', 'numeric']],
      total: 600,
      row: (i) => [String(i), String(1 + (i % 40)), String(2025 + (i % 2)), `${5 + (i % 4) * 5}.00`],
    },
  };
}

const fk = (table: string, columns: string[], ref: string, refColumns: string[], action: FkAction = 'no_action'): [string, ForeignKey] => [
  table,
  { name: `${table}_${columns.join('_')}_fkey`, columns, ref_namespace: 'public', ref_table: ref, ref_columns: refColumns, on_delete: action, on_update: 'no_action' },
];

const LINKS = [
  fk('fatture', ['cliente_id'], 'clienti', ['id']),
  fk('ordini', ['cliente_id'], 'clienti', ['id']),
  fk('righe_ordine', ['ordine_id'], 'ordini', ['id'], 'cascade'),
  fk('righe_ordine', ['articolo_id'], 'articoli', ['id']),
  fk('listini', ['articolo_id'], 'articoli', ['id']),
  fk('promozioni', ['articolo_id', 'anno'], 'listini', ['articolo_id', 'anno']),
];

export function withForeignKeys(schema: Schema, params: URLSearchParams): Schema {
  if (!params.has('fk')) return schema;
  for (const ns of schema.namespaces) {
    if (ns.name !== 'public') continue;
    for (const t of ns.tables) {
      t.foreign_keys = LINKS.filter(([name]) => name === t.name).map(([, k]) => k);
      if (t.name === 'listini') {
        for (const c of t.columns) c.primary_key = c.name !== 'prezzo';
        t.indexes = [{ name: 'listini_pkey', columns: ['articolo_id', 'anno'], unique: true }];
      }
    }
  }
  return schema;
}

type Browser = (sessionId: string, b: Browse) => Promise<Page>;

function lines(column: string, v: number): number | undefined {
  if (!Number.isInteger(v) || v < 1) return 0;
  if (column === 'ordine_id') return v <= 120_000 ? 10 : 0;
  if (column === 'articolo_id') return v <= 4000 ? 300 : 0;
  return undefined;
}

export function fkMock(browse: Browser) {
  const counts = new Map<string, number>();
  return {
    dataBrowse: async (sessionId: string, b: Browse): Promise<Page> => {
      const text = b.search?.trim().toLowerCase();
      const page = text ? await browse(sessionId, { ...b, limit: 100_000, offset: 0, after: null, search: null }) : await browse(sessionId, b);
      const at = b.columns.map((c) => page.columns.findIndex((x) => x.name === c));
      const pick = at.length && at.every((i) => i >= 0) ? (r: Cell[]) => at.map((i) => r[i] ?? null) : (r: Cell[]) => r;
      const columns = at.length && at.every((i) => i >= 0) ? at.flatMap((i) => page.columns[i] ?? []) : page.columns;
      if (!text) return { ...page, columns, rows: page.rows.map(pick) };
      const hit = page.rows.map(pick).filter((r) => r.some((v) => v?.toLowerCase().includes(text)));
      const rows = hit.slice(b.offset ?? 0, (b.offset ?? 0) + b.limit);
      return { ...page, columns, rows, has_more: hit.length > (b.offset ?? 0) + b.limit, next: null, total: hit.length };
    },
    dataRelated: async (sessionId: string, matches: KeyMatch[]): Promise<(number | null)[]> => {
      const out: (number | null)[] = [];
      for (const k of matches) {
        const id = `${k.table.name}|${k.key.map((f) => `${f.column}=${f.value}`).join('&')}`;
        let n = counts.get(id);
        if (n === undefined && k.table.name === 'righe_ordine') n = lines(k.key[0]?.column ?? '', Number(k.key[0]?.value));
        if (n === undefined) {
          const page = await browse(sessionId, { table: k.table, columns: [], key: [], filter: null, quick: k.key.map((f) => ({ column: f.column, op: 'eq', value: f.value ?? null })), order: [], limit: 1, offset: 0, after: null });
          n = page.total ?? 0;
          counts.set(id, n);
        }
        out.push(n);
      }
      return out;
    },
  };
}
