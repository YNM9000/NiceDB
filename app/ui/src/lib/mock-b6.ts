import type { Schema } from './api';
import { rel } from './complete/fixture';

export function b6Schema(schema: Schema, params: URLSearchParams): Schema {
  if (!params.has('b6')) return schema;
  schema.namespaces.push({
    name: 'magazzino',
    tables: [rel('giacenze', ['articolo_id', 'deposito:text', 'quantita:numeric']), rel('Ordini Web', ['Id', 'Cliente:int4', 'Totale:numeric'])],
    views: [rel('v_sottoscorta', ['articolo_id', 'quantita:numeric'])],
    functions: [{ name: 'disponibile', signature: 'articolo integer' }],
    sequences: [],
    types: [],
    triggers: [],
  });
  const many = Number(params.get('b6')) || 0;
  if (many > 100) {
    const tables = Array.from({ length: many }, (_, i) => rel(`storico_${String(i).padStart(4, '0')}`, Array.from({ length: 12 }, (_, c) => `campo_${c}`)));
    schema.namespaces.push({ name: 'storico', tables, views: [], functions: [], sequences: [], types: [], triggers: [] });
  }
  return schema;
}
