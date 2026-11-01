import type { Grant_Serialize as Grant, Principal_Serialize as Principal } from './bindings';
import type { GrantDraft, RoleDraft } from './tools';

export function roleRef(text: string, db: string): { role: string; db: string } {
  const at = text.lastIndexOf('@');
  return at > 0 ? { role: text.slice(0, at), db: text.slice(at + 1) } : { role: text, db };
}

function inline(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(inline).join(', ')}]`;
  if (v && typeof v === 'object') {
    const parts = Object.entries(v).map(([k, x]) => `${JSON.stringify(k)}: ${inline(x)}`);
    return parts.length ? `{ ${parts.join(', ')} }` : '{}';
  }
  return JSON.stringify(v);
}

const command = (doc: Record<string, unknown>) => inline(doc);

export function mongoCreate(r: RoleDraft): string {
  const db = r.host.trim() || 'admin';
  const roles = r.memberOf.map((m) => roleRef(m, db));
  if (!r.login) return command({ createRole: r.name, privileges: [], roles, $db: db });
  return command({ createUser: r.name, pwd: '', roles, $db: db });
}

export function mongoAlter(before: Principal, r: RoleDraft): string {
  const db = before.host ?? 'admin';
  const user = before.attributes.includes('login');
  const added = r.memberOf.filter((m) => !before.member_of.includes(m)).map((m) => roleRef(m, db));
  const removed = before.member_of.filter((m) => !r.memberOf.includes(m)).map((m) => roleRef(m, db));
  const out: string[] = [];
  if (added.length) out.push(command({ [user ? 'grantRolesToUser' : 'grantRolesToRole']: before.name, roles: added, $db: db }));
  if (removed.length) out.push(command({ [user ? 'revokeRolesFromUser' : 'revokeRolesFromRole']: before.name, roles: removed, $db: db }));
  return out.join('\n\n');
}

export function mongoDrop(p: Principal): string {
  return command({ [p.attributes.includes('login') ? 'dropUser' : 'dropRole']: p.name, $db: p.host ?? 'admin' });
}

export function mongoGrant(g: GrantDraft, to: Principal): string {
  if (!g.privileges.length) return '';
  const db = to.host ?? 'admin';
  const user = to.attributes.includes('login');
  if (g.kind === 'database') {
    const roles = g.privileges.map((role) => ({ role, db: g.object }));
    return command({ [user ? 'grantRolesToUser' : 'grantRolesToRole']: to.name, roles, $db: db });
  }
  return command({ grantPrivilegesToRole: to.name, privileges: [{ resource: { db: g.schema, collection: g.object }, actions: g.privileges }], $db: db });
}

export function mongoRevoke(g: Grant, of: Principal): string | null {
  const db = of.host ?? 'admin';
  if (of.attributes.includes('login')) {
    return g.kind === 'database' ? command({ revokeRolesFromUser: of.name, roles: [{ role: g.privilege, db: g.object ?? db }], $db: db }) : null;
  }
  const resource = g.kind === 'global' ? { cluster: true } : g.kind === 'table' ? { db: g.schema ?? '', collection: g.object ?? '' } : { db: g.object ?? '', collection: '' };
  return command({ revokePrivilegesFromRole: of.name, privileges: [{ resource, actions: [g.privilege] }], $db: db });
}
