import type { Schema } from './api';

const SHOWCASE = typeof location !== 'undefined' && new URLSearchParams(location.search).has('showcase');
const SEQUENCES = SHOWCASE ? ['exposures_id_seq', 'nights_id_seq'] : ['ordini_numero_seq', 'fatture_numero_seq'];
const TYPES = SHOWCASE
  ? [{ name: 'sky_condition', detail: 'enum' }, { name: 'spectral_class', detail: 'domain' }]
  : [{ name: 'stato_ordine', detail: 'enum' }, { name: 'cap', detail: 'domain' }];
const TRIGGERS = SHOWCASE
  ? [{ name: 'stamp_exposure', detail: 'exposures' }, { name: 'close_night', detail: 'nights' }]
  : [{ name: 'aggiorna_totale', detail: 'righe_ordine' }, { name: 'registra_fattura', detail: 'fatture' }];

export function withObjects(schema: Schema, many: string | null): Schema {
  const [first, ...rest] = schema.namespaces;
  if (!first) return schema;
  const extra = Math.max(0, Number(many) || 0);
  const tables = Array.from({ length: extra }, (_, i) => ({
    name: `tabella_${String(i + 1).padStart(5, '0')}`,
    estimated_rows: (i * 37) % 5000,
    columns: [{ name: 'id', type_name: 'int4', nullable: false, primary_key: true }],
    indexes: [],
    foreign_keys: [],
  }));
  const grown = {
    ...first,
    tables: [...first.tables, ...tables],
    sequences: [...first.sequences, ...SEQUENCES.map((name) => ({ name, detail: null }))],
    types: [...first.types, ...TYPES],
    triggers: [...first.triggers, ...TRIGGERS],
  };
  return { ...schema, namespaces: [grown, ...rest] };
}
