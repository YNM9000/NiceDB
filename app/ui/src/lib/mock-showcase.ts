import type { ConnectionProfile, SavedNote, Schema, Script, Snippet } from './api';
import type { ForeignKey, Relation_Serialize as Relation } from './bindings';
import { DEFAULT_SESSION } from './session';

type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

export const SHOWCASE_DB = 'observatory';

const pg = (host: string, database: string, user: string) =>
  ({ driver: 'postgres', host, port: 5432, database, user, tls: 'verify', ca_file: null, ssh: null, proxy: null, client_cert: null }) as const;
const profile = (id: string, name: string, folder: string, environment: ConnectionProfile['environment'], target: ConnectionProfile['target']): ConnectionProfile => ({
  id, name, folder, environment, color: null, session: DEFAULT_SESSION, template: { kind: 'none' }, raw_fields: {}, target,
});

export const SHOWCASE_CONNECTIONS: ConnectionProfile[] = [
  profile('bassi-prod', 'Observatory — production', 'f-bassi', 'production', pg('db.observatory.internal', SHOWCASE_DB, 'reader')),
  profile('bassi-staging', 'Observatory — staging', 'f-bassi', 'staging', pg('staging.observatory.internal', SHOWCASE_DB, 'astro')),
  profile('bassi-locale', 'Observatory — laptop', 'f-bassi', 'local', { driver: 'sqlite', path: '~/data/observatory.sqlite', read_only: false }),
  profile('marini', 'Sky survey', 'f-marini', 'development', { ...pg('127.0.0.1', SHOWCASE_DB, 'astro'), port: 55432, tls: 'disable' }),
];

export const SHOWCASE_FOLDERS = { 'f-bassi': 'Observatory', 'f-marini': 'Research' } as const;

