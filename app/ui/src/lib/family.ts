import type { Capabilities, Dialect, Tool } from './bindings';
import type { Shape } from './explorer';
import { globEscape, quoteArg } from './redis';
import { limitedSql, quoteName, selectSql } from './sqlname';

export type { Capabilities };

export function relationalCaps(dialect: Dialect): Capabilities {
  return {
    family: 'relational',
    language: { kind: 'sql', dialect },
    schema: 'declared',
    results: 'rows',
    tree: { databases: true, namespaces: 'always', groups: ['tables', 'views', 'functions', 'sequences', 'types', 'triggers'], server: true },
    words: { table: 'table', column: 'column' },
    transactions: true,
    editable: true,
    explain: true,
    er: true,
    ddl: true,
    sort: true,
    params: true,
    tools: ['activity', 'monitor', 'space', 'users', 'compare', 'rows', 'backup', 'transfer', 'seclab', 'search', 'generate'],
    cancel: 'server',
  };
}

export const sqlOf = (info: { dialect?: Dialect | null }): Dialect => info.dialect ?? 'postgres';

export const isSql = (caps: Capabilities): boolean => caps.language.kind === 'sql';

export const hasTool = (caps: Capabilities, tool: Tool): boolean => caps.tools.includes(tool);

export function shapeOf(caps: Capabilities): Shape {
  return { databases: caps.tree.databases, schemas: caps.tree.namespaces, groups: caps.tree.groups, server: caps.tree.server };
}

export function viewsOf(caps: Capabilities): ('query' | 'data' | 'schema' | 'tools')[] {
  return ['query', ...(isSql(caps) || caps.results === 'keys' ? (['data'] as const) : []), ...(caps.er ? (['schema'] as const) : []), ...(caps.tools.length ? (['tools'] as const) : [])];
}

export function browseText(caps: Capabilities, name: string, sql: () => string): string {
  if (caps.language.kind === 'mql') return `{ "find": ${JSON.stringify(name)}, "filter": {}, "limit": 100 }`;
  if (caps.language.kind === 'redis') return `SCAN 0 MATCH ${quoteArg(`${globEscape(name)}:*`)} COUNT 100`;
  if (caps.language.kind === 'cql') return limitedSql('cql', '*', `FROM ${quoteName('cql', name)}`, 100);
  if (caps.language.kind === 'partiql') return selectSql('partiql', '', name);
  if (caps.language.kind === 'search') return `GET /${encodeURIComponent(name).replaceAll('%2A', '*')}/_search\n{ "query": { "match_all": {} }, "size": 100 }`;
  return sql();
}

export function namesOf(info: { caps: Capabilities; server: { product: string } }): Dialect | null {
  const lang = info.caps.language;
  if (lang.kind === 'sql') return lang.dialect;
  if (lang.kind === 'cql' || lang.kind === 'partiql') return lang.kind;
  if (lang.kind === 'search') return /elastic/i.test(info.server.product) ? 'elasticsearch' : 'opensearch';
  return null;
}
