import type { Api, Preferences, SavedNote, Script, Snippet } from './api';
import { defaultPreferences } from './prefs';
import { SHOWCASE_NOTES, SHOWCASE_SCRIPTS, SHOWCASE_SNIPPETS } from './mock-showcase';

const SHOWCASE = typeof location !== 'undefined' && new URLSearchParams(location.search).has('showcase');

type Library = Pick<
  Api,
  'scriptsList' | 'scriptSave' | 'scriptDelete' | 'snippetsList' | 'snippetSave' | 'snippetDelete' | 'notesList' | 'noteSet' | 'preferencesGet' | 'preferencesSet'
>;

const KEY = 'nicedb.preferences';
const DAY = 86_400_000;
const T0 = Date.UTC(2026, 9, 20, 9, 0);

const script = (id: string, name: string, folder: string, tags: string[], connection_id: string | null, sql: string, age: number): Script => ({
  id,
  name,
  folder,
  tags,
  connection_id,
  sql,
  params: [],
  created_at: T0 - age * DAY,
  updated_at: T0 - age * DAY,
});

const SCRIPTS: Script[] = [
  script('s-scadute', 'Fatture da incassare', 'Report/Incassi', ['incassi', 'mensile'], 'bassi-prod', 'SELECT numero, data, totale, scadenza\nFROM fatture\nWHERE pagata = false\nORDER BY scadenza\nLIMIT 200;', 9),
  script('s-province', 'Clienti per provincia', 'Report', ['clienti'], null, 'SELECT provincia, COUNT(*) AS clienti\nFROM clienti\nGROUP BY provincia\nORDER BY clienti DESC;', 6),
  script('s-orfane', 'Righe senza articolo', 'Controlli', ['qualità'], 'bassi-staging', 'SELECT id, ordine_id\nFROM righe_ordine\nWHERE articolo_id IS NULL\nLIMIT 100;', 4),
  script('s-spazio', 'Dimensione delle tabelle', 'Controlli', ['manutenzione'], null, 'SELECT relname, pg_size_pretty(pg_total_relation_size(relid))\nFROM pg_catalog.pg_statio_user_tables\nORDER BY pg_total_relation_size(relid) DESC;', 2),
  script('s-ultimi', 'Ultimi ordini', '', [], null, 'SELECT * FROM clienti LIMIT 50;', 1),
];

const SNIPPETS: Snippet[] = [{ id: 'u-fmese', name: 'Fatturato per mese', trigger: 'fmese', description: 'Somma delle fatture per mese', body: "SELECT date_trunc('month', ${1:data}) AS mese, SUM(${2:totale}) AS fatturato\nFROM ${3:fatture}\nGROUP BY mese\nORDER BY mese;", dialect: 'postgres' }];

const NOTES: SavedNote[] = [
  {
    scope: 'connection',
    scope_id: 'marini',
    updated_at: T0 - DAY,
    body: '# Studio Marini\n\nServer del gestionale, **solo lettura** salvo manutenzione.\n\n## Da ricordare\n\n- Backup notturno alle `02:00`, non lanciare query pesanti dopo mezzanotte\n- Le fatture si chiudono il 5 del mese\n- [x] Chiesto a Marco l\'accesso al replicante\n- [ ] Indice su `fatture(cliente_id)`\n\n> Prima di ogni UPDATE controllare il numero di righe con un SELECT.\n\n```sql\nSELECT count(*) FROM fatture WHERE pagata = false;\n```\n',
  },
  { scope: 'workspace', scope_id: 'w-bassi', updated_at: T0 - 2 * DAY, body: '# Cliente Bassi\n\nReferente: Anna Bassi, anna@bassi.example.\nChiusura contabile ogni **5 del mese**.\n' },
  { scope: 'connection', scope_id: 'bassi-staging', updated_at: T0 - 3 * DAY, body: 'Staging ricaricato dalla produzione ogni lunedì. I dati di prova si perdono.' },
];

export function readPrefs(): Preferences {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<Preferences>;
    return { ...structuredClone(defaultPreferences), ...raw, editor: { ...defaultPreferences.editor, ...raw.editor }, results: { ...defaultPreferences.results, ...raw.results }, safety: { ...defaultPreferences.safety, ...raw.safety } };
  } catch {
    return structuredClone(defaultPreferences);
  }
}

export function libraryMock(demo: boolean): Library {
  let scripts: Script[] = demo ? structuredClone(SHOWCASE ? SHOWCASE_SCRIPTS : SCRIPTS) : [];
  let snippets: Snippet[] = demo ? structuredClone(SHOWCASE ? SHOWCASE_SNIPPETS : SNIPPETS) : [];
  let notes: SavedNote[] = demo ? structuredClone(SHOWCASE ? SHOWCASE_NOTES : NOTES) : [];

  return {
    scriptsList: async () => structuredClone(scripts),
    scriptSave: async (s) => {
      const old = scripts.find((x) => x.id === s.id);
      const saved: Script = { ...structuredClone(s), created_at: old?.created_at ?? (s.created_at || Date.now()), updated_at: Date.now() };
      scripts = [...scripts.filter((x) => x.id !== s.id), saved];
      return structuredClone(saved);
    },
    scriptDelete: async (id) => {
      scripts = scripts.filter((x) => x.id !== id);
    },
    snippetsList: async () => structuredClone(snippets),
    snippetSave: async (s) => {
      snippets = [...snippets.filter((x) => x.id !== s.id), structuredClone(s)];
    },
    snippetDelete: async (id) => {
      snippets = snippets.filter((x) => x.id !== id);
    },
    notesList: async () => structuredClone(notes),
    noteSet: async (scope, scope_id, body) => {
      notes = notes.filter((n) => !(n.scope === scope && n.scope_id === scope_id));
      if (!body.trim()) return null;
      const note: SavedNote = { scope, scope_id, body, updated_at: Date.now() };
      notes = [note, ...notes];
      return structuredClone(note);
    },
    preferencesGet: async () => readPrefs(),
    preferencesSet: async (v) => {
      try {
        localStorage.setItem(KEY, JSON.stringify(v));
      } catch {
        return structuredClone(v);
      }
      return structuredClone(v);
    },
  };
}
