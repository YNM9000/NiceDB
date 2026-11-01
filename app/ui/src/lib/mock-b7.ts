type Cell = string | null;
type Gen = { columns: [string, string][]; total: number; row: (i: number) => Cell[] };

const LOGO = '\\x89504e470d0a1a0a0000000d4948445200000078000000480802000000b27ca6280000014b4944415478daeddcbd0dc2301086e16cc0164cc000944c4343934919819a8201e8e848fc7367df7d79a5b7b67c8fa2c44aa42ce7cb8d06b44000b428f4e7fd22f380061a68021a68a081061a68021a68a081061ae864b30d08e843423fd7653ba0dd893db88f055d45ec77758b3f0c9b95f5acbda03b89f5b881ce0c6da82c636d0f6daeac61ad0cbdae77c362413b29b759030d741fb4ab72f63b35d040030d34d040cf80b63d75749e4080061a68eed140030db4f0bb0ea081e6c53fd040f3cdd013faf4b86e7710e55d875f75d0e5eb6a43973b5443372cddcfad415c0addbc74a7b50cf13e74e7d2cddc62c45bd086c455dc92c47fa19d947753250e071dc4da69b458d0d3b9fd868a083d85db7b9cb8d0c3b8c70c121dda957be40839a0cdb9c76f3e13b409f7ac6de7836e139fbedbc4d0b9021a68a00968a081061a68a0c9049af8c73fd054dc17655a9950eae627d40000000049454e44ae426082';
const PDF = `\\x${[...'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n'].map((ch) => ch.charCodeAt(0).toString(16).padStart(2, '0')).join('')}${'0a'.repeat(1400)}`;
const noise = (i: number) => `\\x${Array.from({ length: 96 }, (_, k) => ((i * 131 + k * 17) % 256).toString(16).padStart(2, '0')).join('')}`;

const CLIENTS = ['Serramenti Valtrompia S.a.s.', 'Officine Meccaniche Bassi S.r.l.', 'Edilnord Gatti S.p.A.', 'Carpenteria Locatelli S.n.c.'];
const GOODS = ['Bulloni M8', 'Dadi autobloccanti', 'Rondelle piane', 'Staffe zincate', 'Tasselli in nylon'];
const CITIES = ['Bergamo', 'Brescia', 'Seriate', 'Lumezzane'];

function order(i: number) {
  return {
    numero: `2026/${String(i).padStart(6, '0')}`,
    cliente: {
      id: 1 + ((i * 31) % 2400),
      ragione_sociale: CLIENTS[i % 4],
      indirizzi: [
        { tipo: 'sede', via: `Via Roma ${1 + (i % 90)}`, cap: '24121', citta: CITIES[i % 4] },
        { tipo: 'consegna', via: 'Via dei Mille 4', cap: '25065', citta: CITIES[(i + 1) % 4], 'orario ritiro': '8-12' },
      ],
    },
    righe: Array.from({ length: 2 + (i % 4) }, (_, k) => ({ articolo: `ART-${String(1 + ((i + k) % 4000)).padStart(5, '0')}`, descrizione: GOODS[(i + k) % 5], qta: 1 + ((i * k) % 12), prezzo: Number((0.4 + ((i + k) % 90) * 0.37).toFixed(2)) })),
    consegna: { data: '2026-03-14', vettore: i % 2 ? 'BRT' : 'GLS', note: null },
    pagato: i % 2 === 0,
    tag: i % 3 ? ['pec'] : ['urgente', 'pec'],
  };
}

function xml(i: number) {
  const o = order(i);
  return `<?xml version="1.0" encoding="UTF-8"?><ordine numero="${o.numero}"><cliente id="${o.cliente.id}">${(o.cliente.ragione_sociale ?? '').replace('&', '&amp;')}</cliente><righe>${o.righe.map((r) => `<riga articolo="${r.articolo}" qta="${r.qta}">${r.descrizione}</riga>`).join('')}</righe><consegna vettore="${o.consegna.vettore}"/></ordine>`;
}

const NOTE = [
  'Consegna concordata con il magazzino di Seriate.',
  'Il cliente chiede la bolla in duplice copia e il DDT firmato.',
  '',
  'Attenzione: le staffe zincate arrivano in due colli separati,',
  'il secondo collo parte dal fornitore solo dopo il saldo della fattura precedente.',
  'Telefonare al responsabile acquisti prima delle 10.',
];

export function valueTables(params: URLSearchParams): Record<string, Gen> {
  if (!params.has('b7')) return {};
  return {
    documenti: {
      columns: [['id', 'int4'], ['numero', 'text'], ['dati', 'jsonb'], ['tracciato', 'xml'], ['note', 'text'], ['logo', 'bytea'], ['allegato', 'bytea'], ['importo', 'numeric'], ['peso_kg', 'numeric']],
      total: 300,
      row: (i) => [
        String(i),
        order(i).numero,
        i % 7 === 6 ? null : JSON.stringify(order(i)),
        xml(i),
        i % 2 ? NOTE.join('\n') : NOTE[0] ?? null,
        i % 3 === 1 ? LOGO : null,
        i % 4 === 1 ? PDF : i % 4 === 2 ? noise(i) : null,
        i % 9 === 4 ? null : (120 + ((i * 53) % 9000) * 1.1).toFixed(2),
        (0.25 + (i % 40) * 0.125).toFixed(3),
      ],
    },
  };
}
