import type { ExplainRequest, PlanReport } from './api';
import data from './mock-plan.json';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function estimated(r: PlanReport): PlanReport {
  return {
    ...r,
    execution: { kind: 'estimated', reason: 'requested' },
    plan: {
      ...r.plan,
      analyzed: false,
      total_ms: null,
      slowest: null,
      worst_estimate: null,
      nodes: r.plan.nodes.map((n) => ({ ...n, rows: null, loops: null, rows_removed: null, time_ms: null, self_ms: null, buffers: null })),
    },
  };
}

export function planMock(running: Map<string, () => void>, own: (sid: string) => PlanReport | null = () => null, retell: (r: PlanReport) => PlanReport = (r) => r) {
  const seen = new Map<string, number>();
  return {
    explain: async (req: ExplainRequest): Promise<PlanReport> => {
      let cancelled = false;
      running.set(req.run_id, () => (cancelled = true));
      await sleep(260);
      running.delete(req.run_id);
      if (cancelled) throw new Error('cancelled');
      const n = seen.get(req.session_id) ?? 0;
      seen.set(req.session_id, n + 1);
      const base = own(req.session_id) ?? retell(structuredClone((n === 0 ? data.before : data.after) as PlanReport));
      return req.mode === 'estimate' ? estimated(base) : base;
    },
  };
}
