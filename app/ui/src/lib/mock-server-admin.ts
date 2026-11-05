import type { Activity, Api, ConnectionProfile, RunRequest, SessionEvent, SlowQueries } from './api';
import type { ClickhouseAccess_Serialize as ClickhouseAccess } from './bindings';
import { DEFAULT_SESSION } from './session';
import type { DriverId } from './drivers';

const params = () => new URLSearchParams(typeof location === 'undefined' ? '' : location.search);

function pathSet(text: string): string[] | null {
  const m = /\bset\s+(?:session\s+|local\s+)?search_path\s*(?:to|=)\s*([^;]+)/i.exec(text);
  if (!m?.[1]) return null;
  return m[1]
    .split(',')
    .map((s) => s.trim().replace(/^"|"$/g, ''))
    .filter((s) => s && !/^'?\$user'?$/i.test(s));
}

export const SERVER_ADMIN_DEMO: ConnectionProfile[] =
  typeof location !== 'undefined' && new URLSearchParams(location.search).has('extras4')
    ? [
        {
          id: 'officina-my',
          name: 'Officina — MySQL',
          folder: null,
          environment: 'development',
          color: null,
          session: DEFAULT_SESSION,
          template: { kind: 'none' },
          raw_fields: {},
          target: { driver: 'mysql', host: '10.0.4.30', port: 3306, database: 'gestionale', user: 'nicedb', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null },
        },
        {
          id: 'cache-sentinel',
          name: 'Cache — sentinel',
          folder: null,
          environment: 'development',
          color: null,
          session: DEFAULT_SESSION,
          template: { kind: 'none' },
          raw_fields: {},
          target: { driver: 'redis', host: '10.0.6.21:26379, 10.0.6.22:26379', port: 26379, database: 0, user: '', tls: 'disable', ca_file: null, ssh: null, proxy: null, client_cert: null, mode: { kind: 'sentinel', master: 'mymaster' } },
        },
      ]
    : [];

type Cell = string | null;
type Gen = { columns: [string, string][]; total: number; row: (i: number) => Cell[] };

const hex = (bytes: Uint8Array) => `\\x${Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')}`;

export function sampleTiff(w = 160, h = 96, shift = 0): Uint8Array {
  const pixels = w * h * 3;
  const tags: [number, number, number, number][] = [
    [256, 3, 1, w],
    [257, 3, 1, h],
    [258, 3, 3, 0],
    [259, 3, 1, 1],
    [262, 3, 1, 2],
    [273, 4, 1, 0],
    [277, 3, 1, 3],
    [278, 3, 1, h],
    [279, 4, 1, pixels],
    [284, 3, 1, 1],
  ];
  const ifdAt = 8;
  const ifdSize = 2 + tags.length * 12 + 4;
  const bitsAt = ifdAt + ifdSize;
  const dataAt = bitsAt + 6;
  const out = new Uint8Array(dataAt + pixels);
  const v = new DataView(out.buffer);
  out.set([0x49, 0x49, 0x2a, 0x00]);
  v.setUint32(4, ifdAt, true);
  v.setUint16(ifdAt, tags.length, true);
  tags.forEach(([tag, type, count, value], k) => {
    const at = ifdAt + 2 + k * 12;
    v.setUint16(at, tag, true);
    v.setUint16(at + 2, type, true);
    v.setUint32(at + 4, count, true);
    const val = tag === 258 ? bitsAt : tag === 273 ? dataAt : value;
    if (type === 3 && count === 1) v.setUint16(at + 8, val, true);
    else v.setUint32(at + 8, val, true);
  });
  for (let k = 0; k < 3; k++) v.setUint16(bitsAt + k * 2, 8, true);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const at = dataAt + (y * w + x) * 3;
      const band = Math.floor((x + y + shift * 4) / 16) % 3;
      out[at] = band === 0 ? 200 : 40 + y;
      out[at + 1] = band === 1 ? 170 : 60 + (x >> 1);
      out[at + 2] = band === 2 ? 220 : 90;
    }
  return out;
}

