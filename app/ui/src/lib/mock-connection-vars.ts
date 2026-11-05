import { ApiError, type Api, type ConnectionProfile, type Workspace, type Workspaces } from './api';
import type { VarPreview } from './bindings';
import { follow, isModel } from './profile-template';
import { DEFAULT_SESSION } from './session';
import { expand, hasVars } from './vars';

const ENV: Record<string, string> = { HOME: '/home/ada', USER: 'ada', NICEDB_CERTS: '/home/ada/certificati' };

const SPACE_VARS: Record<string, Record<string, string>> = {
  'w-bassi': { BASSI_ENV: 'staging', PG_PORT: '6432', BASTION: 'bastion.bassi.it' },
};

const MODEL: ConnectionProfile = {
  id: 'bassi-modello',
  name: 'Bassi — modello',
  folder: 'f-bassi',
  environment: 'staging',
  color: null,
  session: { ...DEFAULT_SESSION, init: 'SET search_path TO vendite, public;', keepalive_secs: 120 },
  template: { kind: 'model' },
  raw_fields: { port: '${PG_PORT}' },
  target: {
    driver: 'postgres',
    host: 'db.${BASSI_ENV}.bassi.lan',
    port: 0,
    database: 'gestionale',
    user: 'app_${BASSI_ENV}',
    tls: 'verify',
    ca_file: '${NICEDB_CERTS}/ca-bassi.pem',
    ssh: { host: '${BASTION}', port: 22, user: 'ada', auth: { method: 'agent' }, jumps: [{ host: 'gw.bassi.it', port: 2222, user: 'ada', auth: { method: 'agent' } }] },
    proxy: null,
    client_cert: { cert: '${NICEDB_CERTS}/ada.crt', key: '${NICEDB_CERTS}/ada.key' },
  },
};

function followerOf(id: string, name: string, environment: ConnectionProfile['environment'], own: Partial<Extract<ConnectionProfile['target'], { driver: 'postgres' }>>, overrides: string[]): ConnectionProfile {
  const base = structuredClone(MODEL);
  const target = base.target.driver === 'postgres' ? { ...base.target, ...own } : base.target;
  return { ...base, id, name, environment, target, template: { kind: 'follows', id: MODEL.id, overrides } };
}

const DEMO: ConnectionProfile[] = [
  MODEL,
  followerOf('bassi-modello-staging', 'Bassi — staging dal modello', 'staging', {}, []),
  followerOf(
    'bassi-modello-prod',
    'Bassi — produzione dal modello',
    'production',
    { host: 'db-prod.bassi.lan', proxy: { kind: 'socks5', host: 'proxy.bassi.it', port: 1080, user: 'ada' } },
    ['host', 'proxy'],
  ),
];

type B2 = Pick<Api, 'varsPreview' | 'connectionSave' | 'connectionDelete' | 'connectionTest' | 'workspacesGet'>;

export function connectionVarsMock(demo: boolean, list: () => ConnectionProfile[], set: (next: ConnectionProfile[]) => void, base: Pick<Api, 'connectionTest' | 'workspacesGet' | 'workspaceSave'>): B2 {
  if (demo) set([...list(), ...structuredClone(DEMO)]);
  let seeded = !demo;

  async function spaces(): Promise<Workspaces> {
    const all = await base.workspacesGet();
    if (seeded) return all;
    seeded = true;
    for (const w of all.list) {
      const vars = SPACE_VARS[w.id];
      if (vars && !Object.keys(w.vars).length) await base.workspaceSave({ ...w, vars } satisfies Workspace);
    }
    return base.workspacesGet();
  }

  async function lookupFor(connectionId: string | null, folderHint: string | null) {
    const all = await spaces();
    const folder = list().find((c) => c.id === connectionId)?.folder ?? folderHint;
    const space = all.active ? all.list.find((w) => w.id === all.active) : all.list.find((w) => (connectionId && w.connections.includes(connectionId)) || (folder && w.folders.includes(folder)));
    const vars = space?.vars ?? {};
    return (name: string) => vars[name] ?? ENV[name];
  }

  function texts(p: ConnectionProfile): [string, string][] {
    const out: [string, string][] = [];
    const walk = (v: unknown, path: string) => {
      if (typeof v === 'string' && hasVars(v)) out.push([path, v]);
      else if (v && typeof v === 'object') for (const [k, x] of Object.entries(v)) walk(x, path ? `${path}.${k}` : k);
    };
    walk(p.target, '');
    for (const [k, v] of Object.entries(p.raw_fields)) out.push([k, v]);
    if (hasVars(p.session.init)) out.push(['session.init', p.session.init]);
    return out;
  }

  function saveOne(all: ConnectionProfile[], p: ConnectionProfile): ConnectionProfile[] {
    return [...all.filter((c) => c.id !== p.id), structuredClone(p)];
  }

  return {
    workspacesGet: spaces,
    varsPreview: async (connectionId, folder, items) => {
      const look = await lookupFor(connectionId, folder);
      return items.map((t): VarPreview => expand(t, '', look));
    },
    connectionSave: async (given) => {
      let p = structuredClone(given);
      let all = list();
      const t = p.template;
      if (t.kind === 'follows') {
        const model = all.find((c) => c.id === t.id && isModel(c));
        p = model && model.id !== p.id ? follow(model, p, t.overrides) : { ...p, template: { kind: 'none' } };
      }
      all = saveOne(all, p);
      for (const f of all.filter((c) => c.template.kind === 'follows' && c.template.id === p.id)) {
        const ft = f.template;
        all = saveOne(all, isModel(p) && ft.kind === 'follows' ? follow(p, f, ft.overrides) : { ...f, template: { kind: 'none' } });
      }
      set(all);
    },
    connectionDelete: async (id) => {
      set(list().filter((c) => c.id !== id).map((c) => (c.template.kind === 'follows' && c.template.id === id ? { ...c, template: { kind: 'none' } } : c)));
    },
    connectionTest: async (p, s) => {
      const look = await lookupFor(p.id, p.folder);
      for (const [field, text] of texts(p)) {
        const r = expand(text, field, look);
        if (r.kind === 'problem') throw new ApiError({ code: 'variable', detail: r.error });
      }
      return base.connectionTest(p, s);
    },
  };
}