const BRIGHT: [string, string, string, string, number, number][] = [
  ['Sirius', 'α CMa', 'Canis Major', 'A1V', -1.46, 8.6], ['Canopus', 'α Car', 'Carina', 'A9II', -0.74, 310], ['Rigil Kentaurus', 'α Cen', 'Centaurus', 'G2V', -0.27, 4.4],
  ['Arcturus', 'α Boo', 'Boötes', 'K1.5III', -0.05, 36.7], ['Vega', 'α Lyr', 'Lyra', 'A0V', 0.03, 25], ['Capella', 'α Aur', 'Auriga', 'G8III', 0.08, 42.9],
  ['Rigel', 'β Ori', 'Orion', 'B8Ia', 0.13, 860], ['Procyon', 'α CMi', 'Canis Minor', 'F5IV', 0.34, 11.5], ['Achernar', 'α Eri', 'Eridanus', 'B6V', 0.46, 139],
  ['Betelgeuse', 'α Ori', 'Orion', 'M1Ia', 0.5, 548], ['Hadar', 'β Cen', 'Centaurus', 'B1III', 0.61, 390], ['Altair', 'α Aql', 'Aquila', 'A7V', 0.76, 16.7],
  ['Acrux', 'α Cru', 'Crux', 'B0.5IV', 0.76, 320], ['Aldebaran', 'α Tau', 'Taurus', 'K5III', 0.86, 65.3], ['Antares', 'α Sco', 'Scorpius', 'M1Ib', 0.96, 550],
  ['Spica', 'α Vir', 'Virgo', 'B1V', 0.97, 250], ['Pollux', 'β Gem', 'Gemini', 'K0III', 1.14, 33.8], ['Fomalhaut', 'α PsA', 'Piscis Austrinus', 'A3V', 1.16, 25.1],
  ['Deneb', 'α Cyg', 'Cygnus', 'A2Ia', 1.25, 2600], ['Mimosa', 'β Cru', 'Crux', 'B0.5III', 1.25, 280], ['Regulus', 'α Leo', 'Leo', 'B8IV', 1.4, 79.3],
  ['Adhara', 'ε CMa', 'Canis Major', 'B2II', 1.5, 430], ['Castor', 'α Gem', 'Gemini', 'A1V', 1.58, 51], ['Shaula', 'λ Sco', 'Scorpius', 'B2IV', 1.62, 570],
  ['Gacrux', 'γ Cru', 'Crux', 'M3.5III', 1.64, 88.6], ['Bellatrix', 'γ Ori', 'Orion', 'B2III', 1.64, 250], ['Elnath', 'β Tau', 'Taurus', 'B7III', 1.65, 134],
  ['Miaplacidus', 'β Car', 'Carina', 'A1III', 1.67, 113], ['Alnilam', 'ε Ori', 'Orion', 'B0Ia', 1.69, 2000], ['Alnair', 'α Gru', 'Grus', 'B6V', 1.73, 101],
  ['Alnitak', 'ζ Ori', 'Orion', 'O9.5Ib', 1.77, 1260], ['Alioth', 'ε UMa', 'Ursa Major', 'A1III', 1.77, 82.6], ['Dubhe', 'α UMa', 'Ursa Major', 'K0III', 1.79, 123],
  ['Mirfak', 'α Per', 'Perseus', 'F5Ib', 1.8, 510], ['Wezen', 'δ CMa', 'Canis Major', 'F8Ia', 1.83, 1600], ['Kaus Australis', 'ε Sgr', 'Sagittarius', 'B9.5III', 1.85, 143],
  ['Sargas', 'θ Sco', 'Scorpius', 'F1III', 1.86, 300], ['Avior', 'ε Car', 'Carina', 'K3III', 1.86, 630], ['Alkaid', 'η UMa', 'Ursa Major', 'B3V', 1.86, 104],
  ['Menkalinan', 'β Aur', 'Auriga', 'A1IV', 1.9, 81], ['Atria', 'α TrA', 'Triangulum Australe', 'K2Ib', 1.91, 391], ['Alhena', 'γ Gem', 'Gemini', 'A1IV', 1.92, 109],
  ['Peacock', 'α Pav', 'Pavo', 'B2IV', 1.94, 179], ['Polaris', 'α UMi', 'Ursa Minor', 'F7Ib', 1.98, 433], ['Mirzam', 'β CMa', 'Canis Major', 'B1II', 1.98, 490],
  ['Alphard', 'α Hya', 'Hydra', 'K3II', 1.99, 177], ['Hamal', 'α Ari', 'Aries', 'K2III', 2, 66], ['Diphda', 'β Cet', 'Cetus', 'K0III', 2.02, 96],
  ['Nunki', 'σ Sgr', 'Sagittarius', 'B2.5V', 2.05, 228], ['Menkent', 'θ Cen', 'Centaurus', 'K0III', 2.06, 59],
];
const CONSTELLATIONS = ['Orion', 'Cygnus', 'Lyra', 'Cassiopeia', 'Andromeda', 'Perseus', 'Taurus', 'Gemini', 'Leo', 'Virgo', 'Scorpius', 'Sagittarius', 'Aquila', 'Hercules', 'Draco', 'Pegasus'];
const SPECTRA = ['B9V', 'A2V', 'F0V', 'G5V', 'K2III', 'M0III', 'A0V', 'K0III', 'F5V', 'G8III', 'B5V', 'M2III'];
const OBSERVERS: [string, string, string][] = [
  ['Henrietta Swan Leavitt', 'Harvard College Observatory', '1903-10-01'], ['Annie Jump Cannon', 'Harvard College Observatory', '1896-04-01'],
  ['Williamina Fleming', 'Harvard College Observatory', '1881-01-15'], ['Cecilia Payne-Gaposchkin', 'Harvard College Observatory', '1923-09-01'],
  ['Caroline Herschel', 'Slough', '1782-08-01'], ['Edwin Hubble', 'Mount Wilson', '1919-09-01'], ['Vera Rubin', 'Carnegie DTM', '1965-03-01'],
  ['Clyde Tombaugh', 'Lowell Observatory', '1929-01-15'], ['Maria Mitchell', 'Vassar College', '1865-09-01'], ['Jocelyn Bell Burnell', 'Mullard Radio', '1967-07-01'],
  ['Subrahmanyan Chandrasekhar', 'Yerkes Observatory', '1937-01-01'], ['Giovanni Schiaparelli', 'Brera', '1862-01-01'],
  ['Margherita Hack', 'Trieste', '1964-01-01'], ['Tycho Brahe', 'Uraniborg', '1576-01-01'], ['Johannes Kepler', 'Prague', '1600-02-01'], ['Hipparchus', 'Rhodes', '0147-01-01'],
];
const TELESCOPES: [string, string, string, string][] = [
  ['North dome', 'Ridge site', '1.20', 'f/8'], ['South dome', 'Ridge site', '0.80', 'f/7'], ['Survey camera', 'Valley site', '0.50', 'f/3.6'],
  ['Spectrograph bench', 'Ridge site', '1.20', 'f/15'], ['Solar tower', 'Valley site', '0.35', 'f/40'], ['Student refractor', 'Campus roof', '0.15', 'f/12'],
];
const FILTERS = ['U', 'B', 'V', 'R', 'I', 'Hα'];
const SKIES = ['photometric', 'clear', 'thin cirrus', 'clear', 'photometric', 'windy'];
const METHODS = ['transit', 'radial velocity', 'transit', 'direct imaging', 'microlensing', 'transit'];

