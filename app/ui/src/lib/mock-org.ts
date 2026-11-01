import { DEFAULT_SESSION } from './session';
import { SHOWCASE_FOLDERS } from './mock-showcase';
import { ApiError, type Api, type ConnectionProfile, type Folder, type ImportPreview, type Workspace, type WorkspaceSession } from './api';

type Org = Pick<
  Api,
  | 'foldersList'
  | 'folderSave'
  | 'folderDelete'
  | 'nodeMove'
  | 'workspacesGet'
  | 'workspaceSave'
  | 'workspaceDelete'
  | 'workspaceSessionSave'
  | 'workspaceActivate'
  | 'packageExport'
  | 'packagePick'
  | 'packagePreview'
  | 'packageImport'
>;

const blank = (): WorkspaceSession => ({ tabs: [], active: null, split: 0.45, history_open: false, widths: {} });

const WIDTHS = 'nicedb.widths';

function storedWidths(): Record<string, WorkspaceSession['widths']> {
  try {
    return JSON.parse(localStorage.getItem(WIDTHS) ?? '{}') as Record<string, WorkspaceSession['widths']>;
  } catch {
    return {};
  }
}

function storeWidths(id: string | null, widths: WorkspaceSession['widths']) {
  try {
    localStorage.setItem(WIDTHS, JSON.stringify({ ...storedWidths(), [id ?? '']: widths }));
  } catch {
    return;
  }
}

const SHOWCASE = typeof location !== 'undefined' && new URLSearchParams(location.search).has('showcase');
const DEMO_FOLDERS: Folder[] = [
  { id: 'f-bassi', parent: null, name: SHOWCASE ? SHOWCASE_FOLDERS['f-bassi'] : 'Ferramenta Bassi', tint: null, mark: null, collapsed: false },
  { id: 'f-marini', parent: null, name: SHOWCASE ? SHOWCASE_FOLDERS['f-marini'] : 'Studio Marini', tint: null, mark: null, collapsed: false },
];

const RICH_FOLDERS: Folder[] = [
  { id: 'f-bassi', parent: null, name: 'Ferramenta Bassi', tint: 'ochre', mark: 'building', collapsed: false },
  { id: 'f-bassi-remoti', parent: 'f-bassi', name: 'Server remoti', tint: null, mark: null, collapsed: false },
  { id: 'f-marini', parent: null, name: 'Studio Marini', tint: 'green', mark: null, collapsed: false },
  { id: 'f-archivio', parent: null, name: 'Archivio 2024', tint: 'slate', mark: null, collapsed: true },
];

const ARCHIVED: ConnectionProfile = {
  id: 'rossi-old',
  name: 'Rossi Serramenti — vecchio',
  folder: 'f-archivio',
  environment: 'local',
  color: null,
  target: { driver: 'sqlite', path: '/home/ada/archivio/rossi.sqlite', read_only: true },
  session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {},
};

const DEMO_SPACES: Workspace[] = [
  { id: 'w-bassi', name: SHOWCASE ? 'Observatory' : 'Cliente Bassi', folders: ['f-bassi'], connections: [], session: blank(), vars: {} },
  { id: 'w-marini', name: SHOWCASE ? 'Research' : 'Studio Marini', folders: ['f-marini'], connections: [], session: blank(), vars: {} },
];

function demoPreview(): ImportPreview {
  return {
    sealed: true,
    created_at: Date.UTC(2026, 9, 21, 9, 30),
    history: 214,
    scripts: 5,
    snippets: 1,
    notes: 3,
    items: [
      { kind: 'folder', id: 'f-bassi', name: 'Ferramenta Bassi', path: [], conflict: { reason: 'same_id', existing: 'f-bassi' }, secret: false },
      { kind: 'connection', id: 'bassi-prod', name: 'Bassi — produzione', path: ['Ferramenta Bassi'], conflict: { reason: 'same_id', existing: 'bassi-prod' }, secret: true },
      { kind: 'connection', id: 'x-bassi-report', name: 'Bassi — reportistica', path: ['Ferramenta Bassi'], conflict: null, secret: true },
      { kind: 'connection', id: 'x-marini', name: 'Studio Marini — gestionale', path: ['Studio Marini'], conflict: { reason: 'same_name', existing: 'marini' }, secret: false },
      { kind: 'workspace', id: 'x-w', name: 'Cliente Bassi', path: [], conflict: { reason: 'same_name', existing: 'w-bassi' }, secret: false },
      { kind: 'settings', id: 'settings', name: '', path: [], conflict: { reason: 'replaces' }, secret: false },
    ],
  };
}

