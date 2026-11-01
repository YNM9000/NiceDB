import type { ColumnInfo, ForeignKey, PackedSchema_Serialize as PackedSchema, Relation_Serialize as Relation, Schema_Serialize as Schema } from './bindings';
import { packSchema, unpackSchema } from './schema-wire';

const TYPES = ['int4', 'int8', 'text', 'numeric(12,2)', 'timestamptz', 'bool', 'jsonb', 'varchar(255)', 'date', 'uuid'];
const STEMS = ['anagrafica', 'movimento', 'registrazione', 'documento', 'contratto', 'listino', 'giacenza', 'spedizione', 'pagamento', 'provvigione'];
const LONG = '_storico_consolidato_per_esercizio_e_centro_di_costo';

function columns(table: number, wide: boolean): ColumnInfo[] {
  const count = wide ? 200 : 8 + (table % 17);
  return Array.from({ length: count }, (_, c) => ({
    name: c === 0 ? 'id' : `${STEMS[(table + c) % STEMS.length]}_campo_${String(c).padStart(3, '0')}`,
    type_name: c === 0 ? 'int8' : (TYPES[(table * 7 + c) % TYPES.length] ?? 'text'),
    nullable: c > 0,
    primary_key: c === 0,
  }));
}

function table(ns: string, i: number, perNs: number): Relation {
  const wide = i % 97 === 0;
  const long = i % 13 === 0;
  const name = `${STEMS[i % STEMS.length]}_${String(i).padStart(5, '0')}${long ? LONG : ''}`;
  const fks: ForeignKey[] =
    i % perNs > 0 && i % 3 === 0
      ? [{ name: `${name}_padre_fkey`, columns: ['padre_id'], ref_namespace: ns, ref_table: `${STEMS[(i - 1) % STEMS.length]}_${String(i - 1).padStart(5, '0')}${(i - 1) % 13 === 0 ? LONG : ''}`, ref_columns: ['id'], on_delete: 'no_action', on_update: 'no_action' }]
      : [];
  return {
    name,
    estimated_rows: (i * 7919) % 2_000_000,
    columns: columns(i, wide),
    indexes: [{ name: `${name}_pkey`, columns: ['id'], unique: true }],
    foreign_keys: fks,
  };
}

export function bigSchema(schema: Schema, params: URLSearchParams): Schema {
  const total = Number(params.get('big')) || 0;
  if (total <= 0) return schema;
  const schemas = 10;
  const perNs = Math.ceil(total / schemas);
  for (let s = 0; s < schemas; s++) {
    const name = `reparto_${String(s + 1).padStart(2, '0')}`;
    const tables: Relation[] = [];
    for (let i = s * perNs; i < Math.min(total, (s + 1) * perNs); i++) tables.push(table(name, i, perNs));
    const views = tables.slice(0, 40).map((t) => ({ ...t, name: `v_${t.name}`, indexes: [], foreign_keys: [] }));
    const functions = Array.from({ length: 60 }, (_, f) => ({ name: `calcola_${STEMS[f % STEMS.length]}_${f}`, signature: 'id bigint, al date' }));
    schema.namespaces.push({ name, tables, views, functions, sequences: [], types: [], triggers: [] });
  }
  return schema;
}

let cached: { key: string; json: string } | null = null;

export function overIpc(key: string, build: () => Schema): Schema {
  if (cached?.key !== key) cached = { key, json: JSON.stringify(packSchema(build())) };
  return unpackSchema(JSON.parse(cached.json) as PackedSchema);
}