const SVG = (i: number) =>
  `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 80" width="240" height="160"><rect x="10" y="${10 + (i % 4) * 3}" width="100" height="14" fill="#5b7fa6"/><rect x="10" y="10" width="14" height="60" fill="#5b7fa6"/><circle cx="${70 + (i % 5) * 4}" cy="50" r="12" fill="none" stroke="#c9813a" stroke-width="3"/><script>document.title='x'</script></svg>`;

export function serverAdminTables(p: URLSearchParams): Record<string, Gen> {
  if (!p.has('extras4')) return {};
  const enc = new TextEncoder();
  return {
    disegni: {
      columns: [['id', 'int4'], ['nome', 'text'], ['schema_svg', 'text'], ['tavola', 'bytea'], ['scansione', 'bytea']],
      total: 40,
      row: (i) => [String(i), `Staffa ${String.fromCharCode(65 + (i % 26))}${i}`, SVG(i), hex(enc.encode(SVG(i + 1))), i % 5 === 4 ? null : hex(sampleTiff(160, 96, i))],
    },
  };
}

const MY_MDL: Activity = {
  sessions: [
    { id: '412', user: 'report', database: 'gestionale', client: '10.0.5.3', application: null, state: 'Sleep', query: 'SELECT count(*) FROM ordini WHERE data >= CURRENT_DATE - INTERVAL 30 DAY', duration_ms: 940_000, wait: null, blocked_by: [] },
    { id: '418', user: 'nicedb', database: 'gestionale', client: '10.0.4.57', application: null, state: 'Waiting for table metadata lock', query: 'ALTER TABLE ordini ADD COLUMN canale varchar(20)', duration_ms: 41_000, wait: null, blocked_by: [] },
    { id: '421', user: 'erp_web', database: 'gestionale', client: '10.0.4.21', application: null, state: 'Waiting for table metadata lock', query: 'SELECT * FROM ordini WHERE id = 120488', duration_ms: 12_000, wait: null, blocked_by: [] },
  ],
  locks: [
    { waiting: '418', blocking: '412', object: 'gestionale.ordini', mode: 'metadata EXCLUSIVE', waited_ms: 41_000 },
    { waiting: '421', blocking: '418', object: 'gestionale.ordini', mode: 'metadata SHARED_READ', waited_ms: 12_000 },
  ],
  partial: false,
  can_cancel: true,
};

const MS_SLOW: SlowQueries['queries'] = [
  { text: '(@P1 nvarchar(4000),@P2 date)SELECT numero, totale FROM fatture WHERE cliente_id = @P1 AND scadenza < @P2 ORDER BY scadenza', sample: null, calls: 18_400, total_ms: 1_204_000, mean_ms: 65.4, rows: 2_310_000 },
  { text: 'SELECT TOP 50 c.ragione_sociale, sum(r.quantita * r.prezzo) AS venduto FROM righe_ordine r JOIN ordini o ON o.id = r.ordine_id JOIN clienti c ON c.id = o.cliente_id GROUP BY c.ragione_sociale ORDER BY venduto DESC', sample: null, calls: 212, total_ms: 984_000, mean_ms: 4641, rows: 10_600 },
];

const CRDB_SLOW: SlowQueries['queries'] = [
  { text: 'SELECT numero, totale FROM fatture WHERE (cliente_id = _) AND (scadenza < _) ORDER BY scadenza', sample: null, calls: 18_400, total_ms: 1_204_000, mean_ms: 65.4, rows: 2_310_000 },
  { text: 'SELECT id, ragione_sociale FROM clienti WHERE id IN (_, __more1_10__)', sample: null, calls: 9120, total_ms: 402_000, mean_ms: 44.1, rows: 9120 },
];

