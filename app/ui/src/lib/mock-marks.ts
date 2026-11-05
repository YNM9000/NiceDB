import type { Marks } from './api';

const DEMO: Marks = {
  marks: [
    { id: 'm-fatture', label: 'fatture', pinned: true, target: { kind: 'table', connection_id: 'bassi-staging', database: 'gestionale', namespace: 'public', name: 'fatture', view: false } },
    { id: 'm-format', label: 'Formatta', pinned: true, target: { kind: 'item', key: 'cmd:query.format' } },
    { id: 'm-clienti', label: 'clienti', pinned: false, target: { kind: 'table', connection_id: 'bassi-staging', database: 'gestionale', namespace: 'public', name: 'clienti', view: false } },
    { id: 'm-scaduti', label: 'Fatture scadute non pagate', pinned: false, target: { kind: 'query', connection_id: 'bassi-staging', database: 'gestionale', sql: "SELECT numero, cliente_id, totale\nFROM fatture\nWHERE NOT pagata AND scadenza < current_date\nORDER BY scadenza;" } },
  ],
  recent: [],
};

export function marksMock(seed: boolean) {
  let saved: Marks = seed ? structuredClone(DEMO) : { marks: [], recent: [] };
  return {
    marksGet: async (): Promise<Marks> => structuredClone(saved),
    marksSet: async (value: Marks): Promise<Marks> => {
      saved = structuredClone({ marks: value.marks.slice(0, 500), recent: value.recent.slice(0, 40) });
      return structuredClone(saved);
    },
  };
}