const date = (base: number, i: number) => new Date(Date.UTC(2026, 0, 1) + ((base + i) % 300) * 86400000).toISOString().slice(0, 10);

function star(i: number): (string | null)[] {
  const b = BRIGHT[i - 1];
  if (b) return [String(i), b[0], b[1], b[2], b[3], b[4].toFixed(2), b[5].toFixed(1)];
  const mag = 2.06 + ((i - BRIGHT.length) / 118_218) * 4.4;
  return [String(i), null, `HIP ${10000 + ((i * 7919) % 108000)}`, CONSTELLATIONS[i % 16] ?? '', SPECTRA[i % 12] ?? '', mag.toFixed(2), (40 + ((i * 131) % 4000) / 3).toFixed(1)];
}

export const SHOWCASE_TABLES: Record<string, Gen> = {
  stars: {
    columns: [['id', 'int4'], ['name', 'text'], ['designation', 'text'], ['constellation', 'text'], ['spectral_type', 'text'], ['magnitude', 'numeric'], ['distance_ly', 'numeric']],
    total: 118_218,
    row: star,
  },
  exposures: {
    columns: [['id', 'int8'], ['night_id', 'int4'], ['star_id', 'int4'], ['filter', 'text'], ['frames', 'int4'], ['seconds', 'numeric']],
    total: 1_200_000,
    row: (i) => [String(i), String(1 + (i % 3650)), String(1 + ((i * 7) % 50)), FILTERS[i % 6] ?? 'V', String(1 + (i % 12)), (2 + (i % 40) * 7.5).toFixed(1)],
  },
  nights: {
    columns: [['id', 'int4'], ['observer_id', 'int4'], ['telescope_id', 'int4'], ['night_of', 'date'], ['seeing_arcsec', 'numeric'], ['sky', 'text']],
    total: 3650,
    row: (i) => [String(i), String(1 + (i % 16)), String(1 + (i % 6)), date(0, i), (0.6 + (i % 17) * 0.11).toFixed(2), SKIES[i % 6] ?? 'clear'],
  },
  observers: {
    columns: [['id', 'int4'], ['full_name', 'text'], ['institution', 'text'], ['joined_on', 'date']],
    total: OBSERVERS.length,
    row: (i) => { const o = OBSERVERS[(i - 1) % OBSERVERS.length]!; return [String(i), o[0], o[1], o[2]]; },
  },
  telescopes: {
    columns: [['id', 'int4'], ['name', 'text'], ['site', 'text'], ['aperture_m', 'numeric'], ['focal_ratio', 'text']],
    total: TELESCOPES.length,
    row: (i) => { const t = TELESCOPES[(i - 1) % TELESCOPES.length]!; return [String(i), ...t]; },
  },
  night_log: {
    columns: [['night_of', 'date'], ['clear_hours', 'numeric'], ['seeing_arcsec', 'numeric'], ['exposures', 'int4']],
    total: 365,
    row: (i) => {
      const dark = 9.5 + 3.2 * Math.cos(((i - 1) / 365) * Math.PI * 2);
      const clear = Math.max(0, dark * (0.6 + 0.28 * Math.sin(i * 0.21) + 0.1 * Math.sin(i * 1.3)));
      return [new Date(Date.UTC(2025, 10, 1) + (i - 1) * 86400000).toISOString().slice(0, 10), clear.toFixed(1), (0.7 + 0.5 * Math.abs(Math.sin(i * 0.47))).toFixed(2), String(Math.round(clear * 11))];
    },
  },
  discoveries: {
    columns: [['year', 'date'], ['method', 'text'], ['planets', 'int4']],
    total: 31 * 3,
    row: (i) => {
      const year = 1995 + Math.floor((i - 1) / 3);
      const method = ['radial velocity', 'transit', 'other'][(i - 1) % 3] ?? 'other';
      const t = year - 1995;
      const n = method === 'radial velocity' ? 2 + 40 * Math.exp(-(((year - 2011) / 6) ** 2))
        : method === 'transit' ? (year < 2002 ? 0 : 8 * t + (year === 2014 ? 820 : year === 2016 ? 1450 : year >= 2018 ? 160 + 12 * (year - 2018) : 0))
        : 1 + t * 1.6;
      return [`${year}-01-01`, method, String(Math.round(n))];
    },
  },
  visible_tonight: {
    columns: [['name', 'text'], ['constellation', 'text'], ['magnitude', 'numeric'], ['altitude_deg', 'numeric'], ['azimuth_deg', 'numeric'], ['rises_at', 'time']],
    total: 24,
    row: (i) => {
      const b = BRIGHT[(i * 3) % BRIGHT.length]!;
      return [b[0], b[2], b[4].toFixed(2), (12 + ((i * 29) % 70)).toFixed(1), ((i * 47) % 360).toFixed(1), `${String(18 + (i % 6)).padStart(2, '0')}:${String((i * 7) % 60).padStart(2, '0')}`];
    },
  },
  exoplanets: {
    columns: [['id', 'int4'], ['star_id', 'int4'], ['designation', 'text'], ['period_days', 'numeric'], ['radius_earth', 'numeric'], ['method', 'text']],
    total: 5_812,
    row: (i) => i === 1 ? ['1', '17', 'Pollux b', '589.64', null, 'radial velocity'] : i === 2 ? ['2', '18', 'Fomalhaut b', null, null, 'direct imaging']
      : [String(i), String(51 + ((i * 37) % 118000)), `HIP ${10000 + ((i * 6271) % 108000)} ${'bcde'[i % 4]}`, (0.7 + ((i * 53) % 4000) / 10).toFixed(2), (0.6 + (i % 140) / 10).toFixed(2), METHODS[i % 6] ?? 'transit'],
  },
};