const CH_ACCESS: ClickhouseAccess = {
  profiles: [
    {
      name: 'lettura',
      all: false,
      applies_to: ['report', 'lettura'],
      except: [],
      inherits: ['default'],
      settings: [
        { name: 'max_memory_usage', value: '10000000000', min: null, max: '20000000000', writability: null },
        { name: 'max_execution_time', value: '300', min: null, max: null, writability: null },
        { name: 'readonly', value: '1', min: null, max: null, writability: 'CONST' },
      ],
    },
    { name: 'carico_notturno', all: false, applies_to: ['etl', 'carico'], except: [], inherits: [], settings: [{ name: 'max_insert_threads', value: '8', min: null, max: null, writability: null }, { name: 'async_insert', value: '1', min: null, max: null, writability: null }] },
  ],
  policies: [
    { name: 'solo_lombardia', database: 'gestionale', table: 'clienti', filter: "provincia IN ('BG', 'BS', 'MI')", restrictive: false, all: false, applies_to: ['report'], except: [] },
    { name: 'senza_archiviati', database: 'gestionale', table: 'ordini', filter: 'stato != 3', restrictive: true, all: true, applies_to: [], except: ['nicedb'] },
  ],
};

export function serverAdminMock(base: Api, driverOf: (sid: string) => DriverId | undefined): Partial<Api> {
  const paths = new Map<string, string[]>();
  const pgLike = (sid: string) => ['postgres', 'cockroach', 'timescale', 'neon', 'supabase', 'yugabyte'].includes(driverOf(sid) ?? '');
  const startPath = () => params().get('searchpath')?.split(',').filter(Boolean) ?? ['public'];
  let watcher: ((e: SessionEvent) => void) | null = null;
  const SENTINEL_AT = ['10.0.6.11:6379', '10.0.6.12:6379'] as const;
  const sentinelLabel = (moved: boolean) => `db0 · mymaster ${SENTINEL_AT[moved ? 1 : 0]}`;
  const isSentinel = (sid: string) => sid.split('@')[0] === 'mock-cache-sentinel';
  return {
    sessionWatch: async (cb) => {
      watcher = cb;
      await base.sessionWatch(cb);
    },
    sessionOpen: async (id, db) => {
      const info = await base.sessionOpen(id, db);
      if (id !== 'cache-sentinel') return info;
      if (params().has('failover'))
        setTimeout(() => watcher?.({ type: 'reconnected', session_id: info.session_id, transaction_lost: false, server: { ...info.server, database: sentinelLabel(true) } }), 900);
      return { ...info, server: { ...info.server, database: sentinelLabel(false) } };
    },
    serverReport: async (sid) => {
      const report = await base.serverReport(sid);
      if (!isSentinel(sid)) return report;
      const rows = [
        ['master_name', 'mymaster'],
        ['master_now', SENTINEL_AT[1]],
        ['connected_to', SENTINEL_AT[1]],
        ['flags', 'master'],
        ['quorum', '2'],
        ['sentinels', '3'],
        ['asked', '10.0.6.21:26379'],
        ['replica', `${SENTINEL_AT[0]} · slave`],
        ['replica', '10.0.6.13:6379 · slave'],
      ];
      return { sections: [...report.sections, { id: 'sentinel', group: 'info' as const, columns: ['name', 'value'], rows }] };
    },
    fileLoad: async () => ({ name: 'scansione.tif', bytes: sampleTiff(120, 80, 3) }),
    searchPath: async (sid: string) => (pgLike(sid) ? (paths.get(sid) ?? startPath()) : null),
    clickhouseAccess: async (sid: string) => (driverOf(sid) === 'clickhouse' ? structuredClone(CH_ACCESS) : base.clickhouseAccess(sid)),
    serverActivity: async (sid: string) => (driverOf(sid) === 'mysql' && params().has('extras4') ? structuredClone(MY_MDL) : base.serverActivity(sid)),
    serverSlowQueries: async (sid: string) => {
      const d = driverOf(sid);
      if (!params().has('extras4') || (d !== 'sqlserver' && d !== 'cockroach')) return base.serverSlowQueries(sid);
      return { source: d === 'sqlserver' ? 'query_store' : 'statement_statistics', gap: null, queries: structuredClone(d === 'sqlserver' ? MS_SLOW : CRDB_SLOW) };
    },
    run: (req: RunRequest, emit) => {
      const set = pgLike(req.session_id) ? pathSet(req.text) : null;
      if (set) paths.set(req.session_id, set);
      return base.run(req, emit);
    },
  };
}
