type Gen = { columns: [string, string][]; total: number; row: (i: number) => (string | null)[] };

const LONG = 'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '.repeat(176).slice(0, 10_000);

export function stressTables(params: URLSearchParams): Record<string, Gen> {
  if (!params.has('stress')) return {};
  const extra = Array.from({ length: 116 }, (_, k): [string, string] => [`colonna_con_un_nome_molto_lungo_${String(k).padStart(3, '0')}`, k % 3 ? 'text' : 'int4']);
  return {
    stress: {
      columns: [['id', 'int4'], ['testo', 'text'], ['blob', 'bytea'], ['nota', 'varchar(10000)'], ...extra],
      total: 3000,
      row: (i) => [String(i), i % 2 ? LONG : `riga ${i}`, i % 4 ? null : `\\x${'ab'.repeat(i === 4 ? 500_000 : 64)}`, LONG.slice(0, (i * 37) % 10_000), ...extra.map((_, k) => (k % 3 ? `valore ${i}.${k}` : String(i * k)))],
    },
  };
}
