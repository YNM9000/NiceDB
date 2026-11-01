import type { Api } from './api';

export function extras5Mock(base: Api): Partial<Api> {
  const stopped = new Set<string>();
  return {
    dataPick: async (sid, browse, jobId) => {
      const page = await base.dataBrowse(sid, browse);
      if (stopped.delete(jobId)) throw { code: 'driver', detail: { kind: 'cancelled' } };
      return browse.search && page.has_more ? { ...page, total: null, estimated: false } : page;
    },
    jobCancel: async (id) => {
      stopped.add(id);
      await base.jobCancel(id);
    },
  };
}
