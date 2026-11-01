import type { ColumnInfo, ErLayout, ForeignKey, Index, Relation_Serialize as Relation, Schema_Serialize as Schema } from './bindings';

type Col = [name: string, type: string, nullable?: boolean];

const col = ([name, type_name, nullable = false]: Col, i: number): ColumnInfo => ({ name, type_name, nullable, primary_key: i === 0 });

function fk(ns: string, table: string, column: string, ref: string, on_delete: ForeignKey['on_delete'] = 'no_action'): ForeignKey {
  const [refNs, refTable] = ref.includes('.') ? ref.split('.') : [ns, ref];
  return { name: `${table}_${column}_fkey`, columns: [column], ref_namespace: refNs ?? ns, ref_table: refTable ?? ref, ref_columns: ['id'], on_delete, on_update: 'no_action' };
}

function table(ns: string, name: string, rows: number, cols: Col[], refs: [string, string, ForeignKey['on_delete']?][] = [], extra: Index[] = []): Relation {
  return {
    name,
    estimated_rows: rows,
    columns: cols.map(col),
    indexes: [{ name: `${name}_pkey`, columns: [cols[0]?.[0] ?? 'id'], unique: true }, ...extra],
    foreign_keys: refs.map(([c, ref, action]) => fk(ns, name, c, ref, action)),
  };
}

const PUBLIC: Relation[] = [
  table('public', 'agenti', 18, [['id', 'int4'], ['nome', 'text'], ['cognome', 'text'], ['zona', 'text'], ['provvigione', 'numeric(4,2)']]),
  table(
    'public',
    'clienti',
    2400,
    [['id', 'int4'], ['ragione_sociale', 'text'], ['partita_iva', 'char(11)'], ['codice_fiscale', 'char(11)'], ['citta', 'text'], ['provincia', 'char(2)'], ['agente_id', 'int4', true], ['fido', 'numeric(12,2)'], ['creato_il', 'timestamptz']],
    [['agente_id', 'agenti']],
    [{ name: 'clienti_partita_iva_key', columns: ['partita_iva'], unique: true }],
  ),
  table('public', 'fornitori', 342, [['id', 'int4'], ['ragione_sociale', 'text'], ['partita_iva', 'char(11)'], ['citta', 'text']]),
  table(
    'public',
    'articoli',
    8412,
    [['id', 'int4'], ['codice', 'text'], ['descrizione', 'text'], ['um', 'text'], ['prezzo_listino', 'numeric(10,2)'], ['aliquota_iva', 'int2'], ['giacenza', 'int4'], ['fornitore_id', 'int4', true]],
    [['fornitore_id', 'fornitori']],
  ),
  table('public', 'ordini', 96540, [['id', 'int4'], ['numero', 'text'], ['data', 'date'], ['cliente_id', 'int4'], ['stato', 'text'], ['totale', 'numeric(12,2)']], [['cliente_id', 'clienti']]),
  table(
    'public',
    'righe_ordine',
    1_200_000,
    [['id', 'int8'], ['ordine_id', 'int4'], ['articolo_id', 'int4'], ['quantita', 'numeric(10,2)'], ['prezzo', 'numeric(10,2)'], ['sconto', 'numeric(4,2)']],
    [['ordine_id', 'ordini', 'cascade'], ['articolo_id', 'articoli', 'restrict']],
    [{ name: 'righe_ordine_articolo_idx', columns: ['articolo_id'], unique: false }],
  ),
  table(
    'public',
    'fatture',
    90000,
    [['id', 'int4'], ['numero', 'text'], ['data', 'date'], ['cliente_id', 'int4'], ['imponibile', 'numeric(12,2)'], ['iva', 'numeric(12,2)'], ['totale', 'numeric(12,2)'], ['scadenza', 'date'], ['pagata', 'bool'], ['note', 'jsonb', true]],
    [['cliente_id', 'clienti']],
  ),
  table('public', 'pagamenti', 171904, [['id', 'int4'], ['fattura_id', 'int4'], ['data', 'date'], ['importo', 'numeric(12,2)'], ['metodo', 'text']], [['fattura_id', 'fatture']]),
  table(
    'public',
    'listini',
    25236,
    [['id', 'int4'], ['cliente_id', 'int4', true], ['articolo_id', 'int4'], ['prezzo', 'numeric(12,4)'], ['valido_dal', 'date'], ['valido_al', 'date', true]],
    [['cliente_id', 'clienti'], ['articolo_id', 'articoli']],
  ),
];

