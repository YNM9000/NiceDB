type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

const REGIONS = ['Lombardia', 'Veneto', 'Emilia-Romagna', 'Piemonte', 'Toscana', 'Lazio', 'Campania', 'Puglia', 'Sicilia', 'Liguria', 'Marche', 'Trentino-Alto Adige'];
const CATEGORIES = ['Bulloneria', 'Utensili', 'Ferramenta'];
const MONTHS = 24;

const wave = (i: number, k: number) => Math.sin(i * 0.7 + k) * 0.5 + Math.cos(i * 0.23 + k * 2) * 0.5;

function sale(i: number): (string | null)[] {
  const r = (i - 1) % REGIONS.length;
  const c = Math.floor((i - 1) / REGIONS.length) % CATEGORIES.length;
  const month = Math.floor((i - 1) / (REGIONS.length * CATEGORIES.length));
  const date = new Date(Date.UTC(2025, month, 1)).toISOString().slice(0, 10);
  const weight = 1 / (1 + r * 0.35);
  const season = 1 + 0.18 * Math.sin(((month % 12) / 12) * Math.PI * 2 - 1.2);
  const ricavi = (52000 * weight * season * (1 + month * 0.012) * (0.85 + c * 0.12) * (1 + 0.08 * wave(i, c))).toFixed(2);
  return [String(i), date, REGIONS[r] ?? '', CATEGORIES[c] ?? '', ricavi, String(Math.round(Number(ricavi) / (180 + c * 40))), (Number(ricavi) * (0.18 + 0.04 * wave(i, r))).toFixed(2)];
}

export function chartTables(params: URLSearchParams): Record<string, Gen> {
  if (!params.has('b8')) return {};
  return {
    vendite: {
      columns: [['id', 'int4'], ['mese', 'date'], ['regione', 'text'], ['categoria', 'text'], ['ricavi', 'numeric'], ['ordini', 'int4'], ['margine', 'numeric']],
      total: REGIONS.length * CATEGORIES.length * MONTHS,
      row: sale,
    },
    incassi: {
      columns: [['giorno', 'timestamptz'], ['incasso', 'numeric'], ['scontrini', 'int4']],
      total: 180,
      row: (i) => {
        const day = new Date(Date.UTC(2026, 3, 1) + (i - 1) * 86_400_000);
        const weekend = day.getUTCDay() % 6 === 0 ? 0.45 : 1;
        const v = 3200 * weekend * (1 + 0.25 * wave(i, 1) + i * 0.002);
        return [`${day.toISOString().slice(0, 10)} 00:00:00+00`, v.toFixed(2), String(Math.round(v / 38))];
      },
    },
  };
}
