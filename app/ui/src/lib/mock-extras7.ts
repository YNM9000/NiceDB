import type { Api, ConnectionProfile } from './api';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function lazyMock(base: Api, on: boolean): Partial<Api> {
  if (!on) return {};
  return {
    schemaOpen: async (sid) => {
      const all = await base.schemaLoad(sid);
      if (all.namespaces.length < 2) return { schema: all, pending: [] };
      const [first, ...rest] = all.namespaces;
      return {
        schema: { ...all, namespaces: [first!, ...rest.map((n) => ({ name: n.name, tables: [], views: [], functions: [], sequences: [], types: [], triggers: [] }))] },
        pending: rest.map((n) => ({ name: n.name, relations: n.tables.length + n.views.length })),
      };
    },
    schemaNamespace: async (sid, namespace) => {
      await sleep(new URLSearchParams(location.search).has('lazyhold') ? 60_000 : 400);
      return base.schemaNamespace(sid, namespace);
    },
  };
}

export function clickhouseGenMock(base: Api, profile: (sessionId: string) => ConnectionProfile | undefined, on: boolean): Partial<Api> {
  if (!on) return {};
  return {
    genRun: async (req, onProgress) => {
      if (profile(req.session_id)?.target.driver !== 'clickhouse') return base.genRun(req, onProgress);
      await sleep(300);
      return {
        tables: [{ namespace: req.namespace, name: req.name, rows: 3000 }],
        inserted: 3000,
        committed: false,
        kept: true,
        cancelled: false,
        fault: { kind: 'batch', table: req.name, from: 3001, to: 4000, message: `Code: 469. Constraint \`importo_positivo\` for table gestionale.${req.name} is violated at row 212 (VIOLATED_CONSTRAINT)` },
        elapsed_ms: 1840,
      };
    },
  };
}