const fk = (table: string, column: string, ref: string, on_delete: ForeignKey['on_delete'] = 'no_action'): ForeignKey =>
  ({ name: `${table}_${column}_fkey`, columns: [column], ref_namespace: 'public', ref_table: ref, ref_columns: ['id'], on_delete, on_update: 'no_action' });
const REFS: Record<string, ForeignKey[]> = {
  exposures: [fk('exposures', 'night_id', 'nights', 'cascade'), fk('exposures', 'star_id', 'stars', 'restrict')],
  nights: [fk('nights', 'observer_id', 'observers'), fk('nights', 'telescope_id', 'telescopes')],
  exoplanets: [fk('exoplanets', 'star_id', 'stars')],
};

const CHART_ONLY = ['night_log', 'discoveries', 'visible_tonight'];
export function showcaseSchema(): Schema {
  const tables: Relation[] = Object.entries(SHOWCASE_TABLES).filter(([name]) => !CHART_ONLY.includes(name)).map(([name, g]) => ({
    name,
    estimated_rows: g.total,
    columns: g.columns.map(([n, t], i) => ({ name: n, type_name: t, nullable: i > 1 && !n.endsWith('_id'), primary_key: i === 0 })),
    indexes: [{ name: `${name}_pkey`, columns: ['id'], unique: true }, ...(name === 'stars' ? [{ name: 'stars_magnitude_idx', columns: ['magnitude'], unique: false }] : [])],
    foreign_keys: REFS[name] ?? [],
  }));
  return {
    databases: [SHOWCASE_DB, 'observatory_archive', 'postgres'],
    current_database: SHOWCASE_DB,
    namespaces: [{
      name: 'public',
      tables,
      views: [{ name: 'visible_tonight', estimated_rows: 24, columns: SHOWCASE_TABLES.visible_tonight!.columns.map(([n, t]) => ({ name: n, type_name: t, nullable: true, primary_key: false })), indexes: [], foreign_keys: [] }],
      functions: [{ name: 'airmass', signature: 'altitude_deg numeric' }],
      sequences: [], types: [], triggers: [],
    }],
  };
}