export function orgMock(demo: boolean, rich: boolean, list: () => ConnectionProfile[], set: (next: ConnectionProfile[]) => void): Org {
  let folders: Folder[] = demo ? structuredClone(rich ? RICH_FOLDERS : DEMO_FOLDERS) : [];
  if (demo && rich) set([...list().map((c) => (c.id === 'bassi-prod' || c.id === 'bassi-staging' ? { ...c, folder: 'f-bassi-remoti' } : c)), ARCHIVED]);
  const kept = storedWidths();
  let spaces: Workspace[] = (demo ? structuredClone(DEMO_SPACES) : []).map((w) => ({ ...w, session: { ...w.session, widths: kept[w.id] ?? {} } }));
  let active: string | null = null;
  let all = { ...blank(), widths: kept[''] ?? {} };

  const reorder = <T extends { id: string }>(items: T[], id: string, index: number, keep: (x: T) => boolean, patch: (x: T) => T) => {
    const moving = items.find((x) => x.id === id);
    if (!moving) return items;
    const rest = items.filter((x) => x.id !== id);
    const sibs = rest.filter(keep);
    const anchor = sibs[index];
    const at = anchor ? rest.indexOf(anchor) : sibs.length ? rest.indexOf(sibs[sibs.length - 1] as T) + 1 : rest.length;
    rest.splice(at, 0, patch(moving));
    return rest;
  };

  return {
    foldersList: async () => structuredClone(folders),
    folderSave: async (f) => {
      const i = folders.findIndex((x) => x.id === f.id);
      folders = i < 0 ? [...folders, structuredClone(f)] : folders.map((x) => (x.id === f.id ? { ...structuredClone(f), parent: x.parent } : x));
    },
    folderDelete: async (id) => {
      const gone = folders.find((f) => f.id === id);
      if (!gone) return;
      folders = folders.filter((f) => f.id !== id).map((f) => (f.parent === id ? { ...f, parent: gone.parent } : f));
      set(list().map((c) => (c.folder === id ? { ...c, folder: gone.parent } : c)));
      spaces = spaces.map((w) =>
        w.folders.includes(id)
          ? {
              ...w,
              folders: [...w.folders.filter((f) => f !== id), ...folders.filter((f) => f.parent === gone.parent && !w.folders.includes(f.id)).map((f) => f.id)],
              connections: [...w.connections, ...list().filter((c) => c.folder === gone.parent && !w.connections.includes(c.id)).map((c) => c.id)],
            }
          : w,
      );
    },
    nodeMove: async (node, parent, index) => {
      if (node.kind === 'folder') folders = reorder(folders, node.id, index, (f) => f.parent === parent, (f) => ({ ...f, parent }));
      else set(reorder(list(), node.id, index, (c) => c.folder === parent, (c) => ({ ...c, folder: parent })));
    },
    workspacesGet: async () => structuredClone({ active, all, list: spaces }),
    workspaceSave: async (w) => {
      const i = spaces.findIndex((x) => x.id === w.id);
      spaces = i < 0 ? [...spaces, structuredClone(w)] : spaces.map((x) => (x.id === w.id ? structuredClone(w) : x));
    },
    workspaceDelete: async (id) => {
      spaces = spaces.filter((w) => w.id !== id);
      if (active === id) active = null;
    },
    workspaceSessionSave: async (id, session) => {
      storeWidths(id, session.widths);
      if (id === null) all = structuredClone(session);
      else spaces = spaces.map((w) => (w.id === id ? { ...w, session: structuredClone(session) } : w));
    },
    workspaceActivate: async (id) => {
      active = id;
    },
    packageExport: async () => true,
    packagePick: async () => '/home/ada/Scaricati/bassi-ottobre.nicedb',
    packagePreview: async (_path, passphrase) => {
      if (!passphrase) throw new ApiError({ code: 'package', detail: 'needs_passphrase' });
      if (passphrase === 'sbagliata') throw new ApiError({ code: 'package', detail: 'wrong_passphrase' });
      return demoPreview();
    },
    packageImport: async (_path, _pass, choices) => {
      const items = demoPreview().items;
      const pick = (id: string) => choices.find((c) => c.id === id)?.resolution ?? (id === 'settings' || id === 'f-bassi' ? 'skip' : 'keep_both');
      const skipped = items.filter((i) => i.conflict && pick(i.id) === 'skip').length;
      const replaced = items.filter((i) => i.conflict && i.kind !== 'settings' && pick(i.id) === 'replace').length;
      const report = items.find((i) => i.id === 'x-bassi-report');
      if (report && !list().some((c) => c.id === report.id)) {
        set([...list(), { id: report.id, name: report.name, folder: 'f-bassi', environment: 'staging', color: null, target: { driver: 'postgres', host: 'report.bassi.lan', port: 5432, database: 'gestionale', user: 'report', tls: 'verify', ca_file: null, ssh: null, proxy: null, client_cert: null }, session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {} }]);
      }
      const added = items.filter((i) => i.kind !== 'settings' && !(i.conflict && pick(i.id) !== 'keep_both')).length;
      return { added, replaced, skipped, history: 214, settings: pick('settings') === 'replace', scripts: 5, snippets: 1, notes: 3, params: 18, profiles: 2, marks: 6 };
    },
  };
}
