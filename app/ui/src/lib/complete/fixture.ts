import type { Schema } from '../api';
import type { ForeignKey } from '../bindings';

type Rel = Schema['namespaces'][number]['tables'][number];

export function rel(name: string, columns: string[], foreign_keys: ForeignKey[] = []): Rel {
  return {
    name,
    estimated_rows: null,
    columns: columns.map((c, i) => {
      const [n, t] = c.split(':');
      return { name: n ?? c, type_name: t ?? 'int4', nullable: i > 0, primary_key: i === 0 };
    }),
    indexes: [],
    foreign_keys,
  };
}

export const fk = (columns: string[], ref_table: string, ref_columns: string[], ref_namespace = 'public'): ForeignKey => ({
  name: `fk_${columns.join('_')}_${ref_table}`,
  columns,
  ref_namespace,
  ref_table,
  ref_columns,
  on_delete: 'no_action',
  on_update: 'no_action',
});

const ns = (name: string, tables: Rel[], views: Rel[] = [], functions: { name: string; signature: string }[] = []) => ({ name, tables, views, functions, sequences: [], types: [], triggers: [] });

export function shop(home = 'public'): Schema {
  return {
    databases: ['gestionale', 'archivio'],
    current_database: 'gestionale',
    namespaces: [
      ns(
        home,
        [
          rel('clienti', ['id', 'ragione_sociale:text', 'citta:text']),
          rel('fatture', ['numero:text', 'data:date', 'cliente_id', 'totale:numeric'], [fk(['cliente_id'], 'clienti', ['id'], home)]),
          rel('ordini', ['id', 'cliente_id', 'data:date'], [fk(['cliente_id'], 'clienti', ['id'], home)]),
          rel('righe_ordine', ['id', 'ordine_id', 'articolo_id', 'quantita:numeric'], [fk(['ordine_id'], 'ordini', ['id'], home), fk(['articolo_id'], 'articoli', ['id'], home)]),
          rel('articoli', ['id', 'codice:text']),
        ],
        [rel('v_scadenzario', ['numero:text', 'scadenza:date'])],
        [{ name: 'totale_cliente', signature: 'cid integer' }],
      ),
      ns('magazzino', [rel('giacenze', ['articolo_id', 'quantita:numeric']), rel('Ordini Web', ['Id', 'Totale:numeric'])]),
    ],
  };
}

export function huge(count: number, columns: number): Schema {
  const tables: Rel[] = [];
  for (let t = 0; t < count; t++) {
    tables.push(rel(`tabella_${t}`, Array.from({ length: columns }, (_, c) => `colonna_${c}`), t > 0 ? [fk(['colonna_1'], `tabella_${t - 1}`, ['colonna_0'])] : []));
  }
  return { databases: ['grande'], current_database: 'grande', namespaces: [ns('public', tables)] };
}