const PLAN_WORDS: Record<string, string> = {
  righe_ordine_ordine_id_idx: 'exposures_night_id_idx', ordini_data_idx: 'nights_night_of_idx', clienti_pkey: 'observers_pkey', clienti_partita_iva_key: 'observers_designation_key',
  righe_ordine: 'exposures', ordini: 'nights', ordine: 'night', note: 'notes', clienti: 'observers', cliente: 'observer', fatture: 'reductions', fattura: 'reduction', articoli: 'telescopes', articolo: 'telescope',
  listini: 'filters', fornitori: 'sites', agenti: 'operators', magazzino: 'archive', pagamenti: 'calibrations', gestionale: 'observatory', gestionale_archivio: 'observatory_archive',
  ragione_sociale: 'full_name', partita_iva: 'designation', cliente_id: 'observer_id', ordine_id: 'night_id', articolo_id: 'telescope_id', fornitore_id: 'site_id', agente_id: 'operator_id',
  quantita: 'frames', prezzo: 'seconds', sconto: 'gain', ordinato: 'exposure_s', venduto: 'exposure_s', totale: 'exposure_s', imponibile: 'frames', iva: 'airmass',
  scadenza: 'night_of', pagata: 'reduced', data_pagamento: 'reduced_at', provincia: 'constellation', citta: 'site', fido: 'magnitude', numero: 'code', stato: 'sky',
  erp_web: 'pipeline', 'erp-web': 'reduce-nightly', mrossi: 'leavitt', lettura: 'reader', carico: 'loader', report: 'report', v_scadenzario: 'visible_tonight',
  stato_ordine: 'sky_condition', aggiorna_totale: 'stamp_exposure', registra_fattura: 'close_night', totale_cliente: 'airmass',
  Cliente: 'Observer', Clienti: 'Observers', Fattura: 'Reduction', Fatture: 'Reductions', Ordine: 'Night', Ordini: 'Nights', Magazzino: 'Archive', Articoli: 'Telescopes',
  analisti: 'analysts', codice_fiscale: 'tax_code', telefono: 'phone', 'Fatture dell’anno': 'This year’s reductions',
  "SQL injection nella ricostruzione dell'argomento di EXTRACT": 'SQL injection when rebuilding the argument of EXTRACT',
};
const WORDS_RE = new RegExp(`(?<![\\w-])(${Object.keys(PLAN_WORDS).sort((x, y) => y.length - x.length).map((w) => w.replace(/-/g, '\\-')).join('|')})(?![\\w-])`, 'g');
const COMPOUND = Object.keys(PLAN_WORDS).filter((w) => w.includes('_')).sort((x, y) => y.length - x.length);
const ident = (id: string) => {
  if (PLAN_WORDS[id]) return PLAN_WORDS[id];
  if (!id.includes('_')) return id;
  let out = id;
  for (const c of COMPOUND) out = out.split(c).join(PLAN_WORDS[c]!);
  return out.split('_').map((part) => (part.includes('_') ? part : (PLAN_WORDS[part] ?? part))).join('_');
};
export const retell = (text: string) => text.replace(WORDS_RE, (w) => PLAN_WORDS[w] ?? w).replace(/[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+/g, ident);
export function showcaseValue<T>(value: T): T {
  if (value === undefined || value === null || typeof value === 'function') return value;
  try {
    return JSON.parse(retell(JSON.stringify(value))) as T;
  } catch {
    return value;
  }
}
export const showcasePlan = <T,>(plan: T): T => showcaseValue(plan);

const S = (id: string, user: string, client: string, application: string, state: string, query: string, duration_ms: number | null, wait: string | null, blocked_by: string[] = []) =>
  ({ id, user, database: SHOWCASE_DB, client, application, state, query, duration_ms, wait, blocked_by });
export const SHOWCASE_SESSIONS = [
  S('18342', 'pipeline', '10.0.4.21', 'reduce-nightly', 'active', 'UPDATE nights SET sky = $1, seeing_arcsec = $2 WHERE id = $3', 4200, 'Lock: transactionid', ['18297']),
  S('18297', 'leavitt', '10.0.4.57', 'psql', 'idle in transaction', 'SELECT * FROM nights WHERE observer_id = 1 FOR UPDATE', 134_000, 'Client: ClientRead'),
  S('18410', 'report', '10.0.5.3', 'weekly-report', 'active', "SELECT c.full_name, sum(r.frames * r.seconds) AS exposure_s FROM exposures r JOIN nights o ON o.id = r.night_id JOIN observers c ON c.id = o.observer_id WHERE o.night_of >= date_trunc('year', now()) GROUP BY 1 ORDER BY 2 DESC", 48_300, 'IO: DataFileRead'),
  S('18402', 'backup', '10.0.9.2', 'pg_dump', 'active', 'COPY public.exposures (id, night_id, star_id, filter, frames, seconds) TO stdout;', 62_000, null),
  S('18388', 'pipeline', '10.0.4.21', 'reduce-nightly', 'idle', 'COMMIT', null, 'Client: ClientRead'),
  S('18355', 'pipeline', '10.0.4.22', 'reduce-nightly', 'idle', 'SELECT id, name FROM stars WHERE designation = $1', null, 'Client: ClientRead'),
];

const T0 = Date.UTC(2026, 9, 20, 21, 0);
const DAY = 86_400_000;
const script = (id: string, name: string, folder: string, tags: string[], connection_id: string | null, sql: string, age: number): Script =>
  ({ id, name, folder, tags, connection_id, sql, params: [], created_at: T0 - age * DAY, updated_at: T0 - age * DAY });
export const SHOWCASE_SCRIPTS: Script[] = [
  script('s-bright', 'Brightest stars', 'Catalog', ['stars'], null, 'SELECT name, constellation, magnitude\nFROM stars\nWHERE magnitude < 1.5\nORDER BY magnitude;', 9),
  script('s-tonight', 'Exposure time per observer', 'Reports/Weekly', ['nights', 'weekly'], 'bassi-prod', 'SELECT c.full_name, sum(r.frames * r.seconds) AS exposure_s\nFROM observers c\nJOIN nights o ON o.observer_id = c.id\nJOIN exposures r ON r.night_id = o.id\nGROUP BY c.full_name\nORDER BY exposure_s DESC;', 6),
  script('s-orphans', 'Exposures without a star', 'Checks', ['quality'], 'bassi-staging', 'SELECT id, night_id\nFROM exposures\nWHERE star_id IS NULL\nLIMIT 100;', 4),
  script('s-seeing', 'Seeing by month', 'Reports', ['nights'], null, "SELECT date_trunc('month', night_of) AS month, avg(seeing_arcsec) AS seeing\nFROM nights\nGROUP BY 1\nORDER BY 1;", 2),
];
export const SHOWCASE_SNIPPETS: Snippet[] = [{ id: 'u-near', name: 'Stars within a distance', trigger: 'near', description: 'Stars closer than a number of light years', body: 'SELECT name, distance_ly\nFROM stars\nWHERE distance_ly < ${1:50}\nORDER BY distance_ly;', dialect: null }];
export const SHOWCASE_NOTES: SavedNote[] = [
  { scope: 'connection', scope_id: 'marini', updated_at: T0 - DAY, body: '# Sky survey\n\nWorking copy of the catalog, **refreshed every night** at `06:00`.\n\n## Remember\n\n- Magnitudes are apparent, V band\n- Distances in light years, rounded to one decimal\n' },
  { scope: 'workspace', scope_id: 'w-bassi', updated_at: T0 - 2 * DAY, body: '# Observatory\n\nNight log closes at dawn. Production is read-only during observing runs.\n' },
  { scope: 'connection', scope_id: 'bassi-staging', updated_at: T0 - 3 * DAY, body: 'Staging is reloaded from production every Monday.' },
];
