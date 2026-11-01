import type { DdlKind, Dialect, ObjectRef_Deserialize } from './bindings';
import type { ExNode } from './explorer';

export type ObjectRef = ObjectRef_Deserialize;

export function objectOf(n: ExNode, dialect: Dialect | null = null): ObjectRef | null {
  const p = n.path;
  if (p.namespace === null || p.name === null) return null;
  const at = (kind: DdlKind, extra: Partial<ObjectRef> = {}): ObjectRef => ({ kind, namespace: p.namespace ?? '', name: p.name ?? '', table: null, signature: null, ...extra });
  switch (n.kind) {
    case 'relation':
      return at(p.group === 'views' || p.group === 'materialized_views' ? 'view' : 'table');
    case 'routine':
      return at('routine', { signature: n.detail?.replace(/^\((.*)\)$/s, '$1') ?? null });
    case 'index':
      return p.id ? at('index', { name: p.id, table: p.name }) : null;
    case 'object':
      if (p.group === 'triggers') return at('trigger', { table: n.detail });
      if (p.group === 'sequences') return at('sequence');
      if (p.group === 'packages') return at('routine', { signature: 'PACKAGE' });
      if (p.group === 'types' && dialect === 'oracle') return at('routine', { signature: 'TYPE' });
      if (p.group === 'synonyms') return at('table', { signature: 'SYNONYM' });
      return null;
    default:
      return null;
  }
}