const MAGAZZINO: Relation[] = [
  table(
    'magazzino',
    'movimenti',
    612980,
    [['id', 'int8'], ['articolo_id', 'int4'], ['deposito_id', 'int4'], ['data', 'timestamptz'], ['causale', 'text'], ['quantita', 'numeric(12,3)']],
    [['deposito_id', 'depositi'], ['articolo_id', 'public.articoli']],
  ),
  table('magazzino', 'depositi', 6, [['id', 'int4'], ['nome', 'text'], ['indirizzo', 'text', true]]),
];

const WORDS = ['ordini', 'clienti', 'articoli', 'fatture', 'movimenti', 'contratti', 'listini', 'ticket', 'spedizioni', 'resi', 'commesse', 'turni', 'badge', 'cespiti', 'conti', 'mutui', 'stock', 'lotti', 'scadenze', 'fornitori'];
const AREAS = ['vendite', 'acquisti', 'magazzino', 'contabilita', 'personale', 'produzione', 'logistica', 'assistenza', 'crm', 'qualita'];
const TYPES = ['text', 'int4', 'numeric(12,2)', 'date', 'timestamptz', 'bool', 'varchar(80)', 'int8', 'jsonb'];

const hash = (i: number, salt: number) => (i * 2654435761 + salt * 40503) >>> 0;

export function bigSchema(count = 500): Schema {
  const per = Math.ceil(count / AREAS.length);
  const namespaces = AREAS.map((ns, a) => {
    const n = Math.min(per, count - a * per);
    const tables: Relation[] = [];
    for (let i = 0; i < n; i++) {
      const name = `${WORDS[i % WORDS.length]}_${String(i).padStart(2, '0')}`;
      const refs: [string, string][] = [];
      const cols: Col[] = [['id', 'int8']];
      const parents = i === 0 ? [] : [hash(i, 1) % i, ...(i > 3 && i % 3 === 0 ? [hash(i, 2) % i] : [])];
      for (const p of new Set(parents)) {
        const parent = tables[p]?.name ?? '';
        refs.push([`${parent}_id`, parent]);
        cols.push([`${parent}_id`, 'int8', (i + p) % 2 === 0]);
      }
      for (let k = 0; k < 2 + ((i * 5) % 9); k++) cols.push([`campo_${k}`, TYPES[(i + k) % TYPES.length] ?? 'text', k % 3 === 0]);
      const rel = table(ns, name, (i * 7919) % 900000, cols, refs);
      if (a > 0 && i % 25 === 3) rel.foreign_keys.push({ ...fk(AREAS[a - 1] ?? ns, name, 'id', `${WORDS[0]}_00`), name: `${name}_x_fkey` });
      tables.push(rel);
    }
    return { name: ns, tables, views: [], functions: [], sequences: [], types: [], triggers: [] };
  });
  return { databases: ['collaudo'], current_database: 'collaudo', namespaces };
}

export function erSchema(base: Schema, kind: string | null): Schema {
  if (kind === '500') return bigSchema();
  if (kind !== 'gestionale') return base;
  const pub = base.namespaces[0];
  return {
    ...base,
    namespaces: [
      { name: 'public', tables: structuredClone(PUBLIC), views: pub?.views ?? [], functions: pub?.functions ?? [], sequences: [], types: [], triggers: [] },
      { name: 'magazzino', tables: structuredClone(MAGAZZINO), views: [], functions: [], sequences: [], types: [], triggers: [] },
    ],
  };
}

const layouts = new Map<string, ErLayout>();

export const erApi = {
  erLayoutGet: async (connectionId: string, database: string) => structuredClone(layouts.get(`${connectionId}:${database}`) ?? null),
  erLayoutSet: async (connectionId: string, database: string, layout: ErLayout): Promise<void> => {
    layouts.set(`${connectionId}:${database}`, structuredClone(layout));
  },
  exportSave: async (name: string, data: Uint8Array): Promise<boolean> => {
    const url = URL.createObjectURL(new Blob([data.slice()], { type: name.endsWith('.svg') ? 'image/svg+xml' : 'image/png' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return true;
  },
};
