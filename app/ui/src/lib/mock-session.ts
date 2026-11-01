import { ApiError, type ConnectionProfile, type RunEvent, type RunRequest, type SessionEvent } from './api';

type Run = (req: RunRequest, emit: (e: RunEvent) => void) => Promise<void>;

const OWN_LIMIT = /\b(limit|top|fetch)\b/i;
const BAD_INIT = /^\s*(selec|set\s+search_pth)\b/i;

export function sessionMock(profileOf: (sid: string) => ConnectionProfile | undefined) {
  let watcher: ((e: SessionEvent) => void) | null = null;
  let drop: { write: boolean } | null = null;
  if (typeof window !== 'undefined')
    Object.assign(window, {
      __ndSessionEvent: (e: SessionEvent) => watcher?.(e),
      __ndDropNext: (write = false) => (drop = { write }),
    });

  function limited(run: Run): Run {
    return async (req, emit) => {
      if (drop) {
        const { write } = drop;
        drop = null;
        emit({ type: 'statement', index: 0, from: 0, to: req.text.length, sql: req.text, unsortable: null });
        emit({ type: 'failed', index: 0, elapsed_ms: 4, error: { kind: 'io', message: 'server closed the connection unexpectedly' }, position: null, at: null });
        emit({ type: 'reconnected', index: 0, write, transaction_lost: false });
        emit({ type: 'finished', elapsed_ms: 5, cancelled: false });
        return;
      }
      const limit = profileOf(req.session_id)?.session.row_limit ?? null;
      if (!limit || req.all_rows) return run(req, emit);
      const sql = new Map<number, string>();
      const sent = new Map<number, number>();
      const cut = new Set<number>();
      return run(req, (e) => {
        if (e.type === 'statement') sql.set(e.index, e.sql);
        if (!('index' in e) || OWN_LIMIT.test(sql.get(e.index) ?? '')) return emit(e);
        if (e.type === 'rows') {
          const have = sent.get(e.index) ?? 0;
          const room = limit - have;
          if (room <= 0 || e.rows.length > room) cut.add(e.index);
          if (room <= 0) return;
          sent.set(e.index, have + Math.min(room, e.rows.length));
          return emit({ ...e, rows: e.rows.slice(0, room) });
        }
        if (e.type === 'done' && cut.has(e.index)) return emit({ ...e, row_count: limit, truncated: true, row_limit: limit });
        emit(e);
      });
    };
  }

  function checkInit(p: ConnectionProfile) {
    const statements = p.session.init.split(';').map((s) => s.trim()).filter(Boolean);
    const index = statements.findIndex((s) => BAD_INIT.test(s));
    const sql = statements[index];
    if (sql === undefined) return;
    throw new ApiError({ code: 'init', detail: { index, sql, error: { kind: 'query', message: `errore di sintassi vicino a «${sql.split(/\s+/)[1] ?? ''}»`, code: '42601', detail: null, hint: null, offset: null } } });
  }

  return {
    limited,
    checkInit,
    sessionWatch: async (cb: (e: SessionEvent) => void) => {
      watcher = cb;
    },
  };
}
