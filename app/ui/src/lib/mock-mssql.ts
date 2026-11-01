import type { Api } from './api';

export function mssqlMock(browse: Api['dataBrowse'], isMssql: (sid: string) => boolean): Pick<Api, 'dataBrowse'> {
  return {
    dataBrowse: async (sid, b) => {
      const page = await browse(sid, b);
      const unfiltered = !b.filter?.trim() && !b.quick.length && !b.search?.trim();
      return isMssql(sid) && unfiltered && page.total !== null && page.has_more ? { ...page, estimated: true } : page;
    },
  };
}
