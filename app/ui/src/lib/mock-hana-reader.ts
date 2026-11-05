import type { Activity, Api, ConnectionProfile, Monitor, Principals, SlowQueries } from './api';
import { DEFAULT_SESSION } from './session';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);

export const HANA_READER_DEMO: ConnectionProfile[] = params().has('hanareader')
  ? [
      {
        id: 'hana-reader',
        name: 'Contabilità HANA — lettura',
        folder: null,
        environment: 'development',
        color: null,
        session: DEFAULT_SESSION,
        template: { kind: 'none' },
        raw_fields: {},
        target: { driver: 'hana', host: 'hana.bassi.lan', port: 30041, database: 'HXE', user: 'ND_READER', tls: 'verify', ca_file: null, ssh: null, proxy: null },
      },
    ]
  : [];

const NEED = { kind: 'permission', need: 'CATALOG READ' } as const;

const ACTIVITY: Activity = {
  sessions: [{ id: '300417', user: 'ND_READER', database: 'ND_READER', client: null, application: 'NiceDB', state: 'running', query: 'SELECT * FROM M_CONNECTIONS', duration_ms: 3, wait: null, blocked_by: [] }],
  locks: [],
  partial: true,
  can_cancel: true,
};

const MONITOR: Monitor = {
  connections: null,
  transactions: [],
  databases: [],
  cache_hit: null,
  gaps: [
    { part: 'cache_hit', reason: { kind: 'not_exposed' } },
    { part: 'connections', reason: NEED },
    { part: 'transactions', reason: NEED },
    { part: 'databases', reason: NEED },
  ],
};

const SLOW: SlowQueries = { source: null, gap: NEED, queries: [] };

const PRINCIPALS: Principals = {
  partial: true,
  quotas: [],
  principals: ['DBADMIN', 'ND_READER', 'PUBLIC', 'SYSTEM'].map((name) => ({ name, host: null, attributes: name === 'PUBLIC' ? [] : ['login'], member_of: name === 'ND_READER' ? ['PUBLIC'] : [] })),
  grants: [
    { grantee: 'ND_READER', kind: 'schema', schema: 'GESTIONALE', object: null, privilege: 'SELECT', grantable: false },
    { grantee: 'ND_READER', kind: 'schema', schema: 'ND_READER', object: null, privilege: 'CREATE ANY', grantable: true },
  ],
};

export function hanaReaderMock(base: Api, profileOf: (sessionId: string) => ConnectionProfile | undefined): Partial<Api> {
  const reader = (sid: string) => profileOf(sid.split('@')[0] ?? sid)?.id === 'hana-reader';
  return {
    serverActivity: async (sid) => (reader(sid) ? structuredClone(ACTIVITY) : base.serverActivity(sid)),
    serverMonitor: async (sid) => (reader(sid) ? structuredClone(MONITOR) : base.serverMonitor(sid)),
    serverSlowQueries: async (sid) => (reader(sid) ? structuredClone(SLOW) : base.serverSlowQueries(sid)),
    principalsList: async (sid) => (reader(sid) ? structuredClone(PRINCIPALS) : base.principalsList(sid)),
  };
}
